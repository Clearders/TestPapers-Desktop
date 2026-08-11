use serde::Serialize;

use crate::{
    application::{EngineSnapshot, ShellSnapshot},
    domain::{
        CloseBehavior, CloseOutcome, EffectiveTheme, EngineFailure, EngineState, SuggestedAction,
        ThemePreference, ENGINE_SCHEMA_VERSION, SHELL_SCHEMA_VERSION,
    },
};

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EngineErrorV1 {
    schema_version: u8,
    code: &'static str,
    message: String,
    recoverable: bool,
    suggested_action: SuggestedAction,
}

impl From<EngineFailure> for EngineErrorV1 {
    fn from(failure: EngineFailure) -> Self {
        Self {
            schema_version: ENGINE_SCHEMA_VERSION,
            code: failure.code,
            message: failure.message,
            recoverable: failure.recoverable,
            suggested_action: failure.suggested_action,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EngineContextV1 {
    schema_version: u8,
    state: EngineState,
    generation: u64,
    workspace_id: Option<String>,
    database_available: bool,
    maintenance_mode: bool,
    last_error: Option<EngineErrorV1>,
}

impl From<EngineSnapshot> for EngineContextV1 {
    fn from(snapshot: EngineSnapshot) -> Self {
        Self {
            schema_version: ENGINE_SCHEMA_VERSION,
            state: snapshot.state,
            generation: snapshot.generation,
            workspace_id: snapshot.workspace_id,
            database_available: snapshot.database_available,
            maintenance_mode: snapshot.maintenance_mode,
            last_error: snapshot.last_error.map(EngineErrorV1::from),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ThemeState {
    schema_version: u8,
    preference: ThemePreference,
    effective: EffectiveTheme,
}

impl ThemeState {
    pub(crate) fn new(preference: ThemePreference, effective: EffectiveTheme) -> Self {
        Self {
            schema_version: SHELL_SCHEMA_VERSION,
            preference,
            effective,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct IntegrationStatus {
    tray_available: bool,
    settings_persistent: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ShellContext {
    schema_version: u8,
    app_version: String,
    platform: &'static str,
    theme: ThemeState,
    close_behavior: CloseBehavior,
    integrations: IntegrationStatus,
    warnings: Vec<String>,
}

impl ShellContext {
    pub(crate) fn new(
        app_version: impl Into<String>,
        snapshot: ShellSnapshot,
        effective_theme: EffectiveTheme,
    ) -> Self {
        Self {
            schema_version: SHELL_SCHEMA_VERSION,
            app_version: app_version.into(),
            platform: platform_name(),
            theme: ThemeState::new(snapshot.preferences.theme, effective_theme),
            close_behavior: snapshot.close_behavior,
            integrations: IntegrationStatus {
                tray_available: snapshot.tray_available,
                settings_persistent: snapshot.settings_persistent,
            },
            warnings: snapshot.warnings,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CloseRequestedEvent {
    schema_version: u8,
    request_id: u32,
}

impl CloseRequestedEvent {
    pub(crate) fn new(request_id: u32) -> Self {
        Self {
            schema_version: SHELL_SCHEMA_VERSION,
            request_id,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CloseResolution {
    schema_version: u8,
    outcome: CloseOutcome,
}

impl CloseResolution {
    pub(crate) fn new(outcome: CloseOutcome) -> Self {
        Self {
            schema_version: SHELL_SCHEMA_VERSION,
            outcome,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ShellEvent {
    schema_version: u8,
}

impl Default for ShellEvent {
    fn default() -> Self {
        Self {
            schema_version: SHELL_SCHEMA_VERSION,
        }
    }
}

fn platform_name() -> &'static str {
    match std::env::consts::OS {
        "windows" => "windows",
        "macos" => "macos",
        "linux" => "linux",
        _ => "linux",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{AppPreferences, DialogPreview, DialogPreviewKind};

    #[test]
    fn engine_context_and_errors_use_the_versioned_camel_case_wire_shape() {
        let context = EngineContextV1::from(EngineSnapshot {
            state: EngineState::Degraded,
            generation: 2,
            workspace_id: None,
            database_available: false,
            maintenance_mode: false,
            last_error: Some(EngineFailure::new(
                "workspace_locked",
                "The workspace is already in use",
                true,
                SuggestedAction::RestartApp,
            )),
        });

        let value = serde_json::to_value(context).unwrap();
        assert_eq!(value["schemaVersion"], ENGINE_SCHEMA_VERSION);
        assert_eq!(value["state"], "degraded");
        assert_eq!(value["generation"], 2);
        assert!(value["workspaceId"].is_null());
        assert_eq!(value["databaseAvailable"], false);
        assert_eq!(value["maintenanceMode"], false);
        assert_eq!(value["lastError"]["schemaVersion"], ENGINE_SCHEMA_VERSION);
        assert_eq!(value["lastError"]["code"], "workspace_locked");
        assert_eq!(value["lastError"]["suggestedAction"], "restartApp");
        assert!(value.get("workspace_id").is_none());
    }

    #[test]
    fn context_uses_the_camel_case_versioned_wire_shape() {
        let context = ShellContext::new(
            "0.1.0",
            ShellSnapshot {
                preferences: AppPreferences::default(),
                close_behavior: CloseBehavior::Ask,
                tray_available: true,
                settings_persistent: true,
                warnings: Vec::new(),
            },
            EffectiveTheme::Dark,
        );
        let value = serde_json::to_value(context).unwrap();
        assert_eq!(value["schemaVersion"], 1);
        assert_eq!(value["theme"]["preference"], "system");
        assert_eq!(value["integrations"]["trayAvailable"], true);
        assert!(value.get("settings_path").is_none());
    }

    #[test]
    fn every_shell_dto_is_camel_case_and_versioned() {
        let close = serde_json::to_value(CloseRequestedEvent::new(9)).unwrap();
        assert_eq!(close["schemaVersion"], 1);
        assert_eq!(close["requestId"], 9);

        let preference_event = serde_json::to_value(ShellEvent::default()).unwrap();
        assert_eq!(preference_event["schemaVersion"], 1);

        let resolution =
            serde_json::to_value(CloseResolution::new(CloseOutcome::Cancelled)).unwrap();
        assert_eq!(resolution["schemaVersion"], 1);
        assert_eq!(resolution["outcome"], "cancelled");

        let dialog = serde_json::to_value(DialogPreview::new(
            DialogPreviewKind::PaperDocx,
            vec!["paper.docx".into()],
        ))
        .unwrap();
        assert_eq!(dialog["schemaVersion"], 1);
        assert_eq!(dialog["selectionCount"], 1);
        assert_eq!(dialog["displayNames"][0], "paper.docx");
        assert!(dialog.get("path").is_none());
    }
}
