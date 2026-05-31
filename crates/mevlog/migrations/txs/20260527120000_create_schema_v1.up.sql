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

CREATE TABLE logs (
    block_number BIGINT NOT NULL,
    tx_index BIGINT NOT NULL,
    log_index BIGINT NOT NULL,
    tx_hash BLOB NOT NULL,
    address BLOB NOT NULL,
    topic0 BLOB,
    topic1 BLOB,
    topic2 BLOB,
    topic3 BLOB,
    data BLOB NOT NULL,
    erc20_amount BLOB,
    signature TEXT,
    PRIMARY KEY (block_number, log_index)
);

CREATE INDEX idx_logs_tx ON logs (block_number, tx_index);
CREATE INDEX idx_logs_address ON logs (address);
CREATE INDEX idx_logs_topic0 ON logs (topic0);
CREATE INDEX idx_logs_erc20_amount ON logs (erc20_amount) WHERE erc20_amount IS NOT NULL;
