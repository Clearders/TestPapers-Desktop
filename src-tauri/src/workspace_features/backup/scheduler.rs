use serde::{Deserialize, Serialize};
use std::fmt;

const MIN_INTERVAL_MINUTES: u32 = 60;
const MAX_INTERVAL_MINUTES: u32 = 43_200;
const MIN_RETENTION_DAYS: u32 = 1;
const MAX_RETENTION_DAYS: u32 = 3_650;
const MICROS_PER_MINUTE: i64 = 60 * 1_000_000;
const MICROS_PER_DAY: i64 = 24 * 60 * MICROS_PER_MINUTE;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupScheduleConfig {
    pub(crate) enabled: bool,
    pub(crate) destination_id: Option<String>,
    pub(crate) interval_minutes: u32,
    pub(crate) retention_days: u32,
    pub(crate) encrypted: bool,
    pub(crate) key_id: Option<String>,
    pub(crate) recovery_key_confirmed: bool,
}

impl Default for BackupScheduleConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            destination_id: None,
            interval_minutes: 1_440,
            retention_days: 30,
            encrypted: true,
            key_id: None,
            recovery_key_confirmed: false,
        }
    }
}

impl BackupScheduleConfig {
    pub(crate) fn validate(&self) -> Result<(), ScheduleError> {
        if !(MIN_INTERVAL_MINUTES..=MAX_INTERVAL_MINUTES).contains(&self.interval_minutes) {
            return Err(ScheduleError::IntervalOutOfRange);
        }
        if !(MIN_RETENTION_DAYS..=MAX_RETENTION_DAYS).contains(&self.retention_days) {
            return Err(ScheduleError::RetentionOutOfRange);
        }
        if self.enabled
            && self
                .destination_id
                .as_deref()
                .is_none_or(|value| value.trim().is_empty())
        {
            return Err(ScheduleError::MissingDestination);
        }
        if self.enabled
            && self.encrypted
            && (self
                .key_id
                .as_deref()
                .is_none_or(|value| value.trim().is_empty())
                || !self.recovery_key_confirmed)
        {
            return Err(ScheduleError::RecoveryKeyNotConfirmed);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct AutomaticBackupState {
    pub(crate) last_success_micros: Option<i64>,
    pub(crate) last_attempt_micros: Option<i64>,
    pub(crate) consecutive_failures: u32,
}

impl AutomaticBackupState {
    pub(crate) fn next_due_micros(
        &self,
        now_micros: i64,
        config: &BackupScheduleConfig,
    ) -> Result<Option<i64>, ScheduleError> {
        config.validate()?;
        if !config.enabled {
            return Ok(None);
        }
        let interval = i64::from(config.interval_minutes)
            .checked_mul(MICROS_PER_MINUTE)
            .ok_or(ScheduleError::TimeOverflow)?;
        let regular_due = self
            .last_success_micros
            .and_then(|last| last.checked_add(interval))
            .unwrap_or(now_micros);
        if self.consecutive_failures == 0 {
            return Ok(Some(regular_due));
        }
        let retry_delay = interval.max(60 * MICROS_PER_MINUTE);
        let retry_due = self
            .last_attempt_micros
            .and_then(|last| last.checked_add(retry_delay))
            .unwrap_or(regular_due);
        Ok(Some(regular_due.max(retry_due)))
    }

    pub(crate) fn is_due(
        &self,
        now_micros: i64,
        config: &BackupScheduleConfig,
    ) -> Result<bool, ScheduleError> {
        Ok(self
            .next_due_micros(now_micros, config)?
            .is_some_and(|due| due <= now_micros))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ScheduledBackupCandidate {
    pub(crate) path_id: String,
    pub(crate) workspace_id: String,
    pub(crate) created_at_micros: i64,
    pub(crate) automatic: bool,
    pub(crate) verified: bool,
}

impl ScheduledBackupCandidate {
    pub(crate) fn can_delete_for_retention(
        &self,
        now_micros: i64,
        current_workspace_id: &str,
        retention_days: u32,
    ) -> bool {
        if !self.automatic || !self.verified || self.workspace_id != current_workspace_id {
            return false;
        }
        let Some(cutoff) = now_micros.checked_sub(i64::from(retention_days) * MICROS_PER_DAY)
        else {
            return false;
        };
        self.created_at_micros < cutoff
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ScheduleError {
    IntervalOutOfRange,
    RetentionOutOfRange,
    MissingDestination,
    RecoveryKeyNotConfirmed,
    TimeOverflow,
}

impl fmt::Display for ScheduleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IntervalOutOfRange => {
                formatter.write_str("backup interval must be 60..=43200 minutes")
            }
            Self::RetentionOutOfRange => {
                formatter.write_str("backup retention must be 1..=3650 days")
            }
            Self::MissingDestination => {
                formatter.write_str("automatic backup destination is missing")
            }
            Self::RecoveryKeyNotConfirmed => {
                formatter.write_str("encrypted automatic backup requires a confirmed recovery key")
            }
            Self::TimeOverflow => formatter.write_str("backup schedule time overflowed"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn enabled() -> BackupScheduleConfig {
        BackupScheduleConfig {
            enabled: true,
            destination_id: Some("bookmark-1".into()),
            interval_minutes: 1_440,
            retention_days: 30,
            encrypted: true,
            key_id: Some("backup-key-1".into()),
            recovery_key_confirmed: true,
        }
    }

    #[test]
    fn defaults_are_safe_and_disabled() {
        let config = BackupScheduleConfig::default();
        assert!(!config.enabled);
        assert!(config.encrypted);
        config.validate().unwrap();
    }

    #[test]
    fn failed_backup_never_retries_more_frequently_than_interval() {
        let state = AutomaticBackupState {
            last_success_micros: Some(0),
            last_attempt_micros: Some(10 * MICROS_PER_MINUTE),
            consecutive_failures: 1,
        };
        assert_eq!(
            state
                .next_due_micros(20 * MICROS_PER_MINUTE, &enabled())
                .unwrap(),
            Some((10 + 1_440) * MICROS_PER_MINUTE)
        );
    }

    #[test]
    fn retention_never_selects_manual_foreign_or_unverified_backups() {
        let mut candidate = ScheduledBackupCandidate {
            path_id: "backup-1".into(),
            workspace_id: "workspace-1".into(),
            created_at_micros: 1,
            automatic: true,
            verified: true,
        };
        assert!(candidate.can_delete_for_retention(40 * MICROS_PER_DAY, "workspace-1", 30));
        candidate.automatic = false;
        assert!(!candidate.can_delete_for_retention(40 * MICROS_PER_DAY, "workspace-1", 30));
    }
}
