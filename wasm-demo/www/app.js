import init, { JsLayoutEngine, layoutPresets } from './pkg/panes_wasm_demo.js';

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
  prevRects: null,
  targetRects: null,
  animStart: null,
  animFrame: null,
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

// -- Easing --

function easeOutCubic(t) {
  const inv = 1 - t;
  return 1 - inv * inv * inv;
}

function lerp(a, b, t) {
  return a + (b - a) * t;
}

// -- Theme --

function applyTheme() {
  const info = state.themes[state.themeIdx];
  themeCssEl.textContent = state.engine.themeCss(info.id);
}

// -- Rendering --

function resolveLayout() {
  const rect = viewport.getBoundingClientRect();
  if (rect.width === 0 || rect.height === 0) return null;
  const json = state.engine.resolve(rect.width, rect.height);
  return JSON.parse(json);
}

function syncPanels(panels, progress) {
  const activeIds = new Set(panels.map(p => String(p.id)));

  // Remove stale panels
  for (const el of [...viewport.children]) {
    if (!activeIds.has(el.dataset.panelId)) {
      el.remove();
    }
  }

  // Upsert panels
  panels.forEach((p, i) => {
    const key = String(p.id);
    let el = viewport.querySelector(`[data-panel-id="${key}"]`);
    if (!el) {
      el = document.createElement('div');
      el.className = 'panel';
      el.dataset.panelId = key;
      el.innerHTML = '<div class="panel-title"></div><div class="panel-body"></div>';
      viewport.appendChild(el);
    }

    // Interpolate position during animation (keyed by panel ID)
    let x = p.x, y = p.y, w = p.w, h = p.h;
    if (progress !== null && state.prevRects) {
      const prev = state.prevRects.get(key);
      if (prev) {
        const t = easeOutCubic(progress);
        x = lerp(prev.x, p.x, t);
        y = lerp(prev.y, p.y, t);
        w = lerp(prev.w, p.w, t);
        h = lerp(prev.h, p.h, t);
      }
    }

    el.style.left = `${x}px`;
    el.style.top = `${y}px`;
    el.style.width = `${w}px`;
    el.style.height = `${h}px`;

    el.classList.toggle('focused', p.focused);

    const accent = accentColor(p.kind_index);
    const title = el.firstElementChild;
    title.textContent = p.kind;
    title.style.color = p.focused ? accent : 'var(--fg)';
    title.style.background = p.focused ? 'var(--bg-hi, rgba(255,255,255,0.05))' : 'transparent';

    const body = el.lastElementChild;
    body.textContent = `${Math.round(w)}\u00d7${Math.round(h)}  (${Math.round(x)},${Math.round(y)})`;
  });
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

function renderFrame(timestamp) {
  let progress = null;
  if (state.animStart !== null) {
    const elapsed = timestamp - state.animStart;
    progress = Math.min(elapsed / ANIM_MS, 1);
    if (progress >= 1) {
      state.animStart = null;
      state.prevRects = null;
      state.targetRects = null;
      state.animFrame = null;
      progress = null;
    }
  }

  const panels = state.targetRects ?? resolveLayout();
  if (!panels) return;

  syncPanels(panels, progress);
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

function snapshotRects() {
  const panels = resolveLayout();
  if (!panels) return null;
  const map = new Map();
  for (const p of panels) {
    map.set(String(p.id), { x: p.x, y: p.y, w: p.w, h: p.h });
  }
  return map;
}

function animatedAction(fn) {
  state.prevRects = snapshotRects();
  fn();
  state.targetRects = resolveLayout();
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
    case '=':
      animatedAction(() => eng.resizeHorizontal(0.05));
      break;
    case '-':
      animatedAction(() => eng.resizeHorizontal(-0.05));
      break;
    default:
      handled = false;
  }

  if (handled) e.preventDefault();
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

// -- Init --

async function main() {
  await init();

  state.presets = JSON.parse(layoutPresets());
  state.engine = new JsLayoutEngine(state.presets[0].name, 1.0);

  state.themes = JSON.parse(state.engine.themeList());
  applyTheme();

  setupSelects();

  document.addEventListener('keydown', handleKey);

  new ResizeObserver(() => {
    state.targetRects = null;
    render();
  }).observe(viewport);

  render();
}

main();
