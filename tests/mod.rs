use insta::assert_json_snapshot;
use std::sync::atomic::Ordering;
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
fn one_level() {
    let progress = Progress::default();
    progress.update(CustomMainSteps::TheFirstStep);
    assert_json_snapshot!(progress.as_progress_view(), @r#"
    {
      "steps": [
        {
          "currentStep": "the first step",
          "finished": 0,
          "total": 4
        }
      ],
      "percentage": 0.0
    }
    "#);
    progress.update(CustomSubSteps::WeWontGoTooFarThisTime);
    assert_json_snapshot!(progress.as_progress_view(), @r#"
    {
      "steps": [
        {
          "currentStep": "the first step",
          "finished": 0,
          "total": 4
        },
        {
          "currentStep": "we wont go too far this time",
          "finished": 0,
          "total": 3
        }
      ],
      "percentage": 0.0
    }
    "#);
    progress.update(CustomSubSteps::JustOneMore);
    assert_json_snapshot!(progress.as_progress_view(), @r#"
    {
      "steps": [
        {
          "currentStep": "the first step",
          "finished": 0,
          "total": 4
        },
        {
          "currentStep": "just one more",
          "finished": 1,
          "total": 3
        }
      ],
      "percentage": 8.333334
    }
    "#);
    progress.update(CustomSubSteps::WeAreDone);
    let (atomic, unit) = AtomicCustomUnit::new(10);
    atomic.fetch_add(6, Ordering::Relaxed);
    progress.update(unit);
    assert_json_snapshot!(progress.as_progress_view(), @r#"
    {
      "steps": [
        {
          "currentStep": "the first step",
          "finished": 0,
          "total": 4
        },
        {
          "currentStep": "we are done",
          "finished": 2,
          "total": 3
        },
        {
          "currentStep": "custom unit",
          "finished": 6,
          "total": 10
        }
      ],
      "percentage": 21.666666
    }
    "#);
    atomic.fetch_add(3, Ordering::Relaxed);
    assert_json_snapshot!(progress.as_progress_view(), @r#"
    {
      "steps": [
        {
          "currentStep": "the first step",
          "finished": 0,
          "total": 4
        },
        {
          "currentStep": "we are done",
          "finished": 2,
          "total": 3
        },
        {
          "currentStep": "custom unit",
          "finished": 9,
          "total": 10
        }
      ],
      "percentage": 24.166668
    }
    "#);
    // This should delete both the atomic step and the sub step + We're skipping the second step
    progress.update(CustomMainSteps::TheThirdStep);
    assert_json_snapshot!(progress.as_progress_view(), @r#"
    {
      "steps": [
        {
          "currentStep": "the third step",
          "finished": 2,
          "total": 4
        }
      ],
      "percentage": 50.0
    }
    "#);
    let (atomic, unit) = AtomicCustomUnit::new(2);
    // We don't have any check on the max but the percentage should cap itself at the maximum specified value as a "finished" higher than the total means you have a bug
    atomic.fetch_add(1000, Ordering::Relaxed);
    progress.update(unit);
    assert_json_snapshot!(progress.as_progress_view(), @r#"
    {
      "steps": [
        {
          "currentStep": "the third step",
          "finished": 2,
          "total": 4
        },
        {
          "currentStep": "custom unit",
          "finished": 1000,
          "total": 2
        }
      ],
      "percentage": 75.0
    }
    "#);
    // This should delete the atomic step only
    progress.update(CustomMainSteps::TheFinalStep);
    assert_json_snapshot!(progress.as_progress_view(), @r#"
    {
      "steps": [
        {
          "currentStep": "the final step",
          "finished": 3,
          "total": 4
        }
      ],
      "percentage": 75.0
    }
    "#);

    progress.finish();
    assert_json_snapshot!(progress.as_progress_view(), @r#"
    {
      "steps": [],
      "percentage": 0.0
    }
    "#);

    let mut durations = progress.accumulated_durations();
    // sadly we must erase all the values because that would be flaky. But the name and order of the steps should be stable.
    durations.iter_mut().for_each(|(_, v)| *v = "[duration]".to_string());
    assert_json_snapshot!(durations, @r#"
    {
      "the first step > we wont go too far this time": "[duration]",
      "the first step > just one more": "[duration]",
      "the first step > we are done > custom unit": "[duration]",
      "the first step > we are done": "[duration]",
      "the first step": "[duration]",
      "the third step > custom unit": "[duration]",
      "the third step": "[duration]",
      "the final step": "[duration]"
    }
    "#);
}
