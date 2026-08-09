use serde::Serialize;

use crate::{
    application::ShellSnapshot,
    domain::{CloseBehavior, CloseOutcome, EffectiveTheme, ThemePreference, SHELL_SCHEMA_VERSION},
};

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
