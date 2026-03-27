import init, { JsLayoutEngine, layoutPresets, helpBindings } from './pkg/panes_wasm_demo.js';

const ANIM_MS = 250;

// 12 chromatic ANSI slots from palette-core (same order as terminal demo)
const ANSI_ACCENT_VARS = [
  '--ansi-red', '--ansi-green', '--ansi-yellow', '--ansi-blue',
  '--ansi-magenta', '--ansi-cyan',
  '--ansi-bright-red', '--ansi-bright-green', '--ansi-bright-yellow',
  '--ansi-bright-blue', '--ansi-bright-magenta', '--ansi-bright-cyan',
];

function accentColor(kindIndex) {
  const varName = ANSI_ACCENT_VARS[kindIndex % ANSI_ACCENT_VARS.length];
  return getComputedStyle(document.documentElement).getPropertyValue(varName).trim();
}

const state = {
  engine: null,
  themes: [],
  themeIdx: 0,
  presets: [],
  presetIdx: 0,
  animStart: null,
  animFrame: null,
  dragging: false,
};

// -- DOM refs --

const viewport = document.getElementById('viewport');
const presetSelect = document.getElementById('preset-select');
const themeSelect = document.getElementById('theme-select');
const presetDesc = document.getElementById('preset-desc');
const stPreset = document.getElementById('st-preset');
const stTheme = document.getElementById('st-theme');
const stPanels = document.getElementById('st-panels');
const stFocus = document.getElementById('st-focus');
const stDiff = document.getElementById('st-diff');
const themeCssEl = document.getElementById('theme-css');
const overlayContainer = document.getElementById('overlay-container');

// -- Theme --

function applyTheme() {
  const info = state.themes[state.themeIdx];
  themeCssEl.textContent = state.engine.themeCss(info.id);
}

// -- Rendering --

function resolveLayout(t = 1.0) {
  const rect = viewport.getBoundingClientRect();
  if (rect.width === 0 || rect.height === 0) return null;
  const json = state.engine.resolveLerped(rect.width, rect.height, t);
  return JSON.parse(json);
}

function syncPanels(panels) {
  const activeIds = new Set(panels.map(p => String(p.id)));

  // Remove stale panels (skip overlay container)
  for (const el of [...viewport.children]) {
    if (el.id === 'overlay-container') continue;
    if (!activeIds.has(el.dataset.panelId)) {
      el.remove();
    }
  }

  // Upsert panels
  for (const p of panels) {
    const key = String(p.id);
    let el = viewport.querySelector(`[data-panel-id="${key}"]`);
    if (!el) {
      el = document.createElement('div');
      el.className = 'panel';
      el.dataset.panelId = key;
      el.innerHTML = '<div class="panel-title"></div><div class="panel-body"></div>';
      viewport.appendChild(el);
    }

    el.style.left = `${p.x}px`;
    el.style.top = `${p.y}px`;
    el.style.width = `${p.w}px`;
    el.style.height = `${p.h}px`;

    el.classList.toggle('focused', p.focused);

    const accent = accentColor(p.kind_index);
    const title = el.firstElementChild;
    title.textContent = p.kind;
    title.style.color = p.focused ? accent : 'var(--fg)';
    title.style.background = p.focused ? 'var(--bg-hi, rgba(255,255,255,0.05))' : 'transparent';

    const body = el.lastElementChild;
    body.textContent = `${Math.round(p.w)}\u00d7${Math.round(p.h)}  (${Math.round(p.x)},${Math.round(p.y)})`;
  }
}

function updateStatus() {
  const preset = state.presets[state.presetIdx];
  stPreset.textContent = `preset: ${preset.name}`;
  presetDesc.textContent = preset.description;

  const theme = state.themes[state.themeIdx];
  stTheme.textContent = `theme: ${theme.name}`;

  const dynamic = state.engine.is_dynamic();
  stPanels.textContent = dynamic ? `panels: ${state.engine.panel_count()}` : '[fixed]';

  const kind = state.engine.focused_kind();
  stFocus.textContent = `focus: ${kind ?? 'none'}`;

  const diff = JSON.parse(state.engine.diff_counts());
  stDiff.textContent = `+${diff.added} -${diff.removed} ~${diff.resized} =${diff.unchanged} >${diff.moved}`;
}

function renderOverlays() {
  const eng = state.engine;
  if (!eng.helpVisible()) {
    overlayContainer.style.display = 'none';
    return;
  }

  overlayContainer.style.display = '';
  const overlays = JSON.parse(eng.resolveOverlays());

  // Position overlay divs from engine rects
  const activeIds = new Set(overlays.map(o => String(o.id)));
  for (const el of [...overlayContainer.querySelectorAll('.overlay')]) {
    if (!activeIds.has(el.dataset.overlayId)) el.remove();
  }

  // Build help content once (static data)
  const bindings = JSON.parse(helpBindings());
  let cardHtml = '<div class="overlay-card"><h2>Keybindings</h2><table>';
  for (const b of bindings) {
    cardHtml += `<tr><td class="overlay-key">${b.key}</td><td>${b.action}</td></tr>`;
  }
  cardHtml += '</table></div>';

  for (const o of overlays) {
    const key = String(o.id);
    let el = overlayContainer.querySelector(`[data-overlay-id="${key}"]`);
    if (!el) {
      el = document.createElement('div');
      el.className = 'overlay';
      el.dataset.overlayId = key;
      el.innerHTML = cardHtml;
      overlayContainer.appendChild(el);
    }
    el.style.left = `${o.x}px`;
    el.style.top = `${o.y}px`;
    el.style.width = `${o.w}px`;
    el.style.height = `${o.h}px`;
  }
}

function renderFrame(timestamp) {
  let t = 1.0;
  if (state.animStart !== null) {
    const elapsed = timestamp - state.animStart;
    t = Math.min(elapsed / ANIM_MS, 1);
    if (t >= 1) {
      state.animStart = null;
      state.animFrame = null;
      t = 1.0;
    }
  }

  const panels = resolveLayout(t);
  if (!panels) return;

  syncPanels(panels);
  renderOverlays();
  updateStatus();

  if (state.animStart !== null) {
    state.animFrame = requestAnimationFrame(renderFrame);
  }
}

function render() {
  if (state.animFrame) cancelAnimationFrame(state.animFrame);
  state.animFrame = requestAnimationFrame(renderFrame);
}

// -- Animation helpers --

function animatedAction(fn) {
  fn();
  state.animStart = performance.now();
  render();
}

// -- Events --

function handleKey(e) {
  const eng = state.engine;
  let handled = true;

  switch (e.key) {
    case 'ArrowRight':
      animatedAction(() => {
        state.presetIdx = (state.presetIdx + 1) % state.presets.length;
        eng.switch_preset(state.presets[state.presetIdx].name);
        presetSelect.value = state.presets[state.presetIdx].name;
      });
      break;
    case 'ArrowLeft':
      animatedAction(() => {
        state.presetIdx = (state.presetIdx + state.presets.length - 1) % state.presets.length;
        eng.switch_preset(state.presets[state.presetIdx].name);
        presetSelect.value = state.presets[state.presetIdx].name;
      });
      break;
    case 'ArrowDown':
      state.themeIdx = (state.themeIdx + 1) % state.themes.length;
      applyTheme();
      themeSelect.value = state.themes[state.themeIdx].id;
      render();
      break;
    case 'ArrowUp':
      state.themeIdx = (state.themeIdx + state.themes.length - 1) % state.themes.length;
      applyTheme();
      themeSelect.value = state.themes[state.themeIdx].id;
      render();
      break;
    case 'Tab':
      e.shiftKey ? eng.focus_prev() : eng.focus_next();
      render();
      break;
    case 'H':
      eng.focus_direction('left');
      render();
      break;
    case 'J':
      eng.focus_direction('down');
      render();
      break;
    case 'K':
      eng.focus_direction('up');
      render();
      break;
    case 'L':
      eng.focus_direction('right');
      render();
      break;
    case 'a':
      if (eng.is_dynamic()) {
        animatedAction(() => eng.add_panel());
      }
      break;
    case 'd':
      if (eng.is_dynamic()) {
        animatedAction(() => eng.remove_panel());
      }
      break;
    case 'c':
      animatedAction(() => eng.toggle_collapsed());
      break;
    case '[':
      animatedAction(() => eng.swap_prev());
      break;
    case ']':
      animatedAction(() => eng.swap_next());
      break;
    case '+':
      animatedAction(() => eng.resizeVertical(0.05));
      break;
    case '_':
      animatedAction(() => eng.resizeVertical(-0.05));
      break;
    case '=':
      animatedAction(() => eng.resizeHorizontal(0.05));
      break;
    case '-':
      animatedAction(() => eng.resizeHorizontal(-0.05));
      break;
    case '?':
      eng.toggleHelp();
      render();
      break;
    default:
      handled = false;
  }

  if (handled) e.preventDefault();
}

function viewportCoords(e) {
  const rect = viewport.getBoundingClientRect();
  return [e.clientX - rect.left, e.clientY - rect.top];
}

function cursorForBoundary(axis) {
  switch (axis) {
    case 'vertical': return 'col-resize';
    case 'horizontal': return 'row-resize';
    default: return '';
  }
}

function handleMouseDown(e) {
  const [x, y] = viewportCoords(e);
  const eng = state.engine;
  const boundary = eng.boundaryHover(x, y);
  if (boundary) {
    if (eng.dragStart(x, y)) {
      state.dragging = true;
      viewport.style.cursor = cursorForBoundary(boundary);
    }
  } else {
    eng.focusAt(x, y);
    render();
  }
}

function handleMouseMove(e) {
  const [x, y] = viewportCoords(e);
  const eng = state.engine;
  if (state.dragging) {
    eng.dragMove(x, y);
    render();
  } else {
    viewport.style.cursor = cursorForBoundary(eng.boundaryHover(x, y));
  }
}

function handleMouseUp() {
  if (!state.dragging) return;
  state.dragging = false;
  state.engine.dragEnd();
  viewport.style.cursor = '';
  render();
}

function handleWheel(e) {
  e.preventDefault();
  animatedAction(() => state.engine.scrollBy(e.deltaY > 0 ? 1.0 : -1.0));
}

function setupSelects() {
  for (const p of state.presets) {
    const opt = document.createElement('option');
    opt.value = p.name;
    opt.textContent = p.name;
    presetSelect.appendChild(opt);
  }
  presetSelect.value = state.presets[state.presetIdx].name;

  presetSelect.addEventListener('change', () => {
    const name = presetSelect.value;
    const idx = state.presets.findIndex(p => p.name === name);
    if (idx < 0) return;
    animatedAction(() => {
      state.presetIdx = idx;
      state.engine.switch_preset(name);
    });
  });

  for (const t of state.themes) {
    const opt = document.createElement('option');
    opt.value = t.id;
    opt.textContent = `${t.name} [${t.style}]`;
    themeSelect.appendChild(opt);
  }
  themeSelect.value = state.themes[state.themeIdx].id;

  themeSelect.addEventListener('change', () => {
    const id = themeSelect.value;
    const idx = state.themes.findIndex(t => t.id === id);
    if (idx < 0) return;
    state.themeIdx = idx;
    applyTheme();
    render();
  });
}

// -- Persistence --

const STORAGE_SNAPSHOT = 'p3-demo:snapshot';
const STORAGE_THEME = 'p3-demo:themeIdx';

function saveState() {
  try {
    const json = state.engine.snapshot();
    localStorage.setItem(STORAGE_SNAPSHOT, json);
    localStorage.setItem(STORAGE_THEME, String(state.themeIdx));
  } catch (_) {
    // Best-effort: localStorage may be unavailable or quota exceeded.
  }
}

function restoreState() {
  try {
    const json = localStorage.getItem(STORAGE_SNAPSHOT);
    if (!json) return;
    // Extract preset name to sync JS-side index before restoring.
    const parsed = JSON.parse(json);
    const presetName = parsed.preset;
    const presetIdx = state.presets.findIndex(p => p.name === presetName);
    if (presetIdx < 0) return;

    state.engine.restore(json);
    state.presetIdx = presetIdx;
    presetSelect.value = state.presets[presetIdx].name;

    const savedTheme = localStorage.getItem(STORAGE_THEME);
    if (savedTheme !== null) {
      const idx = Number(savedTheme);
      if (idx >= 0 && idx < state.themes.length) {
        state.themeIdx = idx;
        themeSelect.value = state.themes[idx].id;
      }
    }
    applyTheme();
  } catch (_) {
    // Snapshot invalid or engine rejected it — continue with defaults.
  }
}

// -- Init --

async function main() {
  await init();

  state.presets = JSON.parse(layoutPresets());
  state.engine = new JsLayoutEngine(state.presets[0].name, 8.0);

  state.themes = JSON.parse(state.engine.themeList());
  applyTheme();

  setupSelects();

  restoreState();

  document.addEventListener('keydown', handleKey);
  viewport.addEventListener('mousedown', handleMouseDown);
  viewport.addEventListener('mousemove', handleMouseMove);
  document.addEventListener('mouseup', handleMouseUp);
  viewport.addEventListener('wheel', handleWheel, { passive: false });
  window.addEventListener('beforeunload', saveState);

  new ResizeObserver(() => {
    render();
  }).observe(viewport);

  render();
}

main();
