-- Add up migration script here

CREATE TABLE events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    signature TEXT NOT NULL,
    signature_hash TEXT NOT NULL
);

CREATE INDEX events_signature_hash_index ON events (signature_hash);
