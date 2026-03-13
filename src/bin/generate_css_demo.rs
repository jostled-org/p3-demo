use std::fmt::Write as _;
use std::io::Write as _;
use std::{env, fs, io, path};

use palette_core::color::Color;
use palette_core::contrast::ContrastLevel;
use palette_core::css::css_name;
use palette_core::palette::Palette;
use palette_core::registry::{Registry, ThemeInfo};
use palette_core::resolved::ResolvedPalette;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(thiserror::Error, Debug)]
enum Error {
    #[error("failed to load theme '{0}': {1}")]
    ThemeLoad(String, palette_core::PaletteError),

    #[error("registry contains no themes")]
    NoThemes,

    #[error("{0}")]
    Io(#[from] io::Error),
}

// ---------------------------------------------------------------------------
// Theme helpers
// ---------------------------------------------------------------------------

fn is_light(info: &ThemeInfo) -> bool {
    let s = info.style.as_ref();
    matches!(s, "light" | "day" | "latte")
}

fn text_on_color(color: &Color) -> &'static str {
    match color.relative_luminance() > 0.179 {
        true => "#000",
        false => "#fff",
    }
}

fn write_contrast_vars<'a>(
    out: &mut String,
    section: &str,
    slots: impl Iterator<Item = (&'static str, &'a Color)>,
) {
    for (field, color) in slots {
        let slot = match css_name(section, field) {
            Some(name) => name.to_string(),
            None => format!("{section}-{}", field.replace('_', "-")),
        };
        let _ = writeln!(out, "  --text-on-{slot}: {};", text_on_color(color));
    }
}

fn contrast_css(palette: &Palette) -> String {
    let mut out = String::with_capacity(2048);
    write_contrast_vars(&mut out, "base", palette.base.populated_slots());
    write_contrast_vars(&mut out, "semantic", palette.semantic.populated_slots());
    write_contrast_vars(&mut out, "diff", palette.diff.populated_slots());
    write_contrast_vars(&mut out, "surface", palette.surface.populated_slots());
    write_contrast_vars(&mut out, "typography", palette.typography.populated_slots());
    write_contrast_vars(&mut out, "syntax", palette.syntax.populated_slots());
    write_contrast_vars(&mut out, "editor", palette.editor.populated_slots());
    write_contrast_vars(
        &mut out,
        "terminal",
        palette.terminal_ansi.populated_slots(),
    );
    out
}

fn write_resolved_section<'a>(
    out: &mut String,
    section: &str,
    slots: impl Iterator<Item = (&'static str, &'a Color)>,
) {
    for (field, color) in slots {
        let slot = match css_name(section, field) {
            Some(name) => name.to_string(),
            None => format!("{section}-{}", field.replace('_', "-")),
        };
        let _ = writeln!(out, "  --{slot}: {color};");
    }
}

fn resolved_to_css_scoped(resolved: &ResolvedPalette, selector: &str) -> String {
    let mut decls = String::with_capacity(4096);
    write_resolved_section(&mut decls, "base", resolved.base.all_slots());
    write_resolved_section(&mut decls, "semantic", resolved.semantic.all_slots());
    write_resolved_section(&mut decls, "diff", resolved.diff.all_slots());
    write_resolved_section(&mut decls, "surface", resolved.surface.all_slots());
    write_resolved_section(&mut decls, "typography", resolved.typography.all_slots());
    write_resolved_section(&mut decls, "syntax", resolved.syntax.all_slots());
    write_resolved_section(&mut decls, "editor", resolved.editor.all_slots());
    write_resolved_section(&mut decls, "terminal", resolved.terminal_ansi.all_slots());
    // Contrast text vars for the adjusted palette
    write_contrast_vars(&mut decls, "base", resolved.base.all_slots());
    write_contrast_vars(&mut decls, "semantic", resolved.semantic.all_slots());
    write_contrast_vars(&mut decls, "diff", resolved.diff.all_slots());
    write_contrast_vars(&mut decls, "surface", resolved.surface.all_slots());
    write_contrast_vars(&mut decls, "typography", resolved.typography.all_slots());
    write_contrast_vars(&mut decls, "syntax", resolved.syntax.all_slots());
    write_contrast_vars(&mut decls, "editor", resolved.editor.all_slots());
    write_contrast_vars(&mut decls, "terminal", resolved.terminal_ansi.all_slots());
    format!("{selector} {{\n{decls}}}\n")
}

fn generate_theme_css(registry: &Registry, info: &ThemeInfo) -> Result<String, Error> {
    let palette = registry
        .load(&info.id)
        .map_err(|e| Error::ThemeLoad(info.id.to_string(), e))?;
    let selector = format!("[data-theme=\"{}\"]", info.id);
    let mut css = palette.to_css_scoped(&selector, None);
    // Append contrast text vars inside the same selector
    let vars = contrast_css(&palette);
    match css.rfind('}') {
        Some(pos) => css.insert_str(pos, &vars),
        None => css.push_str(&vars),
    }
    // WCAG AA-adjusted variant
    let adjusted = palette.resolve_with_contrast(ContrastLevel::AaNormal);
    let wcag_selector = format!("[data-wcag][data-theme=\"{}\"]", info.id);
    css.push_str(&resolved_to_css_scoped(&adjusted, &wcag_selector));
    Ok(css)
}

fn generate_theme_options(themes: &[ThemeInfo]) -> String {
    let mut dark = Vec::new();
    let mut light = Vec::new();
    for t in themes {
        let opt = format!("        <option value=\"{}\">{}</option>", t.id, t.name);
        match is_light(t) {
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

    // -- First theme as :root default (renders before JS) --
    let first = registry
        .load(&themes[0].id)
        .map_err(|e| Error::ThemeLoad(themes[0].id.to_string(), e))?;
    let mut root_css = first.to_css();
    let root_vars = contrast_css(&first);
    match root_css.rfind('}') {
        Some(pos) => root_css.insert_str(pos, &root_vars),
        None => root_css.push_str(&root_vars),
    }
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
*, *::before, *::after { box-sizing: border-box; margin: 0; padding: 0; }

body {
  font-family: system-ui, -apple-system, sans-serif;
  background: var(--bg);
  color: var(--fg);
  transition: background 0.3s, color 0.3s;
  line-height: 1.5;
}

/* Header */
.header {
  position: sticky;
  top: 0;
  z-index: 100;
  background: var(--ui-statusline);
  border-bottom: 1px solid var(--border);
  padding: 0.75rem 1.5rem;
  display: flex;
  align-items: center;
  gap: 1rem;
  transition: background 0.3s;
}
.header h1 {
  font-size: 1.1rem;
  font-weight: 600;
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
.wcag-toggle input { cursor: pointer; }

/* Grid */
.grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(380px, 1fr));
  gap: 1.25rem;
  padding: 1.5rem;
  max-width: 1600px;
  margin: 0 auto;
}

/* Cards */
.card {
  background: var(--bg-dark);
  border: 1px solid var(--border);
  border-radius: 8px;
  padding: 1.25rem;
  transition: background 0.3s, border-color 0.3s;
}
.card h2 {
  font-size: 0.95rem;
  font-weight: 600;
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
  width: 72px;
  height: 56px;
  border-radius: 4px;
  border: 1px solid rgba(128,128,128,0.4);
  outline: 1px solid rgba(128,128,128,0.15);
  outline-offset: 1px;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: flex-end;
  padding: 3px;
  position: relative;
}
.swatch span {
  font-size: 0.55rem;
  text-align: center;
  line-height: 1.1;
  word-break: break-all;
}
.swatch .hex {
  font-size: 0.5rem;
  opacity: 0.85;
  font-family: monospace;
}

/* Code block */
pre.code {
  background: var(--bg-dark);
  border: 1px solid var(--border);
  border-radius: 6px;
  padding: 1rem;
  overflow-x: auto;
  font-family: 'SF Mono', 'Fira Code', 'Cascadia Code', monospace;
  font-size: 0.8rem;
  line-height: 1.6;
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
  font-size: 0.8rem;
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

/* ANSI grid */
.ansi-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(70px, 1fr));
  gap: 4px;
}
.ansi-cell {
  height: 36px;
  border-radius: 3px;
  display: flex;
  align-items: flex-end;
  justify-content: center;
  padding: 2px;
}
.ansi-cell span {
  font-size: 0.5rem;
  text-align: center;
}

/* Editor swatches */
.editor-row {
  display: flex;
  flex-wrap: wrap;
  gap: 0.5rem;
  margin-bottom: 0.5rem;
}
.editor-swatch {
  min-width: 70px;
  height: 36px;
  border-radius: 4px;
  border: 1px solid var(--border);
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 2px 6px;
  font-size: 0.6rem;
}
"#;

// ---------------------------------------------------------------------------
// HTML body
// ---------------------------------------------------------------------------

fn body_content(theme_options: &str) -> String {
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
      <span>WCAG AA</span>
    </label>
  </header>
  <main class="grid">
"#
    );

    // 1. Base Colors
    html.push_str(&swatch_card(
        "Base Colors",
        &[
            ("--bg", "bg"),
            ("--bg-dark", "bg-dark"),
            ("--bg-hi", "bg-hi"),
            ("--fg", "fg"),
            ("--fg-dark", "fg-dark"),
            ("--border", "border"),
            ("--border-hi", "border-hi"),
        ],
    ));

    // 2. Semantic Colors
    html.push_str(&swatch_card(
        "Semantic Colors",
        &[
            ("--success", "success"),
            ("--warning", "warning"),
            ("--error", "error"),
            ("--info", "info"),
            ("--hint", "hint"),
        ],
    ));

    // 3. Surface Colors
    html.push_str(&swatch_card(
        "Surface Colors",
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
    ));

    // 4. Typography
    html.push_str(&swatch_card(
        "Typography",
        &[
            ("--text-comment", "comment"),
            ("--text-gutter", "gutter"),
            ("--text-line-num", "line-num"),
            ("--text-sel", "sel"),
            ("--text-link", "link"),
            ("--text-title", "title"),
        ],
    ));

    // 5. Syntax Highlighting
    html.push_str(&syntax_card());

    // 6. Editor
    html.push_str(&editor_card());

    // 7. Diff
    html.push_str(&diff_card());

    // 8. ANSI Terminal
    html.push_str(&ansi_card());

    html.push_str("  </main>\n");
    html
}

fn swatch_card(title: &str, swatches: &[(&str, &str)]) -> String {
    let mut html = String::new();
    let _ = write!(
        html,
        "    <section class=\"card\">\n      <h2>{title}</h2>\n      <div class=\"swatches\">\n"
    );
    for (var, label) in swatches {
        let text_var = var.replace("--", "--text-on-");
        let _ = writeln!(
            html,
            "        <div class=\"swatch\" style=\"background: var({var}); color: var({text_var});\"><span>{label}</span></div>"
        );
    }
    html.push_str("      </div>\n    </section>\n");
    html
}

fn syntax_card() -> String {
    let mut html = String::from(
        "    <section class=\"card\" style=\"grid-column: 1 / -1;\">\n      <h2>Syntax Highlighting</h2>\n",
    );
    html.push_str(&code_snippet());
    html.push_str("      <h2 style=\"margin-top: 1rem;\">HTML Tags</h2>\n");
    html.push_str(&html_snippet());
    html.push_str("    </section>\n");
    html
}

fn code_snippet() -> String {
    // Rust code covering all syntax token types
    format!(
        r##"      <pre class="code">{}</pre>
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
        r##"      <pre class="code">{}</pre>
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
    let mut html = String::from("    <section class=\"card\">\n      <h2>Editor</h2>\n");

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

    for (label, swatches) in groups {
        let _ = write!(
            html,
            "      <p style=\"font-size:0.75rem; color:var(--text-comment); margin:0.5rem 0 0.25rem;\">{label}</p>\n      <div class=\"editor-row\">\n"
        );
        for (var, name) in *swatches {
            let text_var = var.replace("--", "--text-on-");
            let _ = writeln!(
                html,
                "        <div class=\"editor-swatch\" style=\"background: var({var}); color: var({text_var});\"><span>{name}</span></div>"
            );
        }
        html.push_str("      </div>\n");
    }

    html.push_str("    </section>\n");
    html
}

fn diff_card() -> String {
    let mut html = String::from("    <section class=\"card\">\n      <h2>Diff</h2>\n");

    // Mock diff hunk
    html.push_str("      <div class=\"diff-hunk\">\n");
    html.push_str("        <div class=\"diff-line diff-context\"> fn main() {</div>\n");
    html.push_str("        <div class=\"diff-line diff-removed\">-    let x = 1;</div>\n");
    html.push_str("        <div class=\"diff-line diff-added\">+    let x = 42;</div>\n");
    html.push_str("        <div class=\"diff-line diff-modified\">~    let y = x + 1;</div>\n");
    html.push_str("        <div class=\"diff-line diff-context\"> }</div>\n");
    html.push_str("      </div>\n");

    // Diff swatches
    html.push_str("      <div class=\"swatches\" style=\"margin-top: 0.75rem;\">\n");
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
    for (var, label) in vars {
        let text_var = var.replace("--", "--text-on-");
        let _ = writeln!(
            html,
            "        <div class=\"swatch\" style=\"background: var({var}); color: var({text_var});\"><span>{label}</span></div>"
        );
    }
    html.push_str("      </div>\n");

    html.push_str("    </section>\n");
    html
}

fn ansi_card() -> String {
    let mut html = String::from("    <section class=\"card\">\n      <h2>ANSI Terminal</h2>\n");
    html.push_str("      <div class=\"ansi-grid\">\n");

    let colors = [
        "black", "red", "green", "yellow", "blue", "magenta", "cyan", "white",
    ];
    // Normal row
    for c in &colors {
        let _ = writeln!(
            html,
            "        <div class=\"ansi-cell\" style=\"background: var(--ansi-{c}); color: var(--text-on-ansi-{c});\"><span>{c}</span></div>"
        );
    }
    // Bright row
    for c in &colors {
        let _ = writeln!(
            html,
            "        <div class=\"ansi-cell\" style=\"background: var(--ansi-bright-{c}); color: var(--text-on-ansi-bright-{c});\"><span>br-{c}</span></div>"
        );
    }

    html.push_str("      </div>\n");
    html.push_str("    </section>\n");
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
    </script>"##
}

// ---------------------------------------------------------------------------
// Full HTML assembly
// ---------------------------------------------------------------------------

fn generate_html(registry: &Registry, themes: &[ThemeInfo]) -> Result<String, Error> {
    let css = generate_css(registry, themes)?;
    let options = generate_theme_options(themes);
    let body = body_content(&options);

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
    let themes: Vec<ThemeInfo> = registry.list().cloned().collect();

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

fn main() {
    match run() {
        Ok(()) => {}
        Err(e) => {
            let _ = writeln!(io::stderr(), "error: {e}");
            std::process::exit(1);
        }
    }
}
