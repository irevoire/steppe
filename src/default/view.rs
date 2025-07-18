use std::borrow::Cow;

use indexmap::IndexMap;
use serde::Serialize;

use super::{DefaultProgress, InnerProgress};

/// The returned view of the progress.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProgressView {
    pub steps: Vec<ProgressStepView>,
    pub percentage: f32,
    #[serde(serialize_with = "jiff::fmt::serde::duration::friendly::compact::required")]
    pub duration: jiff::SignedDuration,
}

/// The view of the individual steps.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProgressStepView {
    pub current_step: Cow<'static, str>,
    pub finished: u64,
    pub total: u64,
    pub percentage: f32,
    #[serde(serialize_with = "jiff::fmt::serde::duration::friendly::compact::required")]
    pub duration: jiff::SignedDuration,
}

impl DefaultProgress {
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

        let mut global_percentage = 0.0;
        let mut prev_factors = 1.0;
        let now = jiff::Timestamp::now();

        let mut step_view = Vec::with_capacity(steps.len());
        for (_, step, started_at) in steps.iter() {
            let total = step.total();
            let current = step.current().min(total) as f32;
            prev_factors *= total as f32;
            global_percentage += current / prev_factors;

            step_view.push(ProgressStepView {
                current_step: step.name(),
                finished: step.current(),
                total: step.total(),
                percentage: current / total as f32 * 100.0,
                duration: now.duration_since(*started_at),
            });
        }

        ProgressView {
            steps: step_view,
            percentage: global_percentage * 100.0,
            duration: now.duration_since(inner.start_time),
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

        let now = jiff::Timestamp::now();
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
            .iter()
            .map(|(name, duration)| (name.to_string(), format!("{duration:.2?}")))
            .collect()
    }

    /// Helper to follow the progression on a tty.
    /// Starts a new screen that:
    /// - Refresh the screen every 100ms.
    /// - Display the progress view while the progress is not finished => It will overwrite itself so if you must print other stuff at the same time it might not come out nice :s
    /// - Display the accumulated durations of each steps once the progress is finished and exit the thread.
    ///
    pub fn follow_progression_on_tty(&self) {
        let this = self.clone();
        std::thread::spawn(move || {
            let refresh_rate = jiff::SignedDuration::from_millis(100);
            let mut last_print = jiff::Timestamp::now();
            let mut lines_of_last_print = 0;
            const CTRL: &str = "\x1b[";
            const UP: &str = "A";
            const CLEAR_LINE: &str = "2K";

            while !this.is_finished() {
                let now = jiff::Timestamp::now();
                if now.duration_since(last_print) > refresh_rate {
                    last_print = now;
                    for _ in 0..lines_of_last_print {
                        print!("{CTRL}{UP}{CTRL}{CLEAR_LINE}");
                    }
                    let view = this.as_progress_view();
                    let json = serde_json::to_string_pretty(&view).unwrap();
                    println!("{}", json);
                    lines_of_last_print = json.lines().count();
                }
            }

            let durations = this.accumulated_durations();
            for (name, duration) in durations {
                println!("{name}: {duration}");
            }
        });
    }
}
