import React, { useState, useEffect } from 'react';
import Editor from 'react-simple-code-editor';
import Prism from 'prismjs/components/prism-core';
import 'prismjs/components/prism-sql';
import SchemaReference from './SchemaReference';
import HelpersReference from './HelpersReference';
import PRESETS from './presets';

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
  // Set when a query needs to auto-run once `sql` state has been populated.
  const [pendingRun, setPendingRun] = useState(false);

  // Resolve a `?q=<preset-name>` (or `?run=1` with a server-provided sql) from
  // the URL on mount, load the matching query, and queue it to auto-execute.
  useEffect(() => {
    const params = new URLSearchParams(window.location.search);
    const slug = params.get('q');
    if (slug) {
      const preset = PRESETS.find((p) => p.key === slug);
      if (preset) {
        setSql(preset.full_sql);
        setPendingRun(true);
      }
    } else if (params.get('run') && (initialValues.sql || '').trim()) {
      setPendingRun(true);
    }
  }, []);

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

  // The preset matching the current editor contents, if any, so its one-sentence
  // description can be shown above the SQL.
  const activePreset = PRESETS.find((p) => p.full_sql === sql);

  // Fire the queued auto-run once `sql` has actually been set from the URL.
  useEffect(() => {
    if (pendingRun && sql.trim()) {
      setPendingRun(false);
      runQuery();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [pendingRun, sql]);

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
          {PRESETS.map((p) => (
            <button
              key={p.key}
              type="button"
              className={`preset-card${sql === p.full_sql ? ' active' : ''}`}
              onClick={() => setSql(p.full_sql)}
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
          {activePreset && (
            <p className="preset-description">{activePreset.description}</p>
          )}
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
