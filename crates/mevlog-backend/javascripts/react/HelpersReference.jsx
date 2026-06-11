import React, { useState } from 'react';

// Sources of truth:
//   functions - evm-sqlite crate (https://github.com/pawurb/evm-sqlite-rs), registered
//               on the read-only query connection via register_functions
//   macros    - crates/mevlog/src/misc/sql_macros.rs::substitute_sql_macros
const GROUPS = [
  {
    name: 'u256 functions',
    entries: [
      {
        sig: 'u256_sum(col)',
        desc: 'Aggregate. Sums U256 BLOB columns (value, erc20_amount, …) into 0x-hex. Plain SUM() cannot total these BLOBs.',
      },
      {
        sig: 'u256_add(a, b)',
        desc: 'Exact 256-bit add.',
      },
      {
        sig: 'u256_mul(a, b)',
        desc: 'Exact 256-bit multiply, e.g. u256_mul(gas_used, effective_gas_price) for tx cost (overflows 64-bit INTEGER).',
      },
      {
        sig: 'u256_to_dec(col)',
        desc: 'U256 BLOB to full-precision decimal string.',
      },
    ],
  },
  {
    name: 'display helpers',
    entries: [
      { sig: 'format_ether(col)', desc: 'Wei to ETH, 6 decimal places.' },
      { sig: 'format_gwei(col)', desc: 'Wei to gwei, 2 decimal places.' },
      {
        sig: 'format_usd(col, price)',
        desc: 'Wei to $-prefixed USD, 2 decimal places. Combine with {NATIVE_TOKEN_PRICE()}.',
      },
    ],
  },
  {
    name: 'macros',
    entries: [
      {
        sig: '{LATEST_BLOCK()}',
        desc: "The chain's current latest block number.",
      },
      {
        sig: '{NATIVE_TOKEN_PRICE()}',
        desc: "The native token's USD price.",
      },
      {
        sig: '{RESOLVE_ENS("name.eth")}',
        desc: "The resolved address as a X'..' blob literal. Ethereum mainnet only; name must end in .eth.",
      },
    ],
  },
];

const HelpersReference = () => {
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
        Custom SQL functions &amp; macros ({GROUPS.map((g) => g.name).join(' · ')})
      </button>
      <a
        className="schema-source-link"
        href="https://github.com/pawurb/evm-sqlite-rs"
        target="_blank"
        rel="noopener noreferrer"
      >
        evm-sqlite-rs ↗
      </a>

      {open && (
        <div className="schema-grid">
          {GROUPS.map((group) => (
            <div key={group.name} className="schema-table">
              <div className="schema-table-name">{group.name}</div>
              {group.entries.map((entry) => (
                <div key={entry.sig} className="helper-entry">
                  <div className="helper-sig">{entry.sig}</div>
                  <div className="helper-desc">{entry.desc}</div>
                </div>
              ))}
            </div>
          ))}
        </div>
      )}
    </div>
  );
};

export default HelpersReference;
