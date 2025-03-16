-- Add up migration script here

CREATE TABLE methods (
    signature_hash BLOB NOT NULL,
    signature TEXT NOT NULL
);

CREATE INDEX methods_signature_hash_index ON methods (signature_hash);
