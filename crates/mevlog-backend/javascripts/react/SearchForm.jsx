import React, { useState, useEffect } from 'react';
import SchemaReference from './SchemaReference';
import HelpersReference from './HelpersReference';

// Search only supports Ethereum mainnet.
const CHAIN_ID = 1;

// Predefined read-only SQL queries. The block range is fixed server-side.
// Tables: transactions, logs, blocks. Macros (braces):
// {LATEST_BLOCK()}, {NATIVE_TOKEN_PRICE()}, {RESOLVE_ENS("name.eth")}.
const PRESETS = [
  {
    label: 'Top 20 txs by gas used (last 50 blocks)',
    sql: 'SELECT block_number, tx_index, tx_hash, gas_used,\n       u256_to_dec(value) AS value\nFROM transactions\nORDER BY gas_used DESC\nLIMIT 20',
  },
  {
    label: 'Total USDC transferred (last 100 blocks)',
    sql: "SELECT u256_sum(erc20_amount) AS total_usdc\nFROM logs\nWHERE address = X'a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48'\n  AND erc20_amount IS NOT NULL",
  },
  {
    label: 'Txs from vitalik.eth (mainnet only)',
    sql: 'SELECT block_number, tx_index, tx_hash, to_address AS "to",\n       u256_to_dec(value) AS value\nFROM transactions\nWHERE from_address = {RESOLVE_ENS("vitalik.eth")}',
  },
  {
    label: 'Tx count per block (recent window)',
    sql: 'SELECT block_number, COUNT(*) AS txs\nFROM transactions\nWHERE block_number > {LATEST_BLOCK()} - 20\nGROUP BY block_number\nORDER BY block_number DESC',
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
      .catch(() => {});
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
      const data = await res.json();
      if (!res.ok || data.error) {
        setError(data.error || `HTTP ${res.status}: ${res.statusText}`);
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

  const labelStyle = {
    display: 'block',
    color: '#888',
    fontSize: '12px',
    fontWeight: '500',
    textTransform: 'uppercase',
    letterSpacing: '0.5px',
    marginBottom: '4px',
  };

  const inputStyle = {
    backgroundColor: '#1a1a1a',
    border: '1px solid #333',
    borderRadius: '4px',
    color: '#fff',
    fontFamily: 'monospace',
    fontSize: '14px',
    padding: '8px 10px',
    width: '100%',
    boxSizing: 'border-box',
  };

  const buttonStyle = {
    backgroundColor: '#ffd700',
    border: '1px solid #ccc',
    borderRadius: '4px',
    cursor: loading ? 'not-allowed' : 'pointer',
    color: '#000',
    fontSize: '14px',
    fontWeight: 'bold',
    padding: '10px 24px',
    opacity: loading ? 0.6 : 1,
  };

  return (
    <div className="search-form">
      {dbInfo && (
        <div style={{
          color: 'var(--foreground)',
          fontFamily: 'monospace',
          fontSize: '13px',
          marginBottom: '12px',
        }}>
          Indexed blocks:{' '}
          <span style={{ color: 'var(--bright-white)' }}>
            {dbInfo.min_block} – {dbInfo.max_block}
          </span>
          {' '}({formatTimestamp(dbInfo.min_block_timestamp)} – {formatTimestamp(dbInfo.max_block_timestamp)})
        </div>
      )}
      <form onSubmit={runQuery}>
        <SchemaReference />
        <HelpersReference />

        <div style={{ marginBottom: '4px' }}>
          <span style={labelStyle}>Preset queries</span>
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

        <div style={{ marginBottom: '12px' }}>
          <label style={labelStyle} htmlFor="search-sql">
            Read-only SQL (tables: transactions, logs, blocks)
          </label>
          <textarea
            id="search-sql"
            value={sql}
            onChange={(e) => setSql(e.target.value)}
            placeholder={'SELECT block_number, tx_index, tx_hash, gas_used\nFROM transactions\nORDER BY gas_used DESC\nLIMIT 20'}
            rows={8}
            style={{ ...inputStyle, resize: 'vertical' }}
          />
        </div>

        <button type="submit" style={buttonStyle} disabled={loading || !sql.trim()}>
          {loading ? 'Running…' : 'Run query'}
        </button>
      </form>

      {error && (
        <div style={{
          marginTop: '16px',
          backgroundColor: '#f8d7da',
          color: '#721c24',
          border: '1px solid #f5c6cb',
          borderRadius: '4px',
          padding: '12px',
          fontFamily: 'monospace',
          fontSize: '13px',
          whiteSpace: 'pre-wrap',
        }}>
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

  const metaStyle = {
    color: '#888',
    fontSize: '13px',
    margin: '16px 0 8px',
    fontFamily: 'monospace',
  };

  const cellStyle = {
    border: '1px solid #333',
    padding: '4px 8px',
    fontFamily: 'monospace',
    fontSize: '13px',
    color: '#fff',
    whiteSpace: 'nowrap',
    overflow: 'hidden',
    textOverflow: 'ellipsis',
    maxWidth: '320px',
  };

  return (
    <div className="query-result">
      <div style={metaStyle}>
        {response.result_count} rows · {response.duration}
      </div>

      {rows.length === 0 ? (
        <div className="no-data" style={{ color: '#888', padding: '12px' }}>
          Query returned no results
        </div>
      ) : (
        <div style={{ overflowX: 'auto' }}>
          <table style={{ borderCollapse: 'collapse', width: '100%' }}>
            <thead>
              <tr>
                {columns.map((col) => (
                  <th key={col} style={{ ...cellStyle, backgroundColor: '#2a2a2a', fontWeight: 'bold' }}>
                    {col}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {rows.map((row, rowIdx) => (
                <tr key={rowIdx}>
                  {columns.map((col) => (
                    <td key={col} style={cellStyle} title={renderCell(row[col])}>
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
