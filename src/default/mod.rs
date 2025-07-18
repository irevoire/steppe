mod view;

use std::{
    any::TypeId,
    sync::{Arc, RwLock},
};

use crate::{Progress, Step};
pub use view::{ProgressStepView, ProgressView, StepDuration};

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

struct InnerProgress {
    /// The hierarchy of steps.
    steps: Vec<InnerStep>,
    /// The durations associated to each steps.
    durations: Vec<(String, jiff::SignedDuration, jiff::SignedDuration)>,
    /// The time at which the progress was finished.
    finished_at: Option<jiff::Timestamp>,
    /// The time at which the progress was created.
    start_time: jiff::Timestamp,
}

struct InnerStep {
    type_id: TypeId,
    step: Box<dyn Step>,
    started_at: jiff::Timestamp,
    time_spent_in_children: jiff::SignedDuration,
}

impl Default for InnerProgress {
    fn default() -> Self {
        Self {
            steps: vec![],
            durations: vec![],
            finished_at: None,
            start_time: jiff::Timestamp::now(),
        }
    }
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
        let InnerProgress {
            steps,
            durations,
            finished_at: _,
            start_time: _,
        } = &mut *inner;

        let now = jiff::Timestamp::now();
        let step_type = TypeId::of::<P>();
        if let Some(idx) = steps.iter().position(|step| step.type_id == step_type) {
            push_steps_durations(steps, durations, now, idx);
            steps.truncate(idx);
        }

        steps.push(InnerStep {
            type_id: step_type,
            step: Box::new(sub_progress),
            started_at: now,
            time_spent_in_children: jiff::SignedDuration::ZERO,
        });
    }

    /// Drop all the steps and update the durations.
    ///
    /// This is not mandatory. But if you don't do it and take a lot of time before calling [`Progress::accumulated_durations`] the last step will appear as taking more time than it actually did.
    /// Directly calling [`Progress::accumulated_durations`] instead of `finish` will give the same result.
    pub fn finish(&self) {
        let mut inner = self.steps.write().unwrap();
        let InnerProgress {
            steps,
            durations,
            finished_at,
            start_time: _,
        } = &mut *inner;

        if finished_at.is_some() {
            return;
        }

        let now = jiff::Timestamp::now();
        *finished_at = Some(now);
        push_steps_durations(steps, durations, now, 0);
        steps.clear();
    }

    pub fn is_finished(&self) -> bool {
        let inner = self.steps.read().unwrap();
        inner.finished_at.is_some()
    }
}

/// Generate the names associated with the durations and push them.
fn push_steps_durations(
    steps: &[InnerStep],
    durations: &mut Vec<(String, jiff::SignedDuration, jiff::SignedDuration)>,
    now: jiff::Timestamp,
    idx: usize,
) {
    let mut father_duration: Option<jiff::SignedDuration> = None;

    for (i, step) in steps.iter().skip(idx).enumerate().rev() {
        let full_name = steps
            .iter()
            .take(idx + i + 1)
            .map(|step| step.step.name())
            .collect::<Vec<_>>()
            .join(" > ");
        let total_duration = now.duration_since(step.started_at);
        let self_duration = match father_duration {
            Some(father) => total_duration - father,
            None => total_duration,
        };
        durations.push((full_name, total_duration, self_duration));
        father_duration = Some(total_duration);
    }
}
