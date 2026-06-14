// Small progressive-enhancement modules for the server-rendered pages.
// WebSocket / data fetching lives in the React bundle.

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

// Hero demo: switch the example query shown in the terminal card. Each query
// keeps its SQL as a raw string plus the single sample result row the card
// renders, and a `name` linking to the matching /search preset.
(function heroDemo() {
  'use strict';

  var CMD = 'mevlog query -b 7200:latest --sql';

  var QUERIES = [
    {
      name: 'ens-gas-spend',
      result: { key: 'gas_spent', val: '$48,210.34' },
      sql:
        'SELECT format_usd(convert_usd(\n' +
        '         u256_sum(u256_mul(gas_used, effective_gas_price)),\n' +
        '         {NATIVE_TOKEN_PRICE()})) AS gas_spent\n' +
        'FROM transactions\n' +
        'WHERE from_address = {RESOLVE_ENS("jaredfromsubway.eth")}',
    },
    {
      name: 'usdc-top-txs',
      result: { key: 'usdc_volume', val: '1,284,019,442' },
      sql:
        'SELECT erc20_to_real(u256_sum(erc20_amount), 6) AS usdc_volume\n' +
        'FROM logs\n' +
        "WHERE address = X'a0b8...eb48'\n" +
        '  AND erc20_amount IS NOT NULL',
    },
    {
      name: 'top-eth-transfers',
      result: { key: 'value_eth', val: '4,512.337000 ETH' },
      sql:
        'SELECT tx_hash,\n' +
        '       format_ether(value) AS value_eth\n' +
        'FROM transactions\n' +
        'ORDER BY value DESC\n' +
        'LIMIT 1',
    },
    {
      name: 'top-gas-txs',
      result: { key: 'gas_usd', val: '$9,418.55' },
      sql:
        'SELECT tx_hash,\n' +
        '       format_usd(convert_usd(u256_mul(gas_used,\n' +
        '         effective_gas_price), {NATIVE_TOKEN_PRICE()})) AS gas_usd\n' +
        'FROM transactions\n' +
        'ORDER BY u256_mul(gas_used, effective_gas_price) DESC\n' +
        'LIMIT 1',
    },
    {
      name: 'top-methods',
      result: { key: 'signature', val: 'transfer()' },
      sql:
        'SELECT signature, COUNT(*) AS calls\n' +
        'FROM transactions\n' +
        'WHERE signature IS NOT NULL\n' +
        'GROUP BY signature\n' +
        'ORDER BY calls DESC\n' +
        'LIMIT 15',
    },
    {
      name: 'new-contracts',
      result: { key: 'contracts_deployed', val: '1,204' },
      sql:
        'SELECT COUNT(*) AS contracts_deployed\n' +
        'FROM transactions\n' +
        "WHERE signature = 'CREATE()'\n" +
        '  AND success = 1',
    },
  ];

  function init() {
    var code = document.getElementById('hero-code');
    var tabs = document.querySelectorAll('.hero-tab');
    if (!code || !tabs.length) return;

    var cmd = document.getElementById('hero-cmd');
    var key = document.getElementById('hero-key');
    var val = document.getElementById('hero-val');
    var tryBtn = document.getElementById('hero-try');

    function show(i) {
      var q = QUERIES[i];
      if (!q) return;
      code.innerHTML = sqlHighlight(q.sql) + '<span class="hero-caret"></span>';
      cmd.textContent = CMD;
      key.textContent = q.result.key;
      val.textContent = q.result.val;
      // Carry the selected query to /search via its short name; it auto-runs there.
      if (tryBtn && q.name) tryBtn.href = '/search?q=' + encodeURIComponent(q.name);
      tabs.forEach(function (t, j) { t.classList.toggle('active', j === i); });
    }

    tabs.forEach(function (t) {
      t.addEventListener('click', function () { show(parseInt(t.dataset.q, 10)); });
    });

    show(0);
  }

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', init);
  } else {
    init();
  }
})();
