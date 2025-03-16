-- Add up migration script here

CREATE TABLE events (
    signature_hash BLOB NOT NULL,
    signature TEXT NOT NULL
);

CREATE INDEX events_signature_hash_index ON events (signature_hash);
