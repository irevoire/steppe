use std::{
    borrow::Cow,
    marker::PhantomData,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use crate::Step;

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
