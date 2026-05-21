// Stem playground entry point. Loads the WASM module, wires the
// textarea + grammar selector to a debounced re-render, and updates
// the preview iframe + diagnostics list.

import init, { render, render_v2 } from './pkg/stem_wasm.js';

const DEFAULTS = {
  v1: `[type:document, title:"Playground"]

section(welcome)(
  # Welcome to Stem (v1 legacy)

  Edit on the left → see the render on the right.

  layout(two-column)(
    col(
      ### Try

      - Add a heading with text(red)[color:red] inline styling
      - Wrap a quote across two lines
      - Introduce a typo to see diagnostics
    )
    col(
      ### Notice

      Block-level functions like layout, col, table appear as blocks
      even when their bodies are short. Inline functions like text
      and footnote stay inline.
    )
  )
)

section(closing)(
  # Thanks
  note(Switch the grammar dropdown to v2 to try the new syntax.)
)
`,
  v2: `[type:document, title:"Playground"]

section{
  h1(Welcome to Stem v2)
  p(Edit on the left → see the render on the right.)
  p(Inline styling: @text[color:primary](critical) text, a
    @footnote(see appendix) for references, or @date(2026.05.20) dates.)
}

section{
  h2(Two-column layout)

  layout[kind:two-column]{
    col{
      h3(Problems)
      ol[style:1.]{
        li(Format fragmentation)
        li(Hard for AI to generate)
        li(Manual conversion work)
      }
    }
    col{
      h3(Opportunities)
      ol[style:가.]{
        li(Single source format)
        li(AI-native design)
        li(Auto conversion)
      }
    }
  }
}

section{
  h2(Tables)

  table[border:outer]{
    row[kind:header]{
      cell(Phase)
      cell(Content)
      cell[colspan:2](Timeline)
    }
    row{
      cell(Phase 1)
      cell(Spec finalization)
      cell(2026 Q2)
      cell[bg:yellow](In Progress)
    }
  }
}

section{
  h2(Try messing with it)

  p(Try removing a closing brace, or writing
    @bogus[foo:bar](thing) — diagnostics appear below.)
}
`,
};

const SHEET_DEMO = `[type:sheet, name:"Q4 Demo"]

sheet[id:q4]{
  col[at:A, width:120]
  col[at:B, fmt:currency]
  col[at:C, fmt:percent]
  row[at:1, weight:bold, bg:gray]

  // Header
  cell[at:A1](Item)
  cell[at:B1](Revenue)
  cell[at:C1](Margin)

  // Data rows
  cell[at:A2](Widget)    cell[at:B2](42000)  cell[at:C2](0.35)
  cell[at:A3](Gadget)    cell[at:B3](38500)  cell[at:C3](0.42)
  cell[at:A4](Sprocket)  cell[at:B4](19200)  cell[at:C4](0.28)

  // Total row — formulas reference the cells above
  cell[at:A5, weight:bold](Total)
  cell[at:B5, weight:bold](@formula("SUM(B2:B4)"))
  cell[at:C5, weight:bold, bg:yellow](@formula("AVERAGE(C2:C4)"))

  format[at:"A1:C1", align:center]
}
`;

const STORAGE_KEY_BY_GRAMMAR = {
  v1: 'stem-playground.src.v1',
  v2: 'stem-playground.src.v2',
};
const GRAMMAR_KEY = 'stem-playground.grammar';

async function main() {
  await init();

  const src = document.getElementById('src');
  const preview = document.getElementById('preview');
  const diags = document.getElementById('diags');
  const stats = document.getElementById('stats');
  const reset = document.getElementById('reset');
  const copy = document.getElementById('copy');
  const grammarSel = document.getElementById('grammar');

  // Pick saved grammar or default to v2
  const savedGrammar = localStorage.getItem(GRAMMAR_KEY);
  if (savedGrammar === 'v1' || savedGrammar === 'v2') {
    grammarSel.value = savedGrammar;
  } else {
    grammarSel.value = 'v2';
  }

  function loadSourceForCurrentGrammar() {
    const g = grammarSel.value;
    const saved = localStorage.getItem(STORAGE_KEY_BY_GRAMMAR[g]);
    src.value = saved ?? DEFAULTS[g];
  }

  loadSourceForCurrentGrammar();

  let timer;
  let lastHtml = '';

  function callRender(text) {
    return grammarSel.value === 'v2' ? render_v2(text) : render(text);
  }

  function rerender() {
    const result = callRender(src.value);
    lastHtml = result.html;
    preview.srcdoc = wrapHtml(result.html);
    renderDiags(diags, result.diagnostics);
    renderStats(stats, result.stats);
    try {
      localStorage.setItem(STORAGE_KEY_BY_GRAMMAR[grammarSel.value], src.value);
    } catch {
      // private window or quota — silently skip
    }
  }

  src.addEventListener('input', () => {
    clearTimeout(timer);
    timer = setTimeout(rerender, 50);
  });

  grammarSel.addEventListener('change', () => {
    localStorage.setItem(GRAMMAR_KEY, grammarSel.value);
    loadSourceForCurrentGrammar();
    rerender();
  });

  reset.addEventListener('click', () => {
    // Cycle reset: first click → default doc; second click (within ~1s)
    // → sheet demo so users can try the spreadsheet view.
    const g = grammarSel.value;
    const now = Date.now();
    if (g === 'v2' && reset.dataset.lastReset && now - +reset.dataset.lastReset < 1500) {
      src.value = SHEET_DEMO;
      reset.dataset.lastReset = '';
    } else {
      src.value = DEFAULTS[g];
      reset.dataset.lastReset = String(now);
    }
    rerender();
    src.focus();
  });

  copy.addEventListener('click', async () => {
    try {
      await navigator.clipboard.writeText(lastHtml);
      flashButton(copy, 'copied!');
    } catch {
      flashButton(copy, 'copy failed');
    }
  });

  rerender();
}

function renderDiags(host, list) {
  host.innerHTML = '';
  if (list.length === 0) {
    const li = document.createElement('li');
    li.className = 'empty';
    li.textContent = 'no diagnostics — clean parse';
    host.appendChild(li);
    return;
  }
  for (const d of list) {
    const li = document.createElement('li');
    li.className = `diag diag-${d.severity}`;
    li.innerHTML =
      `<span class="sev">${escapeHtml(d.severity)}</span>` +
      `<span class="code">${escapeHtml(d.code)}</span>` +
      `<span class="where">L${d.line}:${d.col}</span>` +
      `<span class="msg">${escapeHtml(d.message)}</span>`;
    host.appendChild(li);
  }
}

function renderStats(host, stats) {
  host.classList.remove('has-error', 'has-warning');
  if (stats.errors > 0) host.classList.add('has-error');
  else if (stats.warnings > 0) host.classList.add('has-warning');
  const parts = [`${stats.nodes} nodes`];
  if (stats.errors > 0) parts.push(`${stats.errors} error${stats.errors === 1 ? '' : 's'}`);
  if (stats.warnings > 0) parts.push(`${stats.warnings} warning${stats.warnings === 1 ? '' : 's'}`);
  host.textContent = parts.join('  ·  ');
}

function wrapHtml(fragment) {
  return `<!doctype html>
<html><head><meta charset="utf-8"><style>
  body { font-family: system-ui, -apple-system, "Segoe UI", sans-serif;
         color: #14181f; max-width: 42rem; margin: 1.5rem auto;
         padding: 0 1rem; line-height: 1.55; }
  h1, h2, h3, h4 { font-family: inherit; margin-top: 1.5rem; }
  table { border-collapse: collapse; }
  th, td { border-color: #d0d7de; }
  code { font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
         background: #f6f8fa; padding: 0 0.25em; border-radius: 3px; }
  .stem-pagebreak { height: 0; border-top: 1px dashed #d0d7de; margin: 2rem 0; }
  .stem-note { display: block; padding: 0.5rem 0.75rem;
               background: #f6f8fa; border-left: 3px solid #8b949e;
               margin: 1rem 0; }
  .stem-sheet table { font-family: ui-monospace, monospace; font-size: 13px; }
  .stem-sheet th { background: #f6f8fa; color: #888; }
</style></head><body>${fragment}</body></html>`;
}

function flashButton(btn, msg) {
  const old = btn.textContent;
  btn.textContent = msg;
  setTimeout(() => { btn.textContent = old; }, 900);
}

function escapeHtml(s) {
  return String(s)
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

main().catch((e) => {
  document.body.innerHTML =
    `<pre style="color:#f85149;padding:1rem;font-family:ui-monospace,monospace">` +
    `failed to load stem-wasm:\n\n${escapeHtml(String(e?.stack || e))}\n\n` +
    `did you run scripts/serve-playground.sh?</pre>`;
});
