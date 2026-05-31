CREATE TABLE transactions (
    block_number BIGINT NOT NULL,
    tx_index BIGINT NOT NULL,
    tx_hash BLOB NOT NULL,
    nonce BIGINT NOT NULL,
    from_address BLOB NOT NULL,
    to_address BLOB,
    value TEXT NOT NULL,
    gas_limit BIGINT NOT NULL,
    gas_used BIGINT NOT NULL,
    effective_gas_price TEXT NOT NULL,
    gas_price TEXT NOT NULL,
    max_fee_per_gas TEXT NOT NULL,
    max_priority_fee_per_gas TEXT NOT NULL,
    transaction_type BIGINT,
    success BOOLEAN NOT NULL,
    signature_hash BLOB,
    signature TEXT
);

CREATE INDEX idx_transactions_block ON transactions (block_number);
CREATE UNIQUE INDEX idx_transactions_hash ON transactions (tx_hash);

CREATE TABLE indexed_blocks (
    block_number INTEGER PRIMARY KEY NOT NULL
);
