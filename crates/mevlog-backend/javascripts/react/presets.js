// Single source of truth for the preset SQL queries, shared by the vanilla hero
// demo (javascripts/scripts.js) and the React search form (SearchForm.jsx) so the
// two never drift apart. SearchForm imports this module directly; index.js also
// exposes it on `window.MEVLOG_PRESETS` for the non-bundled hero script.
//
// Each entry:
//   key         - short slug; links a hero tab to /search?q=<key> (auto-runs there)
//   label       - full question shown on the search preset card
//   full_label  - one-sentence explanation of full_sql, shown above the search SQL
//   demo_label  - one-sentence explanation of demo_sql, shown under the hero card (hero entries only)
//   full_sql    - complete query loaded into the search editor and run by /search
//   tab         - hero tab label                  (hero entries only)
//   demo_sql    - trimmed SQL rendered in the hero terminal card (hero entries only)
//   result      - sample { key, val } row shown under the hero SQL (hero entries only)
//
// full_label must describe what full_sql returns; demo_label what the trimmed
// demo_sql returns. They differ because the demo SQL is simplified.
//
// Tables: transactions, logs, blocks. Macros (braces): {LATEST_BLOCK()},
// {NATIVE_TOKEN_PRICE()}, {RESOLVE_ENS("name.eth")}.
const PRESETS = [
  {
    key: 'ens-gas-spend',
    tab: 'ENS gas cost',
    label: 'How much jaredfromsubway.eth spent on gas in last 1 day',
    full_label: 'Total gas the jaredfromsubway.eth MEV bot burned across its transactions in the last day, with tx count in ETH and USD.',
    demo_label: 'Total gas the jaredfromsubway.eth MEV bot burned across its transactions in the last day, in USD.',
    result: { key: 'gas_spent', val: '$48,210.34' },
    demo_sql:
`SELECT format_usd(convert_usd(
         u256_sum(u256_mul(gas_used, effective_gas_price)),
         {NATIVE_TOKEN_PRICE()})) AS gas_spent
FROM transactions
WHERE from_address = {RESOLVE_ENS("jaredfromsubway.eth")}`,
    full_sql:
`SELECT COUNT(*) AS txs,
       format_ether(u256_sum(u256_mul(t.gas_used, t.effective_gas_price))) AS gas_spent_eth,
       format_usd(convert_usd(u256_sum(u256_mul(t.gas_used, t.effective_gas_price)), {NATIVE_TOKEN_PRICE()})) AS gas_spent_usd
FROM transactions t
JOIN blocks b ON b.block_number = t.block_number
WHERE t.from_address = {RESOLVE_ENS("jaredfromsubway.eth")}
  AND b.timestamp >= unixepoch('now', '-1 day')`,
  },
  {
    key: 'usdc-top-txs',
    tab: 'USDC volume',
    label: 'Which 10 txs transferred the most USDC in last 1 day',
    full_label: 'The ten transactions that moved the largest USDC amounts in the last day.',
    demo_label: 'Total USDC volume transferred across all logs in the last day.',
    result: { key: 'usdc_volume', val: '1,284,019,442' },
    demo_sql:
`SELECT erc20_to_real(u256_sum(erc20_amount), 6) AS usdc_volume
FROM logs
WHERE address = X'a0b8...eb48'
  AND erc20_amount IS NOT NULL`,
    full_sql:
`WITH agg AS (
  SELECT block_number, tx_index, u256_sum(erc20_amount) AS amt
  FROM logs
  WHERE address = X'a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48'
    AND erc20_amount IS NOT NULL
    AND block_number >= {LATEST_BLOCK()} - 7200
  GROUP BY block_number, tx_index
  ORDER BY amt DESC
  LIMIT 10
)
SELECT t.tx_hash,
       format_usd(erc20_to_real(agg.amt, 6)) AS usdc
FROM agg
JOIN transactions t
  ON t.block_number = agg.block_number AND t.tx_index = agg.tx_index
ORDER BY agg.amt DESC`,
  },
  {
    key: 'top-gas-txs',
    tab: 'Expensive tx',
    label: 'Which 10 txs spent the most on gas in last 1 day',
    full_label: 'The ten transactions that paid the most for gas in the last day, in ETH and USD.',
    demo_label: 'The single transaction that paid the most for gas, in USD.',
    result: { key: 'gas_usd', val: '$9,418.55' },
    demo_sql:
`SELECT tx_hash,
       format_usd(convert_usd(u256_mul(gas_used,
         effective_gas_price), {NATIVE_TOKEN_PRICE()})) AS gas_usd
FROM transactions
ORDER BY u256_mul(gas_used, effective_gas_price) DESC
LIMIT 1`,
    full_sql:
`SELECT t.block_number, t.tx_hash,
       format_ether(u256_mul(t.gas_used, t.effective_gas_price)) AS gas_eth,
       format_usd(convert_usd(u256_mul(t.gas_used, t.effective_gas_price), {NATIVE_TOKEN_PRICE()})) AS gas_usd
FROM transactions t
JOIN blocks b ON b.block_number = t.block_number
WHERE b.timestamp >= unixepoch('now', '-1 day')
ORDER BY u256_mul(t.gas_used, t.effective_gas_price) DESC
LIMIT 10`,
  },
  {
    key: 'top-eth-transfers',
    tab: 'Top ETH transfer',
    label: 'Top 10 ETH transfers in last 1 day',
    full_label: 'The ten largest native ETH transfers in the last day, in ETH and USD.',
    demo_label: 'The single largest native ETH transfer, in ETH.',
    result: { key: 'value_eth', val: '4,512.337000 ETH' },
    demo_sql:
`SELECT tx_hash,
       format_ether(value) AS value_eth
FROM transactions
ORDER BY value DESC
LIMIT 1`,
    full_sql:
`SELECT t.tx_hash,
       format_ether(t.value) AS value_eth,
       format_usd(convert_usd(t.value, {NATIVE_TOKEN_PRICE()})) AS value_usd
FROM transactions t
JOIN blocks b ON b.block_number = t.block_number
WHERE b.timestamp >= unixepoch('now', '-1 day')
ORDER BY t.value DESC
LIMIT 10`,
  },
  {
    key: 'top-methods',
    tab: 'Top methods',
    label: 'Top 15 most-called methods in last 1 day',
    full_label: 'The fifteen most frequently called contract methods over the last day.',
    demo_label: 'The fifteen most frequently called contract methods.',
    result: { key: 'signature', val: 'transfer()' },
    demo_sql:
`SELECT signature, COUNT(*) AS calls
FROM transactions
WHERE signature IS NOT NULL
GROUP BY signature
ORDER BY calls DESC
LIMIT 15`,
    full_sql:
`SELECT t.signature, COUNT(*) AS calls
FROM transactions t
JOIN blocks b ON b.block_number = t.block_number
WHERE t.signature IS NOT NULL
  AND b.timestamp >= unixepoch('now', '-1 day')
GROUP BY t.signature
ORDER BY calls DESC
LIMIT 15`,
  },
  {
    key: 'method-gas',
    label: 'Which methods burned the most gas in last 1 day',
    full_label: 'Which contract methods burned the most total gas over the last day, with call counts.',
    full_sql:
`SELECT t.signature,
       COUNT(*) AS calls,
       format_ether(u256_sum(u256_mul(t.gas_used, t.effective_gas_price))) AS gas_eth,
       format_usd(convert_usd(u256_sum(u256_mul(t.gas_used, t.effective_gas_price)), {NATIVE_TOKEN_PRICE()})) AS gas_usd
FROM transactions t
JOIN blocks b ON b.block_number = t.block_number
WHERE t.signature IS NOT NULL
  AND b.timestamp >= unixepoch('now', '-1 day')
GROUP BY t.signature
ORDER BY u256_sum(u256_mul(t.gas_used, t.effective_gas_price)) DESC
LIMIT 10`,
  },
  {
    key: 'new-contracts',
    tab: 'New contracts',
    label: 'How many new contracts deployed in last 1 day',
    full_label: 'How many contracts were successfully deployed in the last day.',
    demo_label: 'How many contracts were successfully deployed.',
    result: { key: 'contracts_deployed', val: '1,204' },
    demo_sql:
`SELECT COUNT(*) AS contracts_deployed
FROM transactions
WHERE signature = 'CREATE()'
  AND success = 1`,
    full_sql:
`SELECT COUNT(*) AS contracts_deployed
FROM transactions t
JOIN blocks b ON b.block_number = t.block_number
WHERE t.signature = 'CREATE()'
  AND t.success = 1
  AND b.timestamp >= unixepoch('now', '-1 day')`,
  },
  {
    key: 'top-miners',
    label: 'Top 5 miners by blocks mined in last 1 day',
    full_label: 'The five block producers that mined the most blocks in the last day.',
    full_sql:
`SELECT miner, COUNT(*) AS blocks_mined
FROM blocks
WHERE timestamp >= unixepoch('now', '-1 day')
GROUP BY miner
ORDER BY blocks_mined DESC
LIMIT 5`,
  },
  {
    key: 'db-stats',
    label: 'Get current DB stats info',
    full_label: 'Current local index stats: block range, row counts, missing blocks, and database size.',
    full_sql:
`WITH db AS (
  SELECT (SELECT page_count FROM pragma_page_count()) * (SELECT page_size FROM pragma_page_size()) AS bytes
)
SELECT (SELECT MIN(block_number) FROM blocks) AS min_block,
       datetime((SELECT MIN(timestamp) FROM blocks), 'unixepoch') || ' UTC' AS min_block_time,
       (SELECT MAX(block_number) FROM blocks) AS max_block,
       datetime((SELECT MAX(timestamp) FROM blocks), 'unixepoch') || ' UTC' AS max_block_time,
       (SELECT COUNT(*) FROM blocks) AS total_blocks,
       (SELECT MAX(block_number) - MIN(block_number) + 1 - COUNT(*) FROM blocks) AS missing_blocks,
       (SELECT COUNT(*) FROM logs) AS total_logs,
       (SELECT COUNT(*) FROM transactions) AS total_txs,
       CASE WHEN bytes < 1024 THEN bytes || ' B'
            WHEN bytes < 1048576 THEN printf('%.2f KB', bytes / 1024.0)
            WHEN bytes < 1073741824 THEN printf('%.2f MB', bytes / 1048576.0)
            ELSE printf('%.2f GB', bytes / 1073741824.0) END AS db_size
FROM db`,
  },
];

export default PRESETS;
