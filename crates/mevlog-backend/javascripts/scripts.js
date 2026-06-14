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

// Hero demo: switch the example query shown in the terminal card. Each tab's
// `data-q` is a preset key; the preset data (trimmed demo_sql, sample result,
// one-sentence demo_label) comes from the shared window.MEVLOG_PRESETS source
// that the /search form also uses, so the two never drift apart.
(function heroDemo() {
  'use strict';

  var CMD = 'mevlog query -b 7200:latest --sql';

  function init() {
    var code = document.getElementById('hero-code');
    var tabs = document.querySelectorAll('.hero-tab');
    if (!code || !tabs.length) return;

    var presets = window.MEVLOG_PRESETS || [];
    var byKey = {};
    presets.forEach(function (p) { byKey[p.key] = p; });

    var cmd = document.getElementById('hero-cmd');
    var key = document.getElementById('hero-key');
    var val = document.getElementById('hero-val');
    var desc = document.getElementById('hero-desc');
    var tryBtn = document.getElementById('hero-try');

    function show(t) {
      var q = byKey[t.dataset.q];
      if (!q) return;
      code.innerHTML = sqlHighlight(q.demo_sql) + '<span class="hero-caret"></span>';
      cmd.textContent = CMD;
      if (q.result) {
        key.textContent = q.result.key;
        val.textContent = q.result.val;
      }
      if (desc) desc.textContent = q.demo_label || '';
      // Carry the selected query to /search via its key; it auto-runs there.
      if (tryBtn) tryBtn.href = '/search?q=' + encodeURIComponent(q.key);
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
