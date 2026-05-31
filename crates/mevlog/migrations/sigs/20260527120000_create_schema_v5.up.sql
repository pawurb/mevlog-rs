CREATE TABLE methods (
    signature_hash_4 BLOB NOT NULL,
    signature TEXT NOT NULL
);

CREATE TABLE events (
    signature_hash_32 BLOB NOT NULL,
    signature TEXT NOT NULL
);

CREATE TABLE chains (
    id BIGINT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    explorer_url TEXT,
    currency_symbol TEXT NOT NULL,
    chainlink_oracle TEXT,
    uniswap_v2_pool TEXT
);
