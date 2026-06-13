import React, { useState, useEffect } from 'react';
import Editor from 'react-simple-code-editor';
import Prism from 'prismjs/components/prism-core';
import 'prismjs/components/prism-sql';
import SchemaReference from './SchemaReference';
import HelpersReference from './HelpersReference';

// Teach Prism's SQL grammar about mevlog's custom helpers and {MACRO()} tokens.
// Guard so the insert runs only once even if the module is evaluated twice.
if (Prism.languages.sql && !Prism.languages.sql['mevlog-macro']) {
  Prism.languages.insertBefore('sql', 'function', {
    'mevlog-macro': {
      // {LATEST_BLOCK()}, {NATIVE_TOKEN_PRICE()}, {RESOLVE_ENS("name.eth")}
      pattern: /\{\s*(?:LATEST_BLOCK|NATIVE_TOKEN_PRICE|RESOLVE_ENS)\s*\([^}]*\)\s*\}/,
      alias: 'mevlog-macro',
      greedy: true,
    },
    'mevlog-function': {
      pattern: /\b(?:u256_sum|u256_mul|u256_add|u256_to_dec|erc20_to_real|convert_usd|format_ether|format_gwei|format_usd)(?=\s*\()/,
      alias: 'mevlog-function',
    },
    'hex-blob': {
      // X'a0b8...' blob literals
      pattern: /\bX'[0-9a-fA-F]*'/,
      alias: 'hex-blob',
      greedy: true,
    },
  });
}

const highlightSql = (code) => Prism.highlight(code, Prism.languages.sql, 'sql');

// Search only supports Ethereum mainnet.
const CHAIN_ID = 1;

// Predefined read-only SQL queries. The block range is fixed server-side.
// Tables: transactions, logs, blocks. Macros (braces):
// {LATEST_BLOCK()}, {NATIVE_TOKEN_PRICE()}, {RESOLVE_ENS("name.eth")}.
const PRESETS = [
  {
    label: 'How much jaredfromsubway.eth spent on gas in last 1 day',
    sql: 'SELECT COUNT(*) AS txs,\n       format_ether(u256_sum(u256_mul(t.gas_used, t.effective_gas_price))) AS gas_spent_eth,\n       format_usd(convert_usd(u256_sum(u256_mul(t.gas_used, t.effective_gas_price)), {NATIVE_TOKEN_PRICE()})) AS gas_spent_usd\nFROM transactions t\nJOIN blocks b ON b.block_number = t.block_number\nWHERE t.from_address = {RESOLVE_ENS("jaredfromsubway.eth")}\n  AND b.timestamp >= unixepoch(\'now\', \'-1 day\')',
  },
  {
    label: 'Which 10 txs transferred the most USDC in last 1 day',
    sql: "SELECT t.tx_hash,\n       format_usd(erc20_to_real(u256_sum(l.erc20_amount), 6)) AS usdc\nFROM logs l\nJOIN transactions t ON t.block_number = l.block_number AND t.tx_index = l.tx_index\nJOIN blocks b ON b.block_number = l.block_number\nWHERE l.address = X'a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48'\n  AND l.erc20_amount IS NOT NULL\n  AND l.block_number >= {LATEST_BLOCK()} - 7200\n  AND b.timestamp >= unixepoch('now', '-1 day')\nGROUP BY t.tx_hash\nORDER BY u256_sum(l.erc20_amount) DESC\nLIMIT 10",
  },
  {
    label: 'Which 10 txs spent the most on gas in last 1 day',
    sql: "SELECT t.block_number, t.tx_hash,\n       format_ether(u256_mul(t.gas_used, t.effective_gas_price)) AS gas_eth,\n       format_usd(convert_usd(u256_mul(t.gas_used, t.effective_gas_price), {NATIVE_TOKEN_PRICE()})) AS gas_usd\nFROM transactions t\nJOIN blocks b ON b.block_number = t.block_number\nWHERE b.timestamp >= unixepoch('now', '-1 day')\nORDER BY u256_mul(t.gas_used, t.effective_gas_price) DESC\nLIMIT 10",
  },
  {
    label: 'Top 10 ETH transfers in last 1 day',
    sql: "SELECT t.tx_hash,\n       format_ether(t.value) AS value_eth,\n       format_usd(convert_usd(t.value, {NATIVE_TOKEN_PRICE()})) AS value_usd\nFROM transactions t\nJOIN blocks b ON b.block_number = t.block_number\nWHERE b.timestamp >= unixepoch('now', '-1 day')\nORDER BY t.value DESC\nLIMIT 10",
  },
  {
    label: 'How many new contracts deployed in last 1 day',
    sql: "SELECT COUNT(*) AS contracts_deployed\nFROM transactions t\nJOIN blocks b ON b.block_number = t.block_number\nWHERE t.signature = 'CREATE()'\n  AND t.success = 1\n  AND b.timestamp >= unixepoch('now', '-1 day')",
  },
  {
    label: 'Top 5 miners by blocks mined in last 1 day',
    sql: "SELECT miner, COUNT(*) AS blocks_mined\nFROM blocks\nWHERE timestamp >= unixepoch('now', '-1 day')\nGROUP BY miner\nORDER BY blocks_mined DESC\nLIMIT 5",
  },
  {
    label: 'Get current DB stats info',
    sql: "WITH db AS (\n  SELECT (SELECT page_count FROM pragma_page_count()) * (SELECT page_size FROM pragma_page_size()) AS bytes\n)\nSELECT (SELECT MIN(block_number) FROM blocks) AS min_block,\n       datetime((SELECT MIN(timestamp) FROM blocks), 'unixepoch') AS min_block_time,\n       (SELECT MAX(block_number) FROM blocks) AS max_block,\n       datetime((SELECT MAX(timestamp) FROM blocks), 'unixepoch') AS max_block_time,\n       (SELECT COUNT(*) FROM blocks) AS total_blocks,\n       (SELECT MAX(block_number) - MIN(block_number) + 1 - COUNT(*) FROM blocks) AS missing_blocks,\n       (SELECT COUNT(*) FROM logs) AS total_logs,\n       (SELECT COUNT(*) FROM transactions) AS total_txs,\n       CASE WHEN bytes < 1024 THEN bytes || ' B'\n            WHEN bytes < 1048576 THEN printf('%.2f KB', bytes / 1024.0)\n            WHEN bytes < 1073741824 THEN printf('%.2f MB', bytes / 1048576.0)\n            ELSE printf('%.2f GB', bytes / 1073741824.0) END AS db_size\nFROM db",
  },
];

const formatTimestamp = (ts) => {
  if (ts === null || ts === undefined) return '';
  const d = new Date(ts * 1000);
  const pad = (n) => String(n).padStart(2, '0');
  return `${d.getUTCFullYear()}-${pad(d.getUTCMonth() + 1)}-${pad(d.getUTCDate())} `
    + `${pad(d.getUTCHours())}:${pad(d.getUTCMinutes())}:${pad(d.getUTCSeconds())} UTC`;
};

const SearchForm = ({ initialValues = {} }) => {
  const [sql, setSql] = useState(initialValues.sql || '');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState(null);
  const [response, setResponse] = useState(null);
  const [dbInfo, setDbInfo] = useState(null);

  useEffect(() => {
    let cancelled = false;
    fetch(`/api/db-info?chain_id=${CHAIN_ID}`)
      .then((res) => (res.ok ? res.json() : null))
      .then((data) => {
        if (!cancelled && data && !data.error && data.min_block !== null) {
          setDbInfo(data);
        }
      })
      .catch(() => { });
    return () => { cancelled = true; };
  }, []);

  const runQuery = async (e) => {
    if (e) e.preventDefault();
    if (!sql.trim()) {
      setError('Enter a SQL query (or pick a preset) to run.');
      return;
    }
    setLoading(true);
    setError(null);

    const params = new URLSearchParams();
    params.set('chain_id', CHAIN_ID);
    params.set('sql', sql.trim());

    try {
      const res = await fetch(`/api/search?${params.toString()}`);
      const text = await res.text();
      let data = null;
      try {
        data = text ? JSON.parse(text) : null;
      } catch {
        // Non-JSON response (timeout, panic, proxy error) — surface the raw body.
        setError(text.trim() || `HTTP ${res.status}: ${res.statusText}`);
        setResponse(null);
        setLoading(false);
        return;
      }
      if (!res.ok || !data || data.error) {
        setError((data && data.error) || `HTTP ${res.status}: ${res.statusText}`);
        setResponse(null);
      } else {
        setResponse(data);
      }
    } catch (err) {
      setError(`Failed to run query: ${err.message}`);
      setResponse(null);
    }
    setLoading(false);
  };

  return (
    <div className="search-form">
      {dbInfo && (
        <div className="indexed-blocks">
          Indexed blocks:{' '}
          <span className="indexed-blocks-range">
            {dbInfo.min_block} – {dbInfo.max_block}
          </span>
          {' '}({formatTimestamp(dbInfo.min_block_timestamp)} – {formatTimestamp(dbInfo.max_block_timestamp)})
        </div>
      )}
      <form onSubmit={runQuery}>
        <SchemaReference />
        <HelpersReference />

        <div className="search-field search-field-tight">
          <span className="search-label">Preset queries</span>
        </div>
        <div className="preset-grid">
          {PRESETS.map((p, idx) => (
            <button
              key={idx}
              type="button"
              className={`preset-card${sql === p.sql ? ' active' : ''}`}
              onClick={() => setSql(p.sql)}
            >
              <span className="preset-label">
                <span className="preset-marker">▸</span>
                {p.label}
              </span>
            </button>
          ))}
        </div>

        <div className="search-field">
          <label className="search-label" htmlFor="search-sql">
            Read-only SQL
          </label>
          <Editor
            textareaId="search-sql"
            className="sql-editor"
            value={sql}
            onValueChange={setSql}
            highlight={highlightSql}
            padding={10}
            placeholder={'SELECT block_number, tx_index, tx_hash, gas_used\nFROM transactions\nORDER BY gas_used DESC\nLIMIT 20'}
            textareaClassName="sql-editor-textarea"
            spellCheck={false}
            autoCapitalize="off"
            autoCorrect="off"
          />
        </div>

        <div className="query-actions">
          <button type="submit" className="query-submit" disabled={loading || !sql.trim()}>
            {loading ? 'Running…' : 'Run query'}
          </button>
          {loading && <span className="query-loading-spinner" />}
        </div>
      </form>

      {error && (
        <div className="query-error">
          {error}
        </div>
      )}

      {response && <QueryResult response={response} />}
    </div>
  );
};

const QueryResult = ({ response }) => {
  const rows = response.result || [];
  const columns = rows.length > 0 ? Object.keys(rows[0]) : [];

  const renderCell = (value) => {
    if (value === null || value === undefined) return '';
    if (typeof value === 'object') return JSON.stringify(value);
    return String(value);
  };

  return (
    <div className="query-result">
      <div className="query-result-meta">
        {response.result_count} rows · {response.duration}
      </div>

      {rows.length === 0 ? (
        <div className="no-data query-no-data">
          Query returned no results
        </div>
      ) : (
        <div className="query-table-wrap">
          <table className="query-table">
            <thead>
              <tr>
                {columns.map((col) => (
                  <th key={col} className="query-cell query-cell-header">
                    {col}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {rows.map((row, rowIdx) => (
                <tr key={rowIdx}>
                  {columns.map((col) => (
                    <td key={col} className="query-cell" title={renderCell(row[col])}>
                      {renderCell(row[col])}
                    </td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
};

export default SearchForm;
