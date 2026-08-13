//! Cloud synchronization runtime boundary.
//!
//! CLE-91 adds durable queue, cursor, conflict-baseline, and snapshot-rebuild storage behind the
//! Local Engine. This module still contains no network transport or background worker; CLE-89 owns
//! the pull/apply/ack/push runtime that will consume the persistence APIs.
