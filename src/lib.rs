#![doc = include_str!("../README.md")]

#[cfg(feature = "default-progress")]
pub mod default;
mod helper;
pub use helper::{AtomicSubStep, NamedStep, VariableNameStep};

use std::borrow::Cow;

#[doc(hidden)]
pub use convert_case as _private_convert_case;

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

/// The main trait of the crate. It describes the progress of a task.
/// As a library you should take this trait in parameter to let the client choose the progress implementation.
/// When writing your test you can use the [`NoProgress`] struct to avoid having to import the default-feature..
pub trait Progress: 'static + Send + Sync {
    fn update(&self, sub_progress: impl Step);
}

/// A progress that does nothing.
///
/// This is useful when you want to disable the progress display or to write tests as a lib.
pub struct NoProgress;

impl Progress for NoProgress {
    fn update(&self, _sub_progress: impl Step) {}
}
