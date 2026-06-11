import React, { useState } from 'react';

// Source of truth: crates/mevlog/migrations/txs/20260527120000_create_schema_v1.up.sql
// Keep in sync when the txs schema version changes.
//
// Column hints (not in the DDL, but needed to write working queries):
//   u256     - 32-byte big-endian BLOB; use u256_sum/u256_mul/u256_add/u256_to_dec
//   addr     - 20-byte address BLOB; predicates need X'..' literals
//   hash     - 32-byte hash BLOB
//   selector - 4-byte method selector BLOB
//   unix     - unix epoch seconds
//   0/1      - SQLite has no boolean; stored as 0/1
const TABLES = [
  {
    name: 'transactions',
    columns: [
      { name: 'block_number', type: 'BIGINT' },
      { name: 'tx_index', type: 'BIGINT' },
      { name: 'tx_hash', type: 'BLOB', hint: 'hash' },
      { name: 'nonce', type: 'BIGINT' },
      { name: 'from_address', type: 'BLOB', hint: 'addr' },
      { name: 'to_address', type: 'BLOB', nullable: true, hint: 'addr' },
      { name: 'value', type: 'BLOB', hint: 'u256' },
      { name: 'gas_limit', type: 'BIGINT' },
      { name: 'gas_used', type: 'BIGINT' },
      { name: 'effective_gas_price', type: 'BIGINT' },
      { name: 'gas_price', type: 'BIGINT' },
      { name: 'max_fee_per_gas', type: 'BIGINT' },
      { name: 'max_priority_fee_per_gas', type: 'BIGINT' },
      { name: 'transaction_type', type: 'BIGINT', nullable: true },
      { name: 'success', type: 'BOOLEAN', hint: '0/1' },
      { name: 'coinbase_transfer', type: 'BLOB', nullable: true, hint: 'u256' },
      { name: 'signature_hash', type: 'BLOB', nullable: true, hint: 'selector' },
      { name: 'signature', type: 'TEXT', nullable: true },
    ],
    footer: ['unique index: tx_hash'],
  },
  {
    name: 'blocks',
    columns: [
      { name: 'block_number', type: 'INTEGER', pk: true },
      { name: 'block_hash', type: 'BLOB', hint: 'hash' },
      { name: 'miner', type: 'BLOB', hint: 'addr' },
      { name: 'gas_used', type: 'BIGINT' },
      { name: 'timestamp', type: 'BIGINT', hint: 'unix' },
      { name: 'base_fee_per_gas', type: 'BIGINT', nullable: true },
    ],
    footer: ['index: timestamp'],
  },
  {
    name: 'logs',
    columns: [
      { name: 'block_number', type: 'BIGINT', pk: true },
      { name: 'tx_index', type: 'BIGINT' },
      { name: 'log_index', type: 'BIGINT', pk: true },
      { name: 'address', type: 'BLOB', hint: 'addr' },
      { name: 'topic0', type: 'BLOB', nullable: true, hint: 'hash' },
      { name: 'topic1', type: 'BLOB', nullable: true, hint: 'hash' },
      { name: 'topic2', type: 'BLOB', nullable: true, hint: 'hash' },
      { name: 'topic3', type: 'BLOB', nullable: true, hint: 'hash' },
      { name: 'data', type: 'BLOB' },
      { name: 'erc20_amount', type: 'BLOB', nullable: true, hint: 'u256' },
      { name: 'signature', type: 'TEXT', nullable: true },
    ],
    footer: ['PK (block_number, log_index)'],
  },
];

const SchemaReference = () => {
  const [open, setOpen] = useState(false);

  return (
    <div className="schema-reference">
      <button
        type="button"
        className="schema-toggle"
        aria-expanded={open}
        onClick={() => setOpen(!open)}
      >
        <span className="schema-marker">{open ? '▾' : '▸'}</span>
        Schema reference ({TABLES.map((t) => t.name).join(' · ')})
      </button>

      {open && (
        <div className="schema-grid">
          {TABLES.map((table) => (
            <div key={table.name} className="schema-table">
              <div className="schema-table-name">{table.name}</div>
              <table className="schema-columns">
                <tbody>
                  {table.columns.map((col) => (
                    <tr key={col.name}>
                      <td className="schema-col-name">
                        {col.name}
                        {col.nullable && <span className="schema-nullable">?</span>}
                        {col.pk && <span className="schema-pk"> PK</span>}
                      </td>
                      <td className="schema-col-type">{col.type}</td>
                      <td className="schema-col-hint">{col.hint || ''}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
              <div className="schema-footer">
                {table.footer.map((line) => (
                  <div key={line}>{line}</div>
                ))}
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
};

export default SchemaReference;
