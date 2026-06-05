CREATE TABLE transactions (
    block_number BIGINT NOT NULL,
    tx_index BIGINT NOT NULL,
    tx_hash BLOB NOT NULL,
    nonce BIGINT NOT NULL,
    from_address BLOB NOT NULL,
    to_address BLOB,
    value BLOB NOT NULL,
    gas_limit BIGINT NOT NULL,
    gas_used BIGINT NOT NULL,
    effective_gas_price BIGINT NOT NULL,
    gas_price BIGINT NOT NULL,
    max_fee_per_gas BIGINT NOT NULL,
    max_priority_fee_per_gas BIGINT NOT NULL,
    transaction_type BIGINT,
    success BOOLEAN NOT NULL,
    coinbase_transfer BLOB,
    signature_hash BLOB,
    signature TEXT
);

CREATE UNIQUE INDEX idx_transactions_hash ON transactions (tx_hash);

CREATE TABLE blocks (
    block_number INTEGER PRIMARY KEY NOT NULL,
    block_hash BLOB NOT NULL,
    miner BLOB NOT NULL,
    gas_used BIGINT NOT NULL,
    timestamp BIGINT NOT NULL,
    base_fee_per_gas BIGINT
);

CREATE INDEX idx_blocks_timestamp ON blocks (timestamp);

CREATE TABLE logs (
    block_number BIGINT NOT NULL,
    tx_index BIGINT NOT NULL,
    log_index BIGINT NOT NULL,
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
