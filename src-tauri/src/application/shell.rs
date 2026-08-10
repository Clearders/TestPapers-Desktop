use std::sync::{
    atomic::{AtomicBool, AtomicU32, Ordering},
    Mutex,
};

use crate::domain::{
    AppPreferences, CloseAction, CloseBehavior, CloseDecision, CloseOutcome, ThemePreference,
};

pub(crate) struct LoadPreferences {
    pub(crate) preferences: AppPreferences,
    pub(crate) warnings: Vec<String>,
    pub(crate) persistent: bool,
}

pub(crate) trait PreferencesRepository: Send + Sync {
    fn load(&self) -> LoadPreferences;
    fn save(&self, preferences: &AppPreferences) -> Result<(), String>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ShellSnapshot {
    pub(crate) preferences: AppPreferences,
    pub(crate) close_behavior: CloseBehavior,
    pub(crate) tray_available: bool,
    pub(crate) settings_persistent: bool,
    pub(crate) warnings: Vec<String>,
}

struct RuntimeState {
    preferences: AppPreferences,
    tray_available: bool,
    settings_persistent: bool,
    warnings: Vec<String>,
    pending_close: Option<u32>,
}

pub(crate) struct ShellApplication {
    repository: Box<dyn PreferencesRepository>,
    state: Mutex<RuntimeState>,
    next_close_request: AtomicU32,
    quitting: AtomicBool,
    cleaned_up: AtomicBool,
}

impl ShellApplication {
    pub(crate) fn new(repository: Box<dyn PreferencesRepository>) -> Self {
        let loaded = repository.load();
        Self {
            repository,
            state: Mutex::new(RuntimeState {
                preferences: loaded.preferences,
                tray_available: false,
                settings_persistent: loaded.persistent,
                warnings: loaded.warnings,
                pending_close: None,
            }),
            next_close_request: AtomicU32::new(1),
            quitting: AtomicBool::new(false),
            cleaned_up: AtomicBool::new(false),
        }
    }

    pub(crate) fn snapshot(&self) -> ShellSnapshot {
        let state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let close_behavior =
            if state.preferences.close_behavior == CloseBehavior::Tray && !state.tray_available {
                CloseBehavior::Ask
            } else {
                state.preferences.close_behavior
            };
        ShellSnapshot {
            preferences: state.preferences.clone(),
            close_behavior,
            tray_available: state.tray_available,
            settings_persistent: state.settings_persistent,
            warnings: state.warnings.clone(),
        }
    }

    pub(crate) fn set_tray_available(&self, available: bool, warning: Option<String>) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.tray_available = available;
        if let Some(warning) = warning {
            push_unique(&mut state.warnings, warning);
        }
    }

    pub(crate) fn add_warning(&self, warning: impl Into<String>) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        push_unique(&mut state.warnings, warning.into());
    }

    pub(crate) fn set_theme_preference(&self, preference: ThemePreference) -> Result<(), String> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.preferences.theme = preference;
        self.persist(&mut state)
    }

    pub(crate) fn set_close_behavior(&self, behavior: CloseBehavior) -> Result<(), String> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if behavior == CloseBehavior::Tray && !state.tray_available {
            return Err("The system tray is unavailable in this desktop session".into());
        }
        state.preferences.close_behavior = behavior;
        self.persist(&mut state)
    }

    pub(crate) fn begin_close(&self) -> CloseAction {
        if self.quitting.load(Ordering::SeqCst) {
            return CloseAction::Exit;
        }
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if state.pending_close.is_some() {
            return CloseAction::Ignore;
        }
        match state.preferences.close_behavior {
            CloseBehavior::Quit => {
                self.quitting.store(true, Ordering::SeqCst);
                CloseAction::Exit
            }
            CloseBehavior::Tray if state.tray_available => CloseAction::Hide,
            CloseBehavior::Ask | CloseBehavior::Tray => {
                let request_id = self.next_close_request.fetch_add(1, Ordering::SeqCst);
                state.pending_close = Some(request_id);
                CloseAction::Prompt(request_id)
            }
        }
    }

    pub(crate) fn resolve_close(
        &self,
        request_id: u32,
        decision: CloseDecision,
    ) -> Result<CloseOutcome, String> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if state.pending_close != Some(request_id) {
            return Err("The close request is no longer active".into());
        }
        if decision == CloseDecision::Tray && !state.tray_available {
            return Err("The system tray is unavailable in this desktop session".into());
        }
        state.pending_close = None;
        match decision {
            CloseDecision::Cancel => Ok(CloseOutcome::Cancelled),
            CloseDecision::Tray => {
                state.preferences.close_behavior = CloseBehavior::Tray;
                self.persist(&mut state)?;
                Ok(CloseOutcome::Hiding)
            }
            CloseDecision::Quit => {
                state.preferences.close_behavior = CloseBehavior::Quit;
                self.persist(&mut state)?;
                self.quitting.store(true, Ordering::SeqCst);
                Ok(CloseOutcome::Exiting)
            }
        }
    }

    pub(crate) fn request_explicit_quit(&self) {
        self.quitting.store(true, Ordering::SeqCst);
    }

    pub(crate) fn cleanup(&self) -> bool {
        if self.cleaned_up.swap(true, Ordering::SeqCst) {
            return false;
        }
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Err(error) = self.repository.save(&state.preferences) {
            state.settings_persistent = false;
            push_unique(
                &mut state.warnings,
                format!("Settings could not be flushed during exit: {error}"),
            );
        }
        true
    }

    fn persist(&self, state: &mut RuntimeState) -> Result<(), String> {
        if let Err(error) = self.repository.save(&state.preferences) {
            state.settings_persistent = false;
            let warning = format!("Settings are available for this session only: {error}");
            push_unique(&mut state.warnings, warning);
        }
        Ok(())
    }
}

fn push_unique(items: &mut Vec<String>, item: String) {
    if !items.contains(&item) {
        items.push(item);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    struct MemoryPreferences {
        load: AppPreferences,
        saved: Mutex<Vec<AppPreferences>>,
        fail_save: bool,
    }

    impl PreferencesRepository for MemoryPreferences {
        fn load(&self) -> LoadPreferences {
            LoadPreferences {
                preferences: self.load.clone(),
                warnings: Vec::new(),
                persistent: true,
            }
        }

        fn save(&self, preferences: &AppPreferences) -> Result<(), String> {
            if self.fail_save {
                return Err("read-only test store".into());
            }
            self.saved
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .push(preferences.clone());
            Ok(())
        }
    }

    fn application(preferences: AppPreferences) -> ShellApplication {
        ShellApplication::new(Box::new(MemoryPreferences {
            load: preferences,
            saved: Mutex::new(Vec::new()),
            fail_save: false,
        }))
    }

    #[test]
    fn first_close_prompts_and_rejects_stale_responses() {
        let app = application(AppPreferences::default());
        app.set_tray_available(true, None);
        assert_eq!(app.begin_close(), CloseAction::Prompt(1));
        assert_eq!(app.begin_close(), CloseAction::Ignore);
        assert!(app.resolve_close(2, CloseDecision::Cancel).is_err());
        assert_eq!(
            app.resolve_close(1, CloseDecision::Cancel),
            Ok(CloseOutcome::Cancelled)
        );
        assert_eq!(app.begin_close(), CloseAction::Prompt(2));
    }

    #[test]
    fn tray_choice_is_persisted_and_becomes_the_next_close_action() {
        let app = application(AppPreferences::default());
        app.set_tray_available(true, None);
        assert_eq!(app.begin_close(), CloseAction::Prompt(1));
        assert_eq!(
            app.resolve_close(1, CloseDecision::Tray),
            Ok(CloseOutcome::Hiding)
        );
        assert_eq!(app.snapshot().close_behavior, CloseBehavior::Tray);
        assert_eq!(app.begin_close(), CloseAction::Hide);
    }

    #[test]
    fn unavailable_tray_degrades_a_saved_tray_preference_to_ask() {
        let app = application(AppPreferences {
            close_behavior: CloseBehavior::Tray,
            ..AppPreferences::default()
        });
        assert_eq!(app.snapshot().close_behavior, CloseBehavior::Ask);
        assert_eq!(app.begin_close(), CloseAction::Prompt(1));
        assert!(app.resolve_close(1, CloseDecision::Tray).is_err());
    }

    #[test]
    fn cleanup_is_idempotent() {
        let app = application(AppPreferences::default());
        assert!(app.cleanup());
        assert!(!app.cleanup());
    }

    #[test]
    fn unwritable_preferences_degrade_to_session_state() {
        let app = ShellApplication::new(Box::new(MemoryPreferences {
            load: AppPreferences::default(),
            saved: Mutex::new(Vec::new()),
            fail_save: true,
        }));
        assert_eq!(app.set_theme_preference(ThemePreference::Dark), Ok(()));
        let snapshot = app.snapshot();
        assert_eq!(snapshot.preferences.theme, ThemePreference::Dark);
        assert!(!snapshot.settings_persistent);
        assert_eq!(snapshot.warnings.len(), 1);
    }
}
