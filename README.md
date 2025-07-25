## Steppe
This crate is used to track the progress of a task through multiple steps composed of multiple states.

The objectives are:
- Have a very simple API to describe the steps composing a task. (create the steps and update the progress)
- Provide an easy way to display the current progress while the process is running.
- Provide a way to get the accumulated durations of each steps to quickly see the bottleneck.
- Don't slow down the main process too much.
The crate is composed of only 3 parts:
- The [`Progress`] trait that is used to describe the progress of a task, that's what library should accept in parameter.
- The [`default::DefaultProgress`] struct that is used to track the progress of the task and display it on the tty or returned in an API.
- The [`Step`] trait that is used to describe the steps composing a task.
The [`default::DefaultProgress`] struct is thread-safe, can be cloned cheaply and shared everywhere. While a thread is updating it another can display what we're doing.
The [`Step`] trait is used to describe the steps composing a task.
The API of the [`default::DefaultProgress`] is made of three parts:
- Add something to the stack of steps being processed with the [`default::DefaultProgress::update`] method. It accepts any type that implements the [`Step`] trait.
- Get the current progress view with the [`default::DefaultProgress::as_progress_view`] method.
- Get the accumulated durations of each steps with the [`default::DefaultProgress::accumulated_durations`] method.

There is also a [`Progress`] trait that your library should accept in parameter in case a client wants to use a custom progress implementation.
Since creating [`Step`]s is a bit tedious, you can use the following helpers:
- [`make_enum_progress`] macro.
- [`make_atomic_progress`] macro.
- Or implement the [`NamedStep`] trait.
```rust
use std::sync::atomic::Ordering;
use steppe::{make_enum_progress, make_atomic_progress, Progress, Step, NamedStep, AtomicSubStep};
// This will create a new enum that implements the `Step` trait automatically. Take care it's very case sensitive.
make_enum_progress! {
    pub enum TamosDay {
        PetTheDog,
        WalkTheDog,
        TypeALotOnTheKeyboard,
        WalkTheDogAgain,
    }
}
// This create a new struct that implement the `Step` trait automatically.
// It's displayed as "key strokes" and we cannot change its name.
make_atomic_progress!(KeyStrokes alias AtomicKeyStrokesStep => "key strokes");
let mut progress = steppe::default::DefaultProgress::default();
progress.update(TamosDay::PetTheDog); // We're at 0/4 and 0% of completion
progress.update(TamosDay::WalkTheDog); // We're at 1/4 and 25% of completion
progress.update(TamosDay::TypeALotOnTheKeyboard); // We're at 2/4 and 50% of completion
let (atomic, key_strokes) = AtomicKeyStrokesStep::new(1000);
progress.update(key_strokes);
// Here we enqueued a new step that have 1000 total states. Since we don't want to take a lock everytime
// we type on the keyboard we're instead going to increase an atomic without taking the mutex.
atomic.fetch_add(500, Ordering::Relaxed);
// If we fetch the progress at this point it should be exactly between 50% and 75%.
progress.update(TamosDay::WalkTheDogAgain); // We're at 3/4 and 75% of completion
// By enqueuing this new step the progress is going to drop everything that was pushed after the `TamosDay` type was pushed.
```
