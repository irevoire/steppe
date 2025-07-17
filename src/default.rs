
use indexmap::IndexMap;
use std::{any::TypeId, sync::{Arc, RwLock}, time::{Duration, Instant}};

use crate::{view::{ProgressStepView, ProgressView}, Progress, Step};

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
        let idx = 0;
        for (i, (_, _, started_at)) in steps.iter().skip(idx).enumerate().rev() {
            let full_name = steps
                .iter()
                .take(idx + i + 1)
                .map(|(_, s, _)| s.name())
                .collect::<Vec<_>>()
                .join(" > ");
            durations.push((full_name, now.duration_since(*started_at)));
        }

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
