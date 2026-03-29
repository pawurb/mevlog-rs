-- Add up migration script here

CREATE TABLE methods (
    signature_hash BLOB NOT NULL,
    signature TEXT NOT NULL
);
