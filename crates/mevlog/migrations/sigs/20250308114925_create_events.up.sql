-- Add up migration script here

CREATE TABLE events (
    signature_hash BLOB NOT NULL,
    signature TEXT NOT NULL
);
