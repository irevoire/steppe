//! ## Steppe
//! This crate is used to track the progress of a task through multiple steps composed of multiple states.
//! 
//! The objectives are:
//! - Have a very simple API to describe the steps composing a task. (create the steps and update the progress)
//! - Provide an easy way to display the current progress while the process is running.
//! - Provide a way to get the accumulated durations of each steps to quickly see the bottleneck.
//! - Don't slow down the main process too much.
//! 
//! The crate is composed of only 2 parts:
//! - The [`DefaultProgress`] struct that is used to track the progress of the task.
//! - The [`Step`] trait that is used to describe the steps composing a task.
//! 
//! The [`DefaultProgress`] struct is thread-safe, can be cloned cheaply and shared everywhere. While a thread is updating it another can display what we're doing.
//! The [`Step`] trait is used to describe the steps composing a task.
//! 
//! The API of the [`DefaultProgress`] is made of three parts:
//! - Add something to the stack of steps being processed with the [`DefaultProgress::update`] method. It accepts any type that implements the [`Step`] trait.
//! - Get the current progress view with the [`DefaultProgress::as_progress_view`] method.
//! - Get the accumulated durations of each steps with the [`DefaultProgress::accumulated_durations`] method.
//!
//! There is also a [`Progress`] trait that your library should accept in parameter in case a client wants to use a custom progress implementation.
//! 
//! Since creating [`Step`]s is a bit tedious, you can use the following helpers:
//! - [`make_enum_progress`] macro.
//! - [`make_atomic_progress`] macro.
//! - Or implement the [`NamedStep`] trait.
//! 
//! ```rust
//! use std::sync::atomic::Ordering;
//! use steppe::{make_enum_progress, make_atomic_progress, Progress, Step, NamedStep, AtomicSubStep};
//! 
//! // This will create a new enum that implements the `Step` trait automatically. Take care it's very case sensitive.
//! make_enum_progress! {
//!     pub enum TamosDay {
//!         PetTheDog,
//!         WalkTheDog,
//!         TypeALotOnTheKeyboard,
//!         WalkTheDogAgain,
//!     }
//! }
//! 
//! // This create a new struct that implement the `Step` trait automatically.
//! // It's displayed as "key strokes" and we cannot change its name.
//! make_atomic_progress!(KeyStrokes alias AtomicKeyStrokesStep => "key strokes");
//! 
//! let mut progress = steppe::DefaultProgress::default();
//! progress.update(TamosDay::PetTheDog); // We're at 0/4 and 0% of completion
//! progress.update(TamosDay::WalkTheDog); // We're at 1/4 and 25% of completion
//! 
//! progress.update(TamosDay::TypeALotOnTheKeyboard); // We're at 2/4 and 50% of completion
//! let (atomic, key_strokes) = AtomicKeyStrokesStep::new(1000);
//! progress.update(key_strokes);
//! // Here we enqueued a new step that have 1000 total states. Since we don't want to take a lock everytime
//! // we type on the keyboard we're instead going to increase an atomic without taking the mutex.
//!
//! atomic.fetch_add(500, Ordering::Relaxed);
//! // If we fetch the progress at this point it should be exactly between 50% and 75%.
//! 
//! progress.update(TamosDay::WalkTheDogAgain); // We're at 3/4 and 75% of completion
//! // By enqueuing this new step the progress is going to drop everything that was pushed after the `TamosDay` type was pushed.
//! ```

use std::any::TypeId;
use std::borrow::Cow;
use std::marker::PhantomData;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use indexmap::IndexMap;
use serde::Serialize;

/// The main trait of the crate. That describes an unit of works.
/// - It contains a name that can change over time.
/// - A total number of state the step must go through.
/// - The current state of the step.
/// 
/// The `current` should never exceed the `total`.
pub trait Step: 'static + Send + Sync {
    fn name(&self) -> Cow<'static, str>;
    fn current(&self) -> u64;
    fn total(&self) -> u64;
}

pub trait Progress: 'static + Send + Sync {
    fn update(&self, sub_progress: impl Step);
}

/// The main struct of the crate.
/// It stores the current steps we're processing.
/// It also contains the durations of each steps.
/// 
/// The structure is thread-safe, can be cloned cheaply and shared everywhere.
/// But keep in mind that when you update the step you're at we must take a mutex.
/// If you need to quickly update a tons of values you may want to use atomic numbers
/// that you can update without taking the mutex.
#[derive(Clone, Default)]
pub struct DefaultProgress {
    steps: Arc<RwLock<InnerProgress>>,
}

impl Progress for DefaultProgress {
    fn update(&self, sub_progress: impl Step) {
        self.update(sub_progress);
    }
}

#[derive(Default)]
struct InnerProgress {
    /// The hierarchy of steps.
    steps: Vec<(TypeId, Box<dyn Step>, Instant)>,
    /// The durations associated to each steps.
    durations: Vec<(String, Duration)>,
}

impl DefaultProgress {
    /// Update the progress of the current step.
    /// 
    /// If the step is not found, it will be added.
    /// If the step is found, it will be updated.
    /// 
    /// If the step is found and the current is higher than the total, it will be ignored.
    pub fn update<P: Step>(&self, sub_progress: P) {
        let mut inner = self.steps.write().unwrap();
        let InnerProgress { steps, durations } = &mut *inner;

        let now = Instant::now();
        let step_type = TypeId::of::<P>();
        if let Some(idx) = steps.iter().position(|(id, _, _)| *id == step_type) {
            push_steps_durations(steps, durations, now, idx);
            steps.truncate(idx);
        }

        steps.push((step_type, Box::new(sub_progress), now));
    }

    /// Drop all the steps and update the durations.
    /// 
    /// This is not mandatory. But if you don't do it and take a lot of time before calling [`Progress::accumulated_durations`] the last step will appear as taking more time than it actually did.
    /// Directly calling [`Progress::accumulated_durations`] instead of `finish` will give the same result.
    pub fn finish(&self) {
        let mut inner = self.steps.write().unwrap();
        let InnerProgress { steps, durations } = &mut *inner;

        let now = Instant::now();
        push_steps_durations(steps, durations, now, 0);
        steps.clear();
    }

    /// Get the current progress view.
    /// 
    /// This is useful to display the progress to the user.
    /// 
    /// The view shows a list of steps with their current state, the total number of states for each step and the total percentage of completion at the end:
    /// ```json5
    /// {
    ///     "steps": [
    ///         {
    ///             "currentStep": "step1", // The name of the step
    ///             "finished": 50, // The number of states that have been completed
    ///             "total": 100 // The total number of states for the step
    ///         },
    ///         {
    ///             "currentStep": "step2",
    ///             "finished": 0,
    ///             "total": 100
    ///         }
    ///     ],
    ///     "percentage": 50.0
    /// }
    /// ```
    pub fn as_progress_view(&self) -> ProgressView {
        let inner = self.steps.read().unwrap();
        let InnerProgress { steps, .. } = &*inner;

        let mut percentage = 0.0;
        let mut prev_factors = 1.0;

        let mut step_view = Vec::with_capacity(steps.len());
        for (_, step, _) in steps.iter() {
            let total = step.total();
            prev_factors *= total as f32;
            percentage += step.current().min(total) as f32 / prev_factors;

            step_view.push(ProgressStepView {
                current_step: step.name(),
                finished: step.current(),
                total: step.total(),
            });
        }

        ProgressView {
            steps: step_view,
            percentage: percentage * 100.0,
        }
    }

    /// Get the accumulated durations of each steps.
    /// 
    /// This is useful to see the bottleneck of the process.
    /// 
    /// Returns an ordered map of the step name to the duration:
    /// ```json5
    /// {
    ///     "step1 > step2": "1.23s", // The duration of the step2 within the step1
    ///     "step1": "1.43s", // The total duration of the step1. Here we see that most of the time was spent in step1.
    /// }
    pub fn accumulated_durations(&self) -> IndexMap<String, String> {
        let mut inner = self.steps.write().unwrap();
        let InnerProgress {
            steps, durations, ..
        } = &mut *inner;

        let now = Instant::now();
        push_steps_durations(steps, durations, now, 0);

        durations
            .drain(..)
            .map(|(name, duration)| (name, format!("{duration:.2?}")))
            .collect()
    }
}

/// Generate the names associated with the durations and push them.
fn push_steps_durations(
    steps: &[(TypeId, Box<dyn Step>, Instant)],
    durations: &mut Vec<(String, Duration)>,
    now: Instant,
    idx: usize,
) {
    for (i, (_, _, started_at)) in steps.iter().skip(idx).enumerate().rev() {
        let full_name = steps
            .iter()
            .take(idx + i + 1)
            .map(|(_, s, _)| s.name())
            .collect::<Vec<_>>()
            .join(" > ");
        durations.push((full_name, now.duration_since(*started_at)));
    }
}

/// This trait lets you use the AtomicSubStep defined right below.
/// The name must be a const that never changed but that can't be enforced by the type system because it make the trait non object-safe.
/// By forcing the Default trait + the &'static str we make it harder to miss-use the trait.
pub trait NamedStep: 'static + Send + Sync + Default {
    fn name(&self) -> &'static str;
}

/// Structure to quickly define steps that need very quick, lockless updating of their current step.
/// You can use this struct if:
/// - The name of the step doesn't change
/// - The total number of steps doesn't change
#[derive(Debug, Clone)]
pub struct AtomicSubStep<Name: NamedStep> {
    unit_name: Name,
    current: Arc<AtomicU64>,
    total: u64,
}

impl<Name: NamedStep> AtomicSubStep<Name> {
    pub fn new(total: u64) -> (Arc<AtomicU64>, Self) {
        let current = Arc::new(AtomicU64::new(0));
        (
            current.clone(),
            Self {
                current,
                total,
                unit_name: Name::default(),
            },
        )
    }
}

impl<Name: NamedStep> Step for AtomicSubStep<Name> {
    fn name(&self) -> Cow<'static, str> {
        self.unit_name.name().into()
    }

    fn current(&self) -> u64 {
        self.current.load(Ordering::Relaxed)
    }

    fn total(&self) -> u64 {
        self.total
    }
}

#[doc(hidden)]
pub use convert_case as _private_convert_case;

/// Helper to create a new enum that implements the `Step` trait.
/// It's useful when we're just going to move from one state to another.
///
/// ```rust
/// steppe::make_enum_progress! {
///     pub enum CustomMainSteps {
///         TheFirstStep,
///         TheSecondWeNeverSee,
///         TheThirdStep,
///         TheFinalStep,
///     }
/// }
/// ```
/// Warning: Even though the syntax looks like a rust enum, it's very case sensitive.
///     All the variants unit, named in CamelCase, and finished by a comma.
#[macro_export]
macro_rules! make_enum_progress {
    ($visibility:vis enum $name:ident { $($variant:ident,)+ }) => {
        #[repr(u8)]
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        #[allow(clippy::enum_variant_names)]
        $visibility enum $name {
            $($variant),+
        }

        impl $crate::Step for $name {
            fn name(&self) -> std::borrow::Cow<'static, str> {
                use $crate::_private_convert_case::Casing;

                match self {
                    $(
                        $name::$variant => stringify!($variant).from_case($crate::_private_convert_case::Case::Camel).to_case($crate::_private_convert_case::Case::Lower).into()
                    ),+
                }
            }

            fn current(&self) -> u64 {
                *self as u64
            }

            fn total(&self) -> u64 {
                use $crate::_internal_count;
                $crate::_internal_count!($($variant)+) as u64
            }
        }
    };
}


#[doc(hidden)]
#[macro_export]
macro_rules! _internal_count {
    () => (0u64);
    ( $x:ident ) => (1u64);
    ( $x:ident $($xs:ident)* ) => (1u64 + $crate::_internal_count!($($xs)*));
}

/// This macro is used to create a new atomic progress step quickly.
/// ```rust
/// steppe::make_atomic_progress!(Document alias AtomicDocumentStep => "document");
/// ```
///
/// This will create a new struct `Document` that implements the `NamedStep` trait and a new type `AtomicDocumentStep` that implements the `Step` trait.
///
/// The `AtomicDocumentStep` type can be used to create a new atomic progress step.
#[macro_export]
macro_rules! make_atomic_progress {
    ($struct_name:ident alias $atomic_struct_name:ident => $step_name:literal) => {
        #[derive(Default, Debug, Clone, Copy)]
        pub struct $struct_name {}
        impl $crate::NamedStep for $struct_name {
            fn name(&self) -> &'static str {
                $step_name
            }
        }
        pub type $atomic_struct_name = $crate::AtomicSubStep<$struct_name>;
    };
}

/// The returned view of the progress.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProgressView {
    pub steps: Vec<ProgressStepView>,
    pub percentage: f32,
}

/// The view of the individual steps.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProgressStepView {
    pub current_step: Cow<'static, str>,
    pub finished: u64,
    pub total: u64,
}

/// Used when the name can change but it's still the same step.
/// To avoid conflicts on the `TypeId`, create a unique type every time you use this step:
/// ```text
/// enum UpgradeVersion {}
///
/// progress.update(VariableNameStep::<UpgradeVersion>::new(
///     "v1 to v2",
///     0,
///     10,
/// ));
/// ```
#[derive(Debug, Clone)]
pub struct VariableNameStep<U: Send + Sync + 'static> {
    name: String,
    current: u64,
    total: u64,
    phantom: PhantomData<U>,
}

impl<U: Send + Sync + 'static> VariableNameStep<U> {
    pub fn new(name: impl Into<String>, current: u64, total: u64) -> Self {
        Self {
            name: name.into(),
            current,
            total,
            phantom: PhantomData,
        }
    }
}

impl<U: Send + Sync + 'static> Step for VariableNameStep<U> {
    fn name(&self) -> Cow<'static, str> {
        self.name.clone().into()
    }

    fn current(&self) -> u64 {
        self.current
    }

    fn total(&self) -> u64 {
        self.total
    }
}
