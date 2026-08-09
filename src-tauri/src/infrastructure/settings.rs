use std::{
    fs::{self, File},
    io::Write,
    path::PathBuf,
};

use crate::{
    application::{LoadPreferences, PreferencesRepository},
    domain::AppPreferences,
};

pub(crate) struct FilePreferencesRepository {
    path: PathBuf,
    initial_warning: Option<String>,
}

impl FilePreferencesRepository {
    pub(crate) fn new(path: PathBuf) -> Self {
        let initial_warning = path.parent().and_then(|parent| {
            fs::create_dir_all(parent)
                .err()
                .map(|error| format!("Settings directory is unavailable: {error}"))
        });
        Self {
            path,
            initial_warning,
        }
    }
}

impl PreferencesRepository for FilePreferencesRepository {
    fn load(&self) -> LoadPreferences {
        let mut warnings = self.initial_warning.iter().cloned().collect::<Vec<_>>();
        if self.initial_warning.is_some() {
            return LoadPreferences {
                preferences: AppPreferences::default(),
                warnings,
                persistent: false,
            };
        }
        if !self.path.exists() {
            return LoadPreferences {
                preferences: AppPreferences::default(),
                warnings,
                persistent: true,
            };
        }
        let preferences = fs::read_to_string(&self.path)
            .map_err(|error| error.to_string())
            .and_then(|content| {
                serde_json::from_str::<AppPreferences>(&content).map_err(|error| error.to_string())
            })
            .and_then(AppPreferences::validate);
        match preferences {
            Ok(preferences) => LoadPreferences {
                preferences,
                warnings,
                persistent: true,
            },
            Err(error) => {
                warnings.push(format!(
                    "Stored preferences were invalid and defaults were restored: {error}"
                ));
                LoadPreferences {
                    preferences: AppPreferences::default(),
                    warnings,
                    persistent: true,
                }
            }
        }
    }

    fn save(&self, preferences: &AppPreferences) -> Result<(), String> {
        if let Some(warning) = &self.initial_warning {
            return Err(warning.clone());
        }
        let payload = serde_json::to_vec_pretty(preferences).map_err(|error| error.to_string())?;
        let mut file = File::create(&self.path).map_err(|error| error.to_string())?;
        file.write_all(&payload)
            .map_err(|error| error.to_string())?;
        file.write_all(b"\n").map_err(|error| error.to_string())?;
        file.sync_all().map_err(|error| error.to_string())
    }
}

pub(crate) struct SessionPreferencesRepository {
    warning: String,
}

impl SessionPreferencesRepository {
    pub(crate) fn new(warning: impl Into<String>) -> Self {
        Self {
            warning: warning.into(),
        }
    }
}

impl PreferencesRepository for SessionPreferencesRepository {
    fn load(&self) -> LoadPreferences {
        LoadPreferences {
            preferences: AppPreferences::default(),
            warnings: vec![self.warning.clone()],
            persistent: false,
        }
    }

    fn save(&self, _preferences: &AppPreferences) -> Result<(), String> {
        Err(self.warning.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corrupt_preferences_fall_back_without_overwriting_on_load() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.v1.json");
        fs::write(&path, "{not-json").unwrap();
        let repository = FilePreferencesRepository::new(path.clone());
        let loaded = repository.load();
        assert_eq!(loaded.preferences, AppPreferences::default());
        assert_eq!(fs::read_to_string(path).unwrap(), "{not-json");
        assert_eq!(loaded.warnings.len(), 1);
    }

    #[test]
    fn preferences_round_trip_through_the_versioned_file() {
        let directory = tempfile::tempdir().unwrap();
        let repository = FilePreferencesRepository::new(directory.path().join("settings.v1.json"));
        let expected = AppPreferences::default();
        repository.save(&expected).unwrap();
        assert_eq!(repository.load().preferences, expected);
    }
}
