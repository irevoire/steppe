use insta::assert_json_snapshot;
use jiff::SignedDuration;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use steppe::default::{DefaultProgress, StepDuration};
use steppe::*;

make_enum_progress! {
    pub enum CustomMainSteps {
        TheFirstStep,
        TheSecondWeNeverSee,
        TheThirdStep,
        TheFinalStep,
    }
}

make_enum_progress! {
    pub enum CustomSubSteps {
        WeWontGoTooFarThisTime,
        JustOneMore,
        WeAreDone,
    }
}

make_atomic_progress!(CustomUnit alias AtomicCustomUnit => "custom unit");

#[test]
fn the_test_tm() {
    let progress = DefaultProgress::default();
    progress.update(CustomMainSteps::TheFirstStep);
    assert_json_snapshot!(progress.as_progress_view(), { ".**.duration" => "[duration]" }, @r#"
    {
      "steps": [
        {
          "currentStep": "the first step",
          "finished": 0,
          "total": 4,
          "percentage": 0.0,
          "duration": "[duration]"
        }
      ],
      "percentage": 0.0,
      "duration": "[duration]"
    }
    "#);
    progress.update(CustomSubSteps::WeWontGoTooFarThisTime);
    assert_json_snapshot!(progress.as_progress_view(), { ".**.duration" => "[duration]" }, @r#"
    {
      "steps": [
        {
          "currentStep": "the first step",
          "finished": 0,
          "total": 4,
          "percentage": 0.0,
          "duration": "[duration]"
        },
        {
          "currentStep": "we wont go too far this time",
          "finished": 0,
          "total": 3,
          "percentage": 0.0,
          "duration": "[duration]"
        }
      ],
      "percentage": 0.0,
      "duration": "[duration]"
    }
    "#);
    progress.update(CustomSubSteps::JustOneMore);
    assert_json_snapshot!(progress.as_progress_view(), { ".**.duration" => "[duration]" }, @r#"
    {
      "steps": [
        {
          "currentStep": "the first step",
          "finished": 0,
          "total": 4,
          "percentage": 0.0,
          "duration": "[duration]"
        },
        {
          "currentStep": "just one more",
          "finished": 1,
          "total": 3,
          "percentage": 33.333336,
          "duration": "[duration]"
        }
      ],
      "percentage": 8.333334,
      "duration": "[duration]"
    }
    "#);
    progress.update(CustomSubSteps::WeAreDone);
    let (atomic, unit) = AtomicCustomUnit::new(10);
    atomic.fetch_add(6, Ordering::Relaxed);
    progress.update(unit);
    assert_json_snapshot!(progress.as_progress_view(), { ".**.duration" => "[duration]" }, @r#"
    {
      "steps": [
        {
          "currentStep": "the first step",
          "finished": 0,
          "total": 4,
          "percentage": 0.0,
          "duration": "[duration]"
        },
        {
          "currentStep": "we are done",
          "finished": 2,
          "total": 3,
          "percentage": 66.66667,
          "duration": "[duration]"
        },
        {
          "currentStep": "custom unit",
          "finished": 6,
          "total": 10,
          "percentage": 60.000004,
          "duration": "[duration]"
        }
      ],
      "percentage": 21.666666,
      "duration": "[duration]"
    }
    "#);
    atomic.fetch_add(3, Ordering::Relaxed);
    assert_json_snapshot!(progress.as_progress_view(), { ".**.duration" => "[duration]" }, @r#"
    {
      "steps": [
        {
          "currentStep": "the first step",
          "finished": 0,
          "total": 4,
          "percentage": 0.0,
          "duration": "[duration]"
        },
        {
          "currentStep": "we are done",
          "finished": 2,
          "total": 3,
          "percentage": 66.66667,
          "duration": "[duration]"
        },
        {
          "currentStep": "custom unit",
          "finished": 9,
          "total": 10,
          "percentage": 90.0,
          "duration": "[duration]"
        }
      ],
      "percentage": 24.166668,
      "duration": "[duration]"
    }
    "#);
    // This should delete both the atomic step and the sub step + We're skipping the second step
    progress.update(CustomMainSteps::TheThirdStep);
    assert_json_snapshot!(progress.as_progress_view(), { ".**.duration" => "[duration]" }, @r#"
    {
      "steps": [
        {
          "currentStep": "the third step",
          "finished": 2,
          "total": 4,
          "percentage": 50.0,
          "duration": "[duration]"
        }
      ],
      "percentage": 50.0,
      "duration": "[duration]"
    }
    "#);
    let (atomic, unit) = AtomicCustomUnit::new(2);
    // We don't have any check on the max but the percentage should cap itself at the maximum specified value as a "finished" higher than the total means you have a bug
    atomic.fetch_add(1000, Ordering::Relaxed);
    progress.update(unit);
    assert_json_snapshot!(progress.as_progress_view(), { ".**.duration" => "[duration]" }, @r#"
    {
      "steps": [
        {
          "currentStep": "the third step",
          "finished": 2,
          "total": 4,
          "percentage": 50.0,
          "duration": "[duration]"
        },
        {
          "currentStep": "custom unit",
          "finished": 2,
          "total": 2,
          "percentage": 100.0,
          "duration": "[duration]"
        }
      ],
      "percentage": 75.0,
      "duration": "[duration]"
    }
    "#);
    // This should delete the atomic step only
    progress.update(CustomMainSteps::TheFinalStep);
    assert_json_snapshot!(progress.as_progress_view(), { ".**.duration" => "[duration]" }, @r#"
    {
      "steps": [
        {
          "currentStep": "the final step",
          "finished": 3,
          "total": 4,
          "percentage": 75.0,
          "duration": "[duration]"
        }
      ],
      "percentage": 75.0,
      "duration": "[duration]"
    }
    "#);

    progress.finish();
    assert_json_snapshot!(progress.as_progress_view(), { ".**.duration" => "[duration]" }, @r#"
    {
      "steps": [],
      "percentage": 0.0,
      "duration": "[duration]"
    }
    "#);

    let mut durations = progress.accumulated_durations();
    // sadly we must erase all the values because that would be flaky. But the name and order of the steps should be stable.
    durations.iter_mut().for_each(|(_, v)| {
        *v = StepDuration {
            total_duration: SignedDuration::ZERO,
            self_duration: SignedDuration::ZERO,
        }
    });
    println!("{:?}", durations);
    assert_json_snapshot!(durations, @r#"
    {
      "the first step > we wont go too far this time": {
        "totalDuration": "0s",
        "selfDuration": "0s"
      },
      "the first step > just one more": {
        "totalDuration": "0s",
        "selfDuration": "0s"
      },
      "the first step > we are done > custom unit": {
        "totalDuration": "0s",
        "selfDuration": "0s"
      },
      "the first step > we are done": {
        "totalDuration": "0s",
        "selfDuration": "0s"
      },
      "the first step": {
        "totalDuration": "0s",
        "selfDuration": "0s"
      },
      "the third step > custom unit": {
        "totalDuration": "0s",
        "selfDuration": "0s"
      },
      "the third step": {
        "totalDuration": "0s",
        "selfDuration": "0s"
      },
      "the final step": {
        "totalDuration": "0s",
        "selfDuration": "0s"
      }
    }
    "#);
}

#[test]
fn using_a_custom_provider() {
    struct CustomProgress {
        updated: Arc<AtomicU64>,
    }

    impl Progress for CustomProgress {
        fn update(&self, _sub_progress: impl Step) {
            self.updated.fetch_add(1, Ordering::Relaxed);
        }
    }

    let progress = CustomProgress {
        updated: Arc::new(AtomicU64::new(0)),
    };
    progress.update(CustomMainSteps::TheFirstStep);
    assert_eq!(progress.updated.load(Ordering::Relaxed), 1);
    progress.update(CustomSubSteps::WeWontGoTooFarThisTime);
    assert_eq!(progress.updated.load(Ordering::Relaxed), 2);
    progress.update(CustomSubSteps::JustOneMore);
    assert_eq!(progress.updated.load(Ordering::Relaxed), 3);
    progress.update(CustomSubSteps::WeAreDone);
    assert_eq!(progress.updated.load(Ordering::Relaxed), 4);
    progress.update(CustomMainSteps::TheThirdStep);
    assert_eq!(progress.updated.load(Ordering::Relaxed), 5);
}
