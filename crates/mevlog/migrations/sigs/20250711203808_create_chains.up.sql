-- Add up migration script here

CREATE TABLE chains (
    id BIGINT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    explorer_url TEXT,
    currency_symbol TEXT NOT NULL,
    chainlink_oracle TEXT,
    uniswap_v2_pool TEXT
);
