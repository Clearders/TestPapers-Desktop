# Desktop local-data migrations

Migrations are immutable, ordered SQL resources embedded by the Rust Local Data
module. The migration runner applies them to a SQLite Backup API staging copy,
validates integrity and foreign keys, and only then swaps the database file.

`0001_local_data.sql` establishes every accepted CLE-15 entity projection and
the CLE-25/CLE-26 question, paper-item, history, neutral pending-mutation, FTS5,
and content-addressed attachment foundations. Future migrations must be
additive files; never edit a migration that has shipped.
