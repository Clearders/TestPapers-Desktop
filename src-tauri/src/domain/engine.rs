use serde::Serialize;

pub(crate) const ENGINE_SCHEMA_VERSION: u8 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum EngineState {
    Starting,
    Ready,
    Recovering,
    Degraded,
    Stopping,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum SuggestedAction {
    Retry,
    #[allow(dead_code)] // Reserved by the v1 wire contract for restore preflight failures.
    Restore,
    ChooseDirectory,
    RestartApp,
    ContactSupport,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct EngineFailure {
    pub(crate) code: &'static str,
    pub(crate) message: String,
    pub(crate) recoverable: bool,
    pub(crate) suggested_action: SuggestedAction,
}

impl EngineFailure {
    pub(crate) fn new(
        code: &'static str,
        message: impl Into<String>,
        recoverable: bool,
        suggested_action: SuggestedAction,
    ) -> Self {
        Self {
            code,
            message: message.into(),
            recoverable,
            suggested_action,
        }
    }

    pub(crate) fn retry_not_available(state: EngineState) -> Self {
        let (message, action) = match state {
            EngineState::Ready => (
                "The Local Engine is already available",
                SuggestedAction::ContactSupport,
            ),
            EngineState::Starting | EngineState::Recovering => (
                "The Local Engine is already starting",
                SuggestedAction::Retry,
            ),
            EngineState::Stopping => ("The Local Engine is stopping", SuggestedAction::RestartApp),
            EngineState::Degraded => (
                "The Local Engine could not be restarted",
                SuggestedAction::Retry,
            ),
        };
        Self::new("engine_retry_not_available", message, false, action)
    }
}
