-- Add up migration script here

CREATE TABLE methods (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    signature TEXT NOT NULL,
    signature_hash TEXT NOT NULL
);

CREATE INDEX methods_signature_hash_index ON methods (signature_hash);
