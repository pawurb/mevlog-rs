// Small progressive-enhancement modules for the server-rendered pages.

// Highlights a SQL string with the same <span> classes styles.css themes:
// mevlog helper functions, {MACRO()} tokens, string / hex-blob literals, and
// keywords. Macros and string literals are wrapped first; their spans carry no
// uppercase text, so the later function / keyword passes never re-match inside.
var sqlHighlight = (function () {
  'use strict';

  // Only mevlog's own helpers are themed; bare SQL builtins like COUNT stay plain.
  var FUNCTIONS = [
    'format_usd', 'convert_usd', 'u256_sum', 'u256_mul', 'u256_add',
    'u256_to_dec', 'erc20_to_real', 'format_ether', 'format_gwei',
  ];
  // Longest-first so multi-word keywords win over their fragments.
  var KEYWORDS = [
    'IS NOT NULL', 'ORDER BY', 'GROUP BY', 'SELECT', 'WHERE', 'FROM',
    'LIMIT', 'DESC', 'AND', 'AS',
  ];

  var FN_RE = new RegExp('\\b(' + FUNCTIONS.join('|') + ')\\b(?=\\s*\\()', 'g');
  var KW_RE = new RegExp('\\b(' + KEYWORDS.join('|') + ')\\b', 'g');

  function escapeHtml(s) {
    return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
  }

  function span(cls, inner) {
    return '<span class="' + cls + '">' + inner + '</span>';
  }

  return function highlight(sql) {
    var out = escapeHtml(sql);
    out = out.replace(/\{[^}]*\}/g, function (macro) {
      var inner = macro.replace(/"[^"]*"/g, function (q) { return span('t-str', q); });
      return span('t-macro', inner);
    });
    out = out.replace(/X?'[^']*'/g, function (lit) { return span('t-str', lit); });
    out = out.replace(FN_RE, function (fn) { return span('t-fn', fn); });
    out = out.replace(KW_RE, function (kw) { return span('t-kw', kw); });
    return out;
  };
})();

// Hero demo: switch the example query shown in the static terminal card. Each
// tab's `data-q` is a key into HERO_PRESETS: trimmed SQL, a sample result row,
// and a one-sentence description of what the SQL returns.
var HERO_PRESETS = [
  {
    key: 'ens-gas-spend',
    demo_label: 'Total gas the jaredfromsubway.eth MEV bot burned across its transactions in the last day, in USD.',
    result: { key: 'gas_spent', val: '$48,210.34' },
    demo_sql:
`SELECT format_usd(convert_usd(
         u256_sum(u256_mul(gas_used, effective_gas_price)),
         {NATIVE_TOKEN_PRICE()})) AS gas_spent
FROM transactions
WHERE from_address = {RESOLVE_ENS("jaredfromsubway.eth")}`,
  },
  {
    key: 'usdc-top-txs',
    demo_label: 'Total USDC volume transferred across all logs in the last day.',
    result: { key: 'usdc_volume', val: '1,284,019,442' },
    demo_sql:
`SELECT erc20_to_real(u256_sum(erc20_amount), 6) AS usdc_volume
FROM logs
WHERE address = X'a0b8...eb48'
  AND erc20_amount IS NOT NULL`,
  },
  {
    key: 'top-gas-txs',
    demo_label: 'The single transaction that paid the most for gas, in USD.',
    result: { key: 'gas_usd', val: '$9,418.55' },
    demo_sql:
`SELECT tx_hash,
       format_usd(convert_usd(u256_mul(gas_used,
         effective_gas_price), {NATIVE_TOKEN_PRICE()})) AS gas_usd
FROM transactions
ORDER BY u256_mul(gas_used, effective_gas_price) DESC
LIMIT 1`,
  },
  {
    key: 'top-eth-transfers',
    demo_label: 'The single largest native ETH transfer, in ETH.',
    result: { key: 'value_eth', val: '4,512.337000 ETH' },
    demo_sql:
`SELECT tx_hash,
       format_ether(value) AS value_eth
FROM transactions
ORDER BY value DESC
LIMIT 1`,
  },
  {
    key: 'top-methods',
    demo_label: 'The fifteen most frequently called contract methods.',
    result: { key: 'signature', val: 'transfer()' },
    demo_sql:
`SELECT signature, COUNT(*) AS calls
FROM transactions
WHERE signature IS NOT NULL
GROUP BY signature
ORDER BY calls DESC
LIMIT 15`,
  },
  {
    key: 'new-contracts',
    demo_label: 'How many contracts were successfully deployed.',
    result: { key: 'contracts_deployed', val: '1,204' },
    demo_sql:
`SELECT COUNT(*) AS contracts_deployed
FROM transactions
WHERE signature = 'CREATE()'
  AND success = 1`,
  },
];

(function heroDemo() {
  'use strict';

  function init() {
    var code = document.getElementById('hero-code');
    var tabs = document.querySelectorAll('.hero-tab');
    if (!code || !tabs.length) return;

    var byKey = {};
    HERO_PRESETS.forEach(function (p) { byKey[p.key] = p; });

    var key = document.getElementById('hero-key');
    var val = document.getElementById('hero-val');
    var desc = document.getElementById('hero-desc');

    function show(t) {
      var q = byKey[t.dataset.q];
      if (!q) return;
      code.innerHTML = sqlHighlight(q.demo_sql) + '<span class="hero-caret"></span>';
      key.textContent = q.result.key;
      val.textContent = q.result.val;
      if (desc) desc.textContent = q.demo_label;
      tabs.forEach(function (other) { other.classList.toggle('active', other === t); });
    }

    tabs.forEach(function (t) {
      t.addEventListener('click', function () { show(t); });
    });

    var active = document.querySelector('.hero-tab.active') || tabs[0];
    show(active);
  }

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', init);
  } else {
    init();
  }
})();
