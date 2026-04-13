use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::io::Write as _;
use std::process::ExitCode;
use std::sync::Arc;
use std::{env, fs, io, path};

use demo_presets::{build_css_dashboard_with_overlays, text_on_color};

use palette_core::color::Color;
use palette_core::contrast::ContrastLevel;
use palette_core::css::css_name;
use palette_core::gradient::{ColorSpace, Gradient, GradientStop};
use palette_core::palette::Palette;
use palette_core::registry::{Registry, ThemeInfo};
use palette_core::resolved::ResolvedPalette;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(thiserror::Error, Debug)]
enum Error {
    #[error("failed to load theme '{0}': {1}")]
    ThemeLoad(Arc<str>, palette_core::PaletteError),

    #[error("registry contains no themes")]
    NoThemes,

    #[error("{0}")]
    Io(#[from] io::Error),

    #[error("layout build failed: {0}")]
    Layout(#[from] panes::PaneError),
}

// ---------------------------------------------------------------------------
// Theme helpers
// ---------------------------------------------------------------------------

/// Write one CSS variable declaration per slot.
/// `prefix` is the var prefix (e.g. `"--"` or `"--text-on-"`).
/// `value_fn` maps each color to the value string.
fn write_css_vars<'a>(
    out: &mut String,
    section: &str,
    slots: impl Iterator<Item = (&'static str, &'a Color)>,
    prefix: &str,
    value_fn: fn(&Color) -> String,
) {
    for (field, color) in slots {
        let slot = match css_name(section, field) {
            Some(name) => name.to_string(),
            None => format!("{section}-{}", field.replace('_', "-")),
        };
        let _ = writeln!(out, "  {prefix}{slot}: {};", value_fn(color));
    }
}

fn color_value(color: &Color) -> String {
    color.to_string()
}

fn contrast_value(color: &Color) -> String {
    text_on_color(color).to_string()
}

/// Iterate all 8 palette sections, calling `emit` for each (section_name, slots).
macro_rules! for_each_section {
    ($palette:expr, $slot_method:ident, $emit:expr) => {{
        let p = $palette;
        $emit("base", p.base.$slot_method());
        $emit("semantic", p.semantic.$slot_method());
        $emit("diff", p.diff.$slot_method());
        $emit("surface", p.surface.$slot_method());
        $emit("typography", p.typography.$slot_method());
        $emit("syntax", p.syntax.$slot_method());
        $emit("editor", p.editor.$slot_method());
        $emit("terminal", p.terminal.$slot_method());
    }};
}

fn contrast_css(palette: &Palette) -> String {
    let mut out = String::with_capacity(2048);
    for_each_section!(palette, populated_slots, |section, slots| {
        write_css_vars(&mut out, section, slots, "--text-on-", contrast_value);
    });
    out
}

fn write_gradient_vars(out: &mut String, gradients: &[(Box<str>, Box<str>)]) {
    for (name, gradient) in gradients {
        let _ = writeln!(out, "  --gradient-{name}: {gradient};");
    }
}

fn build_gradient(stops: [Color; 3], space: ColorSpace) -> Option<Gradient> {
    Gradient::new(
        [
            GradientStop {
                color: stops[0],
                position: 0.0,
            },
            GradientStop {
                color: stops[1],
                position: 0.5,
            },
            GradientStop {
                color: stops[2],
                position: 1.0,
            },
        ],
        space,
    )
    .ok()
}

fn resolved_gradient_entries(resolved: &ResolvedPalette) -> Vec<(Box<str>, Box<str>)> {
    let mut entries: Vec<(Box<str>, Box<str>)> = resolved
        .gradients()
        .map(|(name, gradient)| (Box::<str>::from(name), gradient.to_css()))
        .collect();

    let derived = [
        (
            "demo-spectrum",
            build_gradient(
                [
                    resolved.base.background,
                    resolved.surface.focus,
                    resolved.typography.link,
                ],
                ColorSpace::OkLab,
            ),
        ),
        (
            "demo-heat",
            build_gradient(
                [
                    resolved.semantic.success,
                    resolved.semantic.warning,
                    resolved.semantic.error,
                ],
                ColorSpace::OkLch,
            ),
        ),
        (
            "demo-depth",
            build_gradient(
                [
                    resolved.base.background_dark,
                    resolved.base.background_highlight,
                    resolved.surface.overlay,
                ],
                ColorSpace::OkLab,
            ),
        ),
    ];

    for (name, gradient) in derived {
        if let Some(gradient) = gradient {
            entries.push((Box::<str>::from(name), gradient.to_css()));
        }
    }

    entries.sort_by(|a, b| a.0.cmp(&b.0));
    entries
}

fn resolved_to_css_scoped(resolved: &ResolvedPalette, selector: &str) -> String {
    let mut decls = String::with_capacity(4096);
    for_each_section!(resolved, all_slots, |section, slots| {
        write_css_vars(&mut decls, section, slots, "--", color_value);
    });
    for_each_section!(resolved, all_slots, |section, slots| {
        write_css_vars(&mut decls, section, slots, "--text-on-", contrast_value);
    });
    let gradients = resolved_gradient_entries(resolved);
    write_gradient_vars(&mut decls, &gradients);
    format!("{selector} {{\n{decls}}}\n")
}

/// Splice `extra` into `css` just before the final `}`, or append if none found.
fn splice_before_closing_brace(css: &mut String, extra: &str) {
    match css.rfind('}') {
        Some(pos) => css.insert_str(pos, extra),
        None => css.push_str(extra),
    }
}

/// Write the text-on counterpart of a CSS var into `buf`.
///
/// Reuses the buffer to avoid per-swatch allocation in tight loops.
fn write_text_on_var(buf: &mut String, var: &str) {
    buf.clear();
    let rest = var.strip_prefix("--").unwrap_or(var);
    buf.push_str("--text-on-");
    buf.push_str(rest);
}

/// Emit a single color swatch `<div>` with background/text vars.
fn write_swatch(out: &mut String, class: &str, bg_var: &str, text_var: &str, label: &str) {
    let _ = writeln!(
        out,
        "            <div class=\"{class}\" style=\"background: var({bg_var}); \
         color: var({text_var});\"><span>{label}</span></div>"
    );
}

fn gradient_css(resolved: &ResolvedPalette) -> String {
    let mut out = String::new();
    let gradients = resolved_gradient_entries(resolved);
    write_gradient_vars(&mut out, &gradients);
    out
}

fn collect_gradient_names(
    registry: &Registry,
    themes: &[ThemeInfo],
) -> Result<Box<[Box<str>]>, Error> {
    let mut names = BTreeSet::new();
    for info in themes {
        let palette = registry
            .load(&info.id)
            .map_err(|e| Error::ThemeLoad(Arc::clone(&info.id), e))?;
        for (name, _) in resolved_gradient_entries(&palette.resolve()) {
            names.insert(name);
        }
    }
    Ok(names.into_iter().collect())
}

fn generate_theme_css(registry: &Registry, info: &ThemeInfo) -> Result<String, Error> {
    let palette = registry
        .load(&info.id)
        .map_err(|e| Error::ThemeLoad(Arc::clone(&info.id), e))?;
    let selector = format!("[data-theme=\"{}\"]", info.id);
    let mut css = palette.to_css_scoped(&selector, None);
    splice_before_closing_brace(&mut css, &gradient_css(&palette.resolve()));
    splice_before_closing_brace(&mut css, &contrast_css(&palette));
    // WCAG AA-adjusted variant
    let adjusted = palette.resolve_with_contrast(ContrastLevel::AaNormal);
    let wcag_selector = format!("[data-wcag][data-theme=\"{}\"]", info.id);
    css.push_str(&resolved_to_css_scoped(&adjusted, &wcag_selector));
    Ok(css)
}

fn generate_theme_options(registry: &Registry, themes: &[ThemeInfo]) -> String {
    let mut dark = Vec::new();
    let mut light = Vec::new();
    for t in themes {
        let opt = format!("        <option value=\"{}\">{}</option>", t.id, t.name);
        let light_theme = registry
            .load(&t.id)
            .map(|p| p.resolve().is_light())
            .unwrap_or(false);
        match light_theme {
            true => light.push(opt),
            false => dark.push(opt),
        }
    }

    let mut html = String::from("      <optgroup label=\"Dark\">\n");
    for opt in &dark {
        html.push_str(opt);
        html.push('\n');
    }
    html.push_str("      </optgroup>\n");
    html.push_str("      <optgroup label=\"Light\">\n");
    for opt in &light {
        html.push_str(opt);
        html.push('\n');
    }
    html.push_str("      </optgroup>");
    html
}

// ---------------------------------------------------------------------------
// CSS
// ---------------------------------------------------------------------------

fn generate_css(registry: &Registry, themes: &[ThemeInfo]) -> Result<String, Error> {
    let mut css = String::with_capacity(64 * 1024);

    // -- Reset & component styles --
    css.push_str(COMPONENT_CSS);

    // -- panes-css layout + overlay positioning --
    let (layout, overlay_defs) = build_css_dashboard_with_overlays()?;
    css.push_str(&panes_css::emit_full(&layout, &overlay_defs));

    // -- First theme as :root default (renders before JS) --
    let first = registry
        .load(&themes[0].id)
        .map_err(|e| Error::ThemeLoad(Arc::clone(&themes[0].id), e))?;
    let mut root_css = first.to_css();
    splice_before_closing_brace(&mut root_css, &gradient_css(&first.resolve()));
    splice_before_closing_brace(&mut root_css, &contrast_css(&first));
    css.push_str(&root_css);
    css.push('\n');

    // -- WCAG AA-adjusted :root default --
    let adjusted_root = first.resolve_with_contrast(ContrastLevel::AaNormal);
    css.push_str(&resolved_to_css_scoped(&adjusted_root, "[data-wcag]"));
    css.push('\n');

    // -- Per-theme scoped blocks --
    for info in themes {
        let block = generate_theme_css(registry, info)?;
        css.push_str(&block);
        css.push('\n');
    }

    Ok(css)
}

const COMPONENT_CSS: &str = r#"
html:not(.loaded) * { transition: none !important; }

*, *::before, *::after { box-sizing: border-box; margin: 0; padding: 0; }

body {
  font-family: system-ui, -apple-system, sans-serif;
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
  background: var(--bg);
  background-image: radial-gradient(ellipse at 50% 0%, color-mix(in srgb, var(--ui-focus) 3%, transparent) 0%, transparent 70%);
  color: var(--fg);
  transition: background 0.3s, color 0.3s, background-image 0.3s ease;
  line-height: 1.5;
}

/* Header */
.header {
  position: sticky;
  top: 0;
  z-index: 100;
  background: var(--ui-statusline);
  background: color-mix(in srgb, var(--ui-statusline) 80%, transparent);
  -webkit-backdrop-filter: blur(12px);
  backdrop-filter: blur(12px);
  border-bottom: 1px solid var(--border);
  border-bottom: 1px solid color-mix(in srgb, var(--border) 60%, transparent);
  padding: 1rem 1.5rem;
  display: flex;
  align-items: center;
  gap: 1rem;
  transition: background 0.3s;
}
.header h1 {
  font-size: 1.1rem;
  font-weight: 700;
  letter-spacing: -0.02em;
  color: var(--text-title);
}
.header select {
  background: var(--bg-dark);
  color: var(--fg);
  border: 1px solid var(--border-hi);
  border-radius: 4px;
  padding: 0.3rem 0.5rem;
  font-size: 0.85rem;
  cursor: pointer;
}
.header select:focus { outline: 2px solid var(--ui-focus); }
.wcag-toggle {
  display: flex;
  align-items: center;
  gap: 0.4rem;
  font-size: 0.85rem;
  cursor: pointer;
  margin-left: auto;
}
.wcag-toggle input {
  opacity: 0;
  width: 0;
  height: 0;
  position: absolute;
}
.toggle-track {
  width: 36px;
  height: 20px;
  background: var(--border-hi);
  border-radius: 10px;
  position: relative;
  transition: background 0.25s;
  flex-shrink: 0;
}
.toggle-knob {
  width: 16px;
  height: 16px;
  background: var(--fg);
  border-radius: 50%;
  position: absolute;
  top: 2px;
  left: 2px;
  transition: transform 0.25s;
}
.wcag-toggle input:checked + .toggle-track {
  background: var(--ui-focus);
}
.wcag-toggle input:checked + .toggle-track .toggle-knob {
  transform: translateX(16px);
}
.wcag-toggle input:focus-visible + .toggle-track {
  outline: 2px solid var(--ui-focus);
  outline-offset: 2px;
}

/* Help toggle button */
.help-btn {
  width: 32px;
  height: 32px;
  border-radius: 50%;
  border: 1px solid var(--border-hi);
  background: var(--bg-dark);
  color: var(--fg);
  font-size: 1rem;
  font-weight: 700;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: background 0.2s, border-color 0.2s;
}
.help-btn:hover { background: var(--bg-hi); }
.help-btn:focus-visible { outline: 2px solid var(--ui-focus); outline-offset: 2px; }

/* Help overlay content (positioning from panes-css) */
[data-pane-overlay] {
  transition: opacity 0.2s;
}
[data-overlay-visible="false"] {
  opacity: 0;
  pointer-events: none;
}
[data-overlay-visible="true"] {
  opacity: 1;
}
.help-content {
  background: var(--ui-float);
  border: 1px solid var(--border-hi);
  border-radius: 10px;
  padding: 1.5rem;
  box-shadow: 0 8px 32px rgba(0,0,0,0.24);
  color: var(--fg);
  max-height: 80vh;
  overflow-y: auto;
}
.help-content h2 {
  font-size: 1.1rem;
  font-weight: 600;
  color: var(--text-title);
  margin-bottom: 1rem;
}
.help-table {
  width: 100%;
  border-collapse: collapse;
}
.help-table td {
  padding: 0.4rem 0.75rem;
  border-bottom: 1px solid var(--border);
  font-size: 0.9rem;
}
.help-table td:first-child {
  white-space: nowrap;
  width: 1%;
}
.help-table kbd {
  display: inline-block;
  padding: 0.15rem 0.5rem;
  background: var(--bg-hi);
  border: 1px solid var(--border);
  border-radius: 4px;
  font-family: 'SF Mono', 'Fira Code', monospace;
  font-size: 0.85rem;
}

/* Root layout cosmetics (grid rules emitted by panes-css) */
[data-pane-root] {
  max-width: 1600px;
  margin: 0 auto;
  padding: 1.5rem;
}
/* Cards */
.card {
  background: var(--bg-dark);
  border: 1px solid transparent;
  border-radius: 10px;
  padding: 1.25rem;
  box-shadow: 0 1px 3px rgba(0,0,0,0.08), 0 4px 12px rgba(0,0,0,0.04);
  transition: background 0.3s, border-color 0.3s, transform 0.25s ease, box-shadow 0.25s ease;
}
.card:hover {
  transform: translateY(-2px);
  box-shadow: 0 4px 12px rgba(0,0,0,0.12), 0 8px 24px rgba(0,0,0,0.08);
}
@media (prefers-reduced-motion: reduce) {
  .card { transition: background 0.3s, border-color 0.3s; }
  .card:hover { transform: none; }
}
.card h2 {
  font-size: 1.25rem;
  font-weight: 600;
  letter-spacing: -0.01em;
  color: var(--text-title);
  margin-bottom: 0.75rem;
  padding-bottom: 0.5rem;
  border-bottom: 1px solid var(--border);
}

/* Swatches */
.swatches {
  display: flex;
  flex-wrap: wrap;
  gap: 0.5rem;
}
.swatch {
  width: 88px;
  height: 64px;
  border-radius: 6px;
  box-shadow: inset 0 0 0 1px rgba(128,128,128,0.25);
  overflow: hidden;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: flex-end;
  padding: 3px;
  position: relative;
}
.swatch span {
  font-size: 0.7rem;
  text-align: center;
  line-height: 1.1;
  word-break: break-all;
}
.swatch .hex {
  font-size: 0.6rem;
  opacity: 0;
  transform: translateY(4px);
  transition: opacity 0.2s, transform 0.2s;
  font-family: monospace;
}
.swatch:hover .hex {
  opacity: 0.9;
  transform: translateY(0);
}

/* Code block */
pre.code {
  background: var(--bg-dark);
  border: none;
  border-left: 3px solid var(--ui-focus);
  border-radius: 6px;
  padding: 1rem 1rem 1rem 1.25rem;
  overflow-x: auto;
  font-family: 'SF Mono', 'Fira Code', 'Cascadia Code', monospace;
  font-size: 0.9rem;
  line-height: 1.7;
  tab-size: 4;
}

/* Syntax token classes */
.syn-keyword { color: var(--syn-keyword); }
.syn-keyword-fn { color: var(--syn-keyword-fn); }
.syn-keyword-ctrl { color: var(--syn-keyword-ctrl); }
.syn-keyword-import { color: var(--syn-keyword-import); }
.syn-keyword-op { color: var(--syn-keyword-op); }
.syn-fn { color: var(--syn-fn); }
.syn-fn-builtin { color: var(--syn-fn-builtin); }
.syn-fn-method { color: var(--syn-fn-method); }
.syn-fn-macro { color: var(--syn-fn-macro); }
.syn-var { color: var(--syn-var); }
.syn-var-builtin { color: var(--syn-var-builtin); }
.syn-param { color: var(--syn-param); }
.syn-prop { color: var(--syn-prop); }
.syn-type { color: var(--syn-type); }
.syn-type-builtin { color: var(--syn-type-builtin); }
.syn-const { color: var(--syn-const); }
.syn-const-char { color: var(--syn-const-char); }
.syn-number { color: var(--syn-number); }
.syn-bool { color: var(--syn-bool); }
.syn-string { color: var(--syn-string); }
.syn-string-doc { color: var(--syn-string-doc); }
.syn-string-esc { color: var(--syn-string-esc); }
.syn-string-re { color: var(--syn-string-re); }
.syn-op { color: var(--syn-op); }
.syn-punct { color: var(--syn-punct); }
.syn-punct-bracket { color: var(--syn-punct-bracket); }
.syn-punct-special { color: var(--syn-punct-special); }
.syn-annotation { color: var(--syn-annotation); }
.syn-attr { color: var(--syn-attr); }
.syn-attr-builtin { color: var(--syn-attr-builtin); }
.syn-ctor { color: var(--syn-ctor); }
.syn-module { color: var(--syn-module); }
.syn-label { color: var(--syn-label); }
.syn-comment { color: var(--syn-comment); font-style: italic; }
.syn-comment-doc { color: var(--syn-comment-doc); font-style: italic; }
.syn-tag { color: var(--syn-tag); }
.syn-tag-delim { color: var(--syn-tag-delim); }
.syn-tag-attr { color: var(--syn-tag-attr); }

/* Diff */
.diff-line {
  padding: 0.15rem 0.75rem;
  font-family: 'SF Mono', 'Fira Code', monospace;
  font-size: 0.9rem;
  white-space: pre;
}
.diff-added { background: var(--diff-added-bg); color: var(--diff-added-fg); }
.diff-removed { background: var(--diff-removed-bg); color: var(--diff-removed-fg); }
.diff-modified { background: var(--diff-modified-bg); color: var(--diff-modified-fg); }
.diff-context { background: var(--diff-text-bg); color: var(--fg); }
.diff-hunk {
  border-radius: 6px;
  overflow: hidden;
  border: 1px solid var(--border);
}

/* ANSI grid — 8 columns so normal/bright wrap into two rows */
.ansi-grid {
  display: grid;
  grid-template-columns: repeat(8, 1fr);
  gap: 8px;
}
.ansi-cell {
  height: 56px;
  border-radius: 6px;
  display: flex;
  align-items: flex-end;
  justify-content: center;
  padding: 6px;
}
.ansi-cell span {
  font-size: 0.75rem;
  text-align: center;
}

/* Editor swatches */
.editor-row {
  display: flex;
  flex-wrap: wrap;
  gap: 0.75rem;
  margin-bottom: 1rem;
}
.editor-swatch {
  min-width: 96px;
  height: 48px;
  border-radius: 6px;
  border: 1px solid var(--border);
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 4px 8px;
  font-size: 0.8rem;
}

"#;

// ---------------------------------------------------------------------------
// HTML body
// ---------------------------------------------------------------------------

fn body_content(theme_options: &str, gradient_names: &[Box<str>]) -> String {
    let mut html = String::with_capacity(16 * 1024);

    // Header
    let _ = write!(
        html,
        r#"  <header class="header">
    <h1>palette-core CSS Theme Showcase</h1>
    <label for="theme-select">Theme:</label>
    <select id="theme-select">
{theme_options}
    </select>
    <label class="wcag-toggle">
      <input type="checkbox" id="wcag-toggle">
      <span class="toggle-track"><span class="toggle-knob"></span></span>
      <span>WCAG AA</span>
    </label>
    <button class="help-btn" id="help-toggle" aria-label="Toggle help">?</button>
  </header>
  <main data-pane-root>
    <div data-pane-node="1">
"#
    );

    // Cards wrapped in their pane-node containers (nodes 2-10, depth-first)
    let cards: Vec<String> = vec![
        swatch_card(
            "Base Colors",
            "base-colors",
            &[
                ("--bg", "bg"),
                ("--bg-dark", "bg-dark"),
                ("--bg-hi", "bg-hi"),
                ("--fg", "fg"),
                ("--fg-dark", "fg-dark"),
                ("--border", "border"),
                ("--border-hi", "border-hi"),
            ],
        ),
        swatch_card(
            "Semantic Colors",
            "semantic-colors",
            &[
                ("--success", "success"),
                ("--warning", "warning"),
                ("--error", "error"),
                ("--info", "info"),
                ("--hint", "hint"),
            ],
        ),
        swatch_card(
            "Surface Colors",
            "surface-colors",
            &[
                ("--ui-menu", "menu"),
                ("--ui-sidebar", "sidebar"),
                ("--ui-statusline", "statusline"),
                ("--ui-float", "float"),
                ("--ui-popup", "popup"),
                ("--ui-overlay", "overlay"),
                ("--ui-hi", "hi"),
                ("--ui-sel", "sel"),
                ("--ui-focus", "focus"),
                ("--ui-search", "search"),
            ],
        ),
        gradient_card(gradient_names),
        swatch_card(
            "Typography",
            "typography",
            &[
                ("--text-comment", "comment"),
                ("--text-gutter", "gutter"),
                ("--text-line-num", "line-num"),
                ("--text-sel", "sel"),
                ("--text-link", "link"),
                ("--text-title", "title"),
            ],
        ),
        syntax_card(),
        editor_card(),
        diff_card(),
        ansi_card(),
    ];

    for (i, card) in cards.iter().enumerate() {
        let node_id = i + 2; // nodes 2 through 10
        let _ = write!(
            html,
            "      <div data-pane-node=\"{node_id}\">\n{card}      </div>\n"
        );
    }

    html.push_str("    </div>\n  </main>\n");

    // Help overlay
    let _ = write!(
        html,
        r#"  <div data-pane-overlay="help" data-overlay-visible="false">
    <div class="help-content">
      <h2>Keyboard Shortcuts</h2>
      <table class="help-table">
        <tr><td><kbd>←</kbd> <kbd>→</kbd></td><td>Previous / Next theme</td></tr>
        <tr><td><kbd>W</kbd></td><td>Toggle WCAG AA</td></tr>
        <tr><td><kbd>?</kbd></td><td>Toggle this help</td></tr>
      </table>
    </div>
  </div>
"#
    );

    html
}

fn swatch_card(title: &str, pane_name: &str, swatches: &[(&str, &str)]) -> String {
    let mut html = String::new();
    let _ = write!(
        html,
        "        <section class=\"card\" data-pane=\"{pane_name}\">\n          <h2>{title}</h2>\n          <div class=\"swatches\">\n"
    );
    let mut text_var = String::new();
    for (var, label) in swatches {
        write_text_on_var(&mut text_var, var);
        write_swatch(&mut html, "swatch", var, &text_var, label);
    }
    html.push_str("          </div>\n        </section>\n");
    html
}

fn gradient_card(gradient_names: &[Box<str>]) -> String {
    let mut html = String::from(
        "        <section class=\"card\" data-pane=\"gradients\">\n          <h2>Gradients</h2>\n          <div class=\"swatches\">\n",
    );
    match gradient_names.is_empty() {
        true => html.push_str(
            "            <p style=\"color:var(--text-comment);\">This theme set does not define any gradients.</p>\n",
        ),
        false => {
            for name in gradient_names {
                let var = format!("--gradient-{name}");
                write_swatch(&mut html, "swatch", &var, "--fg", name);
            }
        }
    }
    html.push_str("          </div>\n        </section>\n");
    html
}

fn syntax_card() -> String {
    let mut html = String::from(
        "        <section class=\"card\" data-pane=\"syntax\">\n          <h2>Syntax Highlighting</h2>\n",
    );
    html.push_str(&code_snippet());
    html.push_str("          <h2 style=\"margin-top: 1rem;\">HTML Tags</h2>\n");
    html.push_str(&html_snippet());
    html.push_str("        </section>\n");
    html
}

fn code_snippet() -> String {
    // Rust code covering all syntax token types
    format!(
        r##"          <pre class="code">{}</pre>
"##,
        [
            r#"<span class="syn-comment-doc">/// A documented struct covering all token types.</span>"#,
            r#"<span class="syn-annotation">#</span><span class="syn-punct-bracket">[</span><span class="syn-attr-builtin">derive</span><span class="syn-punct-bracket">(</span><span class="syn-type">Debug</span><span class="syn-punct">,</span> <span class="syn-type">Clone</span><span class="syn-punct-bracket">)</span><span class="syn-punct-bracket">]</span>"#,
            r#"<span class="syn-keyword">pub</span> <span class="syn-keyword">struct</span> <span class="syn-type">Config</span> <span class="syn-punct-bracket">{</span>"#,
            r#"    <span class="syn-prop">name</span><span class="syn-punct">:</span> <span class="syn-type-builtin">String</span><span class="syn-punct">,</span>"#,
            r#"    <span class="syn-prop">count</span><span class="syn-punct">:</span> <span class="syn-type-builtin">usize</span><span class="syn-punct">,</span>"#,
            r#"    <span class="syn-prop">enabled</span><span class="syn-punct">:</span> <span class="syn-type-builtin">bool</span><span class="syn-punct">,</span>"#,
            r#"<span class="syn-punct-bracket">}</span>"#,
            r#""#,
            r#"<span class="syn-comment">// Regular comment</span>"#,
            r#"<span class="syn-keyword">const</span> <span class="syn-const">MAX_RETRIES</span><span class="syn-punct">:</span> <span class="syn-type-builtin">u32</span> <span class="syn-op">=</span> <span class="syn-number">42</span><span class="syn-punct">;</span>"#,
            r#""#,
            r#"<span class="syn-keyword">impl</span> <span class="syn-type">Config</span> <span class="syn-punct-bracket">{</span>"#,
            r#"    <span class="syn-keyword-fn">fn</span> <span class="syn-fn">new</span><span class="syn-punct-bracket">(</span><span class="syn-param">name</span><span class="syn-punct">:</span> <span class="syn-op">&amp;</span><span class="syn-type-builtin">str</span><span class="syn-punct">,</span> <span class="syn-param">count</span><span class="syn-punct">:</span> <span class="syn-type-builtin">usize</span><span class="syn-punct-bracket">)</span> <span class="syn-op">-&gt;</span> <span class="syn-var-builtin">Self</span> <span class="syn-punct-bracket">{</span>"#,
            r#"        <span class="syn-var-builtin">Self</span> <span class="syn-punct-bracket">{</span>"#,
            r#"            <span class="syn-prop">name</span><span class="syn-punct">:</span> <span class="syn-param">name</span><span class="syn-punct">.</span><span class="syn-fn-method">to_string</span><span class="syn-punct-bracket">(</span><span class="syn-punct-bracket">)</span><span class="syn-punct">,</span>"#,
            r#"            <span class="syn-prop">count</span><span class="syn-punct">,</span>"#,
            r#"            <span class="syn-prop">enabled</span><span class="syn-punct">:</span> <span class="syn-bool">true</span><span class="syn-punct">,</span>"#,
            r#"        <span class="syn-punct-bracket">}</span>"#,
            r#"    <span class="syn-punct-bracket">}</span>"#,
            r#"<span class="syn-punct-bracket">}</span>"#,
            r#""#,
            r#"<span class="syn-keyword-import">use</span> <span class="syn-module">std</span><span class="syn-punct-special">::</span><span class="syn-module">collections</span><span class="syn-punct-special">::</span><span class="syn-type">HashMap</span><span class="syn-punct">;</span>"#,
            r#""#,
            r#"<span class="syn-keyword-fn">fn</span> <span class="syn-fn">process</span><span class="syn-punct-bracket">(</span><span class="syn-param">input</span><span class="syn-punct">:</span> <span class="syn-op">&amp;</span><span class="syn-type-builtin">str</span><span class="syn-punct-bracket">)</span> <span class="syn-op">-&gt;</span> <span class="syn-type">Option</span><span class="syn-punct-bracket">&lt;</span><span class="syn-type-builtin">String</span><span class="syn-punct-bracket">&gt;</span> <span class="syn-punct-bracket">{</span>"#,
            r#"    <span class="syn-keyword">let</span> <span class="syn-var">re</span> <span class="syn-op">=</span> <span class="syn-fn-macro">regex!</span><span class="syn-punct-bracket">(</span><span class="syn-string-re">r"(\w+)-(\d+)"</span><span class="syn-punct-bracket">)</span><span class="syn-punct">;</span>"#,
            r#"    <span class="syn-keyword">let</span> <span class="syn-var">msg</span> <span class="syn-op">=</span> <span class="syn-fn-builtin">format!</span><span class="syn-punct-bracket">(</span><span class="syn-string">"hello </span><span class="syn-string-esc">{}</span><span class="syn-string">"</span><span class="syn-punct">,</span> <span class="syn-param">input</span><span class="syn-punct-bracket">)</span><span class="syn-punct">;</span>"#,
            r#"    <span class="syn-keyword">let</span> <span class="syn-var">ch</span> <span class="syn-op">=</span> <span class="syn-const-char">'x'</span><span class="syn-punct">;</span>"#,
            r#"    <span class="syn-keyword">let</span> <span class="syn-keyword-op">_</span> <span class="syn-op">=</span> <span class="syn-punct-bracket">(</span><span class="syn-var">re</span><span class="syn-punct">,</span> <span class="syn-var">ch</span><span class="syn-punct-bracket">)</span><span class="syn-punct">;</span>"#,
            r#"    <span class="syn-keyword-ctrl">if</span> <span class="syn-param">input</span><span class="syn-punct">.</span><span class="syn-fn-method">is_empty</span><span class="syn-punct-bracket">(</span><span class="syn-punct-bracket">)</span> <span class="syn-punct-bracket">{</span>"#,
            r#"        <span class="syn-keyword-ctrl">return</span> <span class="syn-ctor">None</span><span class="syn-punct">;</span>"#,
            r#"    <span class="syn-punct-bracket">}</span>"#,
            r#"    <span class="syn-label">'outer</span><span class="syn-punct">:</span> <span class="syn-keyword-ctrl">loop</span> <span class="syn-punct-bracket">{</span>"#,
            r#"        <span class="syn-keyword-ctrl">break</span> <span class="syn-label">'outer</span><span class="syn-punct">;</span>"#,
            r#"    <span class="syn-punct-bracket">}</span>"#,
            r#"    <span class="syn-ctor">Some</span><span class="syn-punct-bracket">(</span><span class="syn-var">msg</span><span class="syn-punct-bracket">)</span>"#,
            r#"<span class="syn-punct-bracket">}</span>"#,
            r#""#,
            r#"<span class="syn-string-doc">/// Documentation comment for the trait</span>"#,
            r#"<span class="syn-attr">#[</span><span class="syn-annotation">allow</span><span class="syn-punct-bracket">(</span><span class="syn-attr">unused</span><span class="syn-punct-bracket">)</span><span class="syn-attr">]</span>"#,
            r#"<span class="syn-keyword">trait</span> <span class="syn-type">Render</span> <span class="syn-punct-bracket">{</span>"#,
            r#"    <span class="syn-keyword-fn">fn</span> <span class="syn-fn">draw</span><span class="syn-punct-bracket">(</span><span class="syn-op">&amp;</span><span class="syn-var-builtin">self</span><span class="syn-punct-bracket">)</span> <span class="syn-op">-&gt;</span> <span class="syn-type-builtin">bool</span> <span class="syn-punct-bracket">{</span> <span class="syn-bool">false</span> <span class="syn-punct-bracket">}</span>"#,
            r#"<span class="syn-punct-bracket">}</span>"#,
        ]
        .join("\n")
    )
}

fn html_snippet() -> String {
    format!(
        r##"          <pre class="code">{}</pre>
"##,
        [
            r#"<span class="syn-tag-delim">&lt;</span><span class="syn-tag">div</span> <span class="syn-tag-attr">class</span><span class="syn-op">=</span><span class="syn-string">"container"</span><span class="syn-tag-delim">&gt;</span>"#,
            r##"  <span class="syn-tag-delim">&lt;</span><span class="syn-tag">a</span> <span class="syn-tag-attr">href</span><span class="syn-op">=</span><span class="syn-string">"#link"</span><span class="syn-tag-delim">&gt;</span>click<span class="syn-tag-delim">&lt;/</span><span class="syn-tag">a</span><span class="syn-tag-delim">&gt;</span>"##,
            r#"<span class="syn-tag-delim">&lt;/</span><span class="syn-tag">div</span><span class="syn-tag-delim">&gt;</span>"#,
        ]
        .join("\n")
    )
}

fn editor_card() -> String {
    let mut html = String::from(
        "        <section class=\"card\" data-pane=\"editor\">\n          <h2>Editor</h2>\n",
    );

    let groups: &[(&str, &[(&str, &str)])] = &[
        (
            "Cursor & Selection",
            &[
                ("--ed-cursor", "cursor"),
                ("--ed-cursor-text", "cursor-text"),
                ("--ed-match-paren", "paren"),
                ("--ed-sel-bg", "sel-bg"),
                ("--ed-sel-fg", "sel-fg"),
            ],
        ),
        (
            "Search",
            &[
                ("--ed-search-bg", "search-bg"),
                ("--ed-search-fg", "search-fg"),
            ],
        ),
        (
            "Hints",
            &[("--ed-hint-bg", "hint-bg"), ("--ed-hint-fg", "hint-fg")],
        ),
        (
            "Diagnostics",
            &[
                ("--ed-diag-error", "error"),
                ("--ed-diag-warn", "warn"),
                ("--ed-diag-info", "info"),
                ("--ed-diag-hint", "hint"),
            ],
        ),
        (
            "Diagnostic Underlines",
            &[
                ("--ed-diag-ul-error", "ul-error"),
                ("--ed-diag-ul-warn", "ul-warn"),
                ("--ed-diag-ul-info", "ul-info"),
                ("--ed-diag-ul-hint", "ul-hint"),
            ],
        ),
    ];

    let mut text_var = String::new();
    for (label, swatches) in groups {
        let _ = write!(
            html,
            "          <p style=\"font-size:0.85rem; color:var(--text-comment); margin:0.5rem 0 0.25rem;\">{label}</p>\n          <div class=\"editor-row\">\n"
        );
        for (var, name) in *swatches {
            write_text_on_var(&mut text_var, var);
            write_swatch(&mut html, "editor-swatch", var, &text_var, name);
        }
        html.push_str("          </div>\n");
    }

    html.push_str("        </section>\n");
    html
}

fn diff_card() -> String {
    let mut html = String::from(
        "        <section class=\"card\" data-pane=\"diff\">\n          <h2>Diff</h2>\n",
    );

    // Mock diff hunk
    html.push_str("          <div class=\"diff-hunk\">\n");
    html.push_str("            <div class=\"diff-line diff-context\"> fn main() {</div>\n");
    html.push_str("            <div class=\"diff-line diff-removed\">-    let x = 1;</div>\n");
    html.push_str("            <div class=\"diff-line diff-added\">+    let x = 42;</div>\n");
    html.push_str("            <div class=\"diff-line diff-modified\">~    let y = x + 1;</div>\n");
    html.push_str("            <div class=\"diff-line diff-context\"> }</div>\n");
    html.push_str("          </div>\n");

    // Diff swatches
    html.push_str("          <div class=\"swatches\" style=\"margin-top: 0.75rem;\">\n");
    let vars = [
        ("--diff-added", "added"),
        ("--diff-added-bg", "add-bg"),
        ("--diff-added-fg", "add-fg"),
        ("--diff-modified", "mod"),
        ("--diff-modified-bg", "mod-bg"),
        ("--diff-modified-fg", "mod-fg"),
        ("--diff-removed", "removed"),
        ("--diff-removed-bg", "rm-bg"),
        ("--diff-removed-fg", "rm-fg"),
        ("--diff-text-bg", "text-bg"),
        ("--diff-ignored", "ignored"),
    ];
    let mut text_var = String::new();
    for (var, label) in vars {
        write_text_on_var(&mut text_var, var);
        write_swatch(&mut html, "swatch", var, &text_var, label);
    }
    html.push_str("          </div>\n");

    html.push_str("        </section>\n");
    html
}

fn ansi_card() -> String {
    let mut html = String::from(
        "        <section class=\"card\" data-pane=\"ansi-terminal\">\n          <h2>ANSI Terminal</h2>\n",
    );
    html.push_str("          <div class=\"ansi-grid\">\n");

    let colors = [
        "black", "red", "green", "yellow", "blue", "magenta", "cyan", "white",
    ];
    let mut fg_buf = String::new();
    let rows: &[(&str, &str)] = &[("", ""), ("bright-", "br-")];
    for (prefix, label_prefix) in rows {
        for c in &colors {
            let bg = format!("--ansi-{prefix}{c}");
            write_text_on_var(&mut fg_buf, &bg);
            let label = format!("{label_prefix}{c}");
            write_swatch(&mut html, "ansi-cell", &bg, &fg_buf, &label);
        }
    }

    html.push_str("          </div>\n");
    html.push_str("        </section>\n");
    html
}

// ---------------------------------------------------------------------------
// JavaScript
// ---------------------------------------------------------------------------

fn theme_js() -> &'static str {
    r##"    <script>
      function rgbToHex(rgb) {
        const m = rgb.match(/\d+/g);
        if (!m) return '';
        return '#' + m.slice(0,3).map(x => (+x).toString(16).padStart(2,'0')).join('');
      }
      function updateSwatches() {
        document.querySelectorAll('.swatch, .editor-swatch, .ansi-cell').forEach(el => {
          const bg = getComputedStyle(el).backgroundColor;
          let hex = el.querySelector('.hex');
          if (!hex) { hex = document.createElement('span'); hex.className = 'hex'; el.appendChild(hex); }
          hex.textContent = rgbToHex(bg);
        });
      }
      const sel = document.getElementById('theme-select');
      const wcag = document.getElementById('wcag-toggle');
      const saved = localStorage.getItem('palette-theme');
      if (saved && sel.querySelector(`option[value="${saved}"]`)) {
        sel.value = saved;
        document.documentElement.dataset.theme = saved;
      }
      if (localStorage.getItem('palette-wcag') === 'true') {
        wcag.checked = true;
        document.documentElement.dataset.wcag = '';
      }
      sel.addEventListener('change', () => {
        document.documentElement.dataset.theme = sel.value;
        localStorage.setItem('palette-theme', sel.value);
        requestAnimationFrame(() => setTimeout(updateSwatches, 50));
      });
      wcag.addEventListener('change', () => {
        if (wcag.checked) { document.documentElement.dataset.wcag = ''; }
        else { delete document.documentElement.dataset.wcag; }
        localStorage.setItem('palette-wcag', wcag.checked);
        requestAnimationFrame(() => setTimeout(updateSwatches, 50));
      });
      requestAnimationFrame(updateSwatches);
      requestAnimationFrame(() => document.documentElement.classList.add('loaded'));

      // Help overlay toggle
      const helpBtn = document.getElementById('help-toggle');
      const overlay = document.querySelector('[data-pane-overlay]');
      function toggleHelp() {
        const vis = overlay.getAttribute('data-overlay-visible') === 'true';
        overlay.setAttribute('data-overlay-visible', vis ? 'false' : 'true');
      }
      helpBtn.addEventListener('click', toggleHelp);

      // Keyboard shortcuts
      document.addEventListener('keydown', (e) => {
        if (e.target.tagName === 'SELECT' || e.target.tagName === 'INPUT') return;
        switch (e.key) {
          case '?': toggleHelp(); break;
          case 'ArrowLeft':
            sel.selectedIndex = Math.max(0, sel.selectedIndex - 1);
            sel.dispatchEvent(new Event('change'));
            break;
          case 'ArrowRight':
            sel.selectedIndex = Math.min(sel.options.length - 1, sel.selectedIndex + 1);
            sel.dispatchEvent(new Event('change'));
            break;
          case 'w': case 'W':
            wcag.checked = !wcag.checked;
            wcag.dispatchEvent(new Event('change'));
            break;
        }
      });
    </script>"##
}

// ---------------------------------------------------------------------------
// Full HTML assembly
// ---------------------------------------------------------------------------

fn generate_html(registry: &Registry, themes: &[ThemeInfo]) -> Result<String, Error> {
    let css = generate_css(registry, themes)?;
    let options = generate_theme_options(registry, themes);
    let gradient_names = collect_gradient_names(registry, themes)?;
    let body = body_content(&options, &gradient_names);

    let mut html = String::with_capacity(css.len() + body.len() + 1024);
    html.push_str("<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n");
    html.push_str("  <meta charset=\"utf-8\">\n");
    html.push_str("  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");
    html.push_str("  <title>palette-core CSS Theme Showcase</title>\n");
    let _ = write!(html, "  <style>\n{css}  </style>\n");
    html.push_str("</head>\n<body>\n");
    html.push_str(&body);
    html.push_str(theme_js());
    html.push('\n');
    html.push_str("</body>\n</html>\n");

    Ok(html)
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

fn ensure_parent_dir(path: Option<&str>) -> Result<(), Error> {
    let parent = path
        .map(path::Path::new)
        .and_then(path::Path::parent)
        .filter(|p| !p.as_os_str().is_empty());
    match parent {
        Some(dir) => Ok(fs::create_dir_all(dir)?),
        None => Ok(()),
    }
}

fn run() -> Result<(), Error> {
    let registry = Registry::new();
    let themes: Box<[ThemeInfo]> = registry.list().cloned().collect();

    if themes.is_empty() {
        return Err(Error::NoThemes);
    }

    let html = generate_html(&registry, &themes)?;

    let out_path = env::args().nth(1);
    ensure_parent_dir(out_path.as_deref())?;

    match out_path {
        Some(ref p) => fs::write(p, html.as_bytes())?,
        None => io::stdout().write_all(html.as_bytes())?,
    }

    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            let _ = writeln!(io::stderr(), "error: {e}");
            ExitCode::FAILURE
        }
    }
}
