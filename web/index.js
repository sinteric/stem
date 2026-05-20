// Stem playground entry point. Loads the WASM module, wires the
// textarea to a debounced re-render, and updates the preview iframe +
// diagnostics list.

import init, { render } from './pkg/stem_wasm.js';

const DEFAULT_SRC = `[type:document, title:"Playground"]

section(welcome)(
  # Welcome to Stem

  Edit on the left → see the render on the right.

  layout(two-column)(
    col(
      ### Try

      - Add a heading with text(red)[color:red] inline styling
      - Wrap your text(quote)[weight:bold] across two lines
      - Introduce a typo like section(toc)[blob:wrong] to see diagnostics
    )
    col(
      ### Notice

      Block-level functions like layout, col, table appear as blocks
      even when their bodies are short. Inline functions like text
      and footnote stay inline.
    )
  )

  table[border:outer](
    row(header)(
      cell(Metric)
      cell(Value)[align:right]
    )
    row(
      cell(MRR)
      cell($4.2M)[align:right, bg:yellow]
    )
  )
)

section(closing)(
  # Thanks for trying

  note(Type into the source pane to play with the syntax.)
)
`;

const STORAGE_KEY = 'stem-playground.src';

async function main() {
  await init();

  const src = document.getElementById('src');
  const preview = document.getElementById('preview');
  const diags = document.getElementById('diags');
  const stats = document.getElementById('stats');
  const reset = document.getElementById('reset');
  const copy = document.getElementById('copy');

  // Restore from localStorage if present, otherwise default.
  src.value = localStorage.getItem(STORAGE_KEY) || DEFAULT_SRC;

  let timer;
  let lastHtml = '';

  function rerender() {
    const result = render(src.value);
    lastHtml = result.html;
    preview.srcdoc = wrapHtml(result.html);
    renderDiags(diags, result.diagnostics);
    renderStats(stats, result.stats);
    try {
      localStorage.setItem(STORAGE_KEY, src.value);
    } catch {
      // private window or quota — silently skip
    }
  }

  src.addEventListener('input', () => {
    clearTimeout(timer);
    timer = setTimeout(rerender, 50);
  });

  reset.addEventListener('click', () => {
    src.value = DEFAULT_SRC;
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
  // The wasm renderer emits a HTML fragment (no doctype). Wrap it in a
  // minimal page with the same theme CSS the full renderer uses, so
  // preview matches what stem render --format html would produce.
  return `<!doctype html>
<html><head><meta charset="utf-8"><style>
  body { font-family: system-ui, -apple-system, "Segoe UI", sans-serif;
         color: #14181f; max-width: 42rem; margin: 1.5rem auto;
         padding: 0 1rem; line-height: 1.55; }
  h1, h2, h3, h4 { font-family: inherit; margin-top: 1.5rem; }
  table { width: 100%; border-collapse: collapse; }
  th, td { border-color: #d0d7de; }
  code { font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
         background: #f6f8fa; padding: 0 0.25em; border-radius: 3px; }
  .stem-pagebreak { height: 0; border-top: 1px dashed #d0d7de; margin: 2rem 0; }
  .stem-note { display: block; padding: 0.5rem 0.75rem;
               background: #f6f8fa; border-left: 3px solid #8b949e;
               margin: 1rem 0; }
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
