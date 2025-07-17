use std::borrow::Cow;

use serde::Serialize;

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
