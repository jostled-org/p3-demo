use std::io;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use palette_core::registry::{Registry, ThemeInfo};
use palette_core::terminal::{ResolvedTerminalTheme, style, to_resolved_terminal_theme};
use panes::runtime::{Frame as PanesFrame, LayoutRuntime};
use panes::{FocusDirection, Layout, PanelId, PanelInputKind, PresetInfo, ResolvedLayout, fixed};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};
use ratatui::{Frame, Terminal};

// -- Constants --

const DEFAULT_PANELS: &[&str] = &["editor", "terminal", "logs"];
const ANIM_DURATION: Duration = Duration::from_millis(250);
const FRAME_BUDGET: Duration = Duration::from_millis(16);

// -- Preset construction --
// Each preset is one line: pick a layout, set ratios/gaps, get a runtime.

fn build_preset(info: &PresetInfo, panels: &[Arc<str>]) -> Option<LayoutRuntime> {
    let iter = || panels.iter().map(Arc::clone);
    let rt = match info.name {
        "master-stack" => Layout::master_stack(iter())
            .master_ratio(0.6)
            .gap(1.0)
            .into_runtime(),
        "centered-master" => Layout::centered_master(iter())
            .master_ratio(0.5)
            .gap(1.0)
            .into_runtime(),
        "monocle" => Layout::monocle(iter()).into_runtime(),
        "scrollable" => Layout::scrollable(iter()).gap(1.0).into_runtime(),
        "dwindle" => Layout::dwindle(iter()).ratio(0.5).gap(1.0).into_runtime(),
        "spiral" => Layout::spiral(iter()).ratio(0.5).gap(1.0).into_runtime(),
        "columns" => Layout::columns(3, iter()).gap(1.0).into_runtime(),
        "deck" => Layout::deck(iter())
            .master_ratio(0.7)
            .gap(1.0)
            .into_runtime(),
        "tabbed" => Layout::tabbed(iter()).tab_height(3.0).into_runtime(),
        "stacked" => Layout::stacked(iter()).title_height(1.0).into_runtime(),
        "dashboard" => {
            let cards: Vec<(Arc<str>, usize)> = panels.iter().map(|p| (Arc::clone(p), 1)).collect();
            Layout::dashboard(cards).columns(3).gap(1.0).into_runtime()
        }
        "grid" => Layout::grid(3, iter()).gap(1.0).into_runtime(),
        "split" => Layout::split(
            Arc::clone(&panels[0]),
            panels.get(1).map_or_else(|| Arc::from("empty"), Arc::clone),
        )
        .ratio(0.5)
        .gap(1.0)
        .into_runtime(),
        "sidebar" => Layout::sidebar("nav", "content")
            .sidebar_width(20.0)
            .gap(1.0)
            .into_runtime(),
        "holy-grail" => Layout::holy_grail("header", "footer", "left", "main", "right")
            .header_height(3.0)
            .footer_height(3.0)
            .sidebar_width(15.0)
            .gap(1.0)
            .into_runtime(),
        _ => return None,
    };
    rt.ok()
}

// Custom layout via builder: content area grows, status bar is fixed height.
fn build_chrome() -> Option<LayoutRuntime> {
    let layout = Layout::build_col(|c| {
        c.panel("content");
        c.panel_with("status", fixed(3.0));
    })
    .ok()?;
    Some(LayoutRuntime::new(layout.into()))
}

// Registry loads any of 30+ built-in themes by id; resolve() fills all color slots.
fn resolve_theme(registry: &Registry, info: &ThemeInfo) -> ResolvedTerminalTheme {
    let palette = registry.load(&info.id).unwrap_or_default().resolve();
    to_resolved_terminal_theme(&palette)
}

// -- Animation state --
// Smooth transitions via lerp between layout snapshots.

struct Animation {
    from: PanesFrame,
    start: Instant,
    buf: Vec<Option<panes::Rect>>,
}

// Ease-out cubic for snappy, decelerating motion
fn ease_out(t: f32) -> f32 {
    let inv = 1.0 - t;
    1.0 - inv * inv * inv
}

// -- App state --

struct App {
    presets: &'static [PresetInfo],
    preset_idx: usize,
    panels: Vec<Arc<str>>,
    next_panel_id: usize,
    runtime: Option<LayoutRuntime>,
    chrome: Option<LayoutRuntime>,
    registry: Registry,
    themes: Vec<ThemeInfo>,
    theme_idx: usize,
    theme: ResolvedTerminalTheme,
    animation: Option<Animation>,
    last_frame: Option<PanesFrame>,
    last_area: (f32, f32),
    running: bool,
}

impl App {
    fn new() -> Self {
        // palette-core: discover all installed themes
        let registry = Registry::new();
        let themes: Vec<ThemeInfo> = registry.list().cloned().collect();
        let theme = resolve_theme(&registry, &themes[0]);

        // panes: catalog of all 15 built-in presets
        let panels: Vec<Arc<str>> = DEFAULT_PANELS.iter().map(|s| Arc::from(*s)).collect();
        let presets = Layout::presets();
        let runtime = build_preset(&presets[0], &panels);
        let chrome = build_chrome();

        Self {
            presets,
            preset_idx: 0,
            panels,
            next_panel_id: DEFAULT_PANELS.len() + 1,
            runtime,
            chrome,
            registry,
            themes,
            theme_idx: 0,
            theme,
            animation: None,
            last_frame: None,
            last_area: (0.0, 0.0),
            running: true,
        }
    }

    fn is_animating(&self) -> bool {
        self.animation.is_some()
    }

    // Snapshot the current layout before a mutation so we can animate from it
    fn snapshot_for_animation(&mut self) {
        if let Some(ref frame) = self.last_frame {
            self.animation = Some(Animation {
                from: frame.clone(),
                start: Instant::now(),
                buf: Vec::new(),
            });
        }
    }

    fn current_preset(&self) -> &PresetInfo {
        &self.presets[self.preset_idx]
    }

    // Catalog tells us whether the preset accepts dynamic panel lists
    fn is_dynamic(&self) -> bool {
        self.current_preset().input == PanelInputKind::DynamicList
    }

    fn current_theme_name(&self) -> &str {
        &self.themes[self.theme_idx].name
    }

    fn current_theme_style(&self) -> &str {
        &self.themes[self.theme_idx].style
    }

    fn focused_pid(&self) -> Option<PanelId> {
        self.runtime.as_ref().and_then(|rt| rt.focused())
    }

    fn focused_kind(&self) -> Option<String> {
        self.runtime.as_ref()?.focused_kind().map(String::from)
    }

    fn rebuild_current(&mut self) {
        self.runtime = build_preset(self.current_preset(), &self.panels);
    }

    fn switch_preset(&mut self, idx: usize) {
        self.snapshot_for_animation();
        self.preset_idx = idx;
        self.rebuild_current();
    }

    fn next_preset(&mut self) {
        self.switch_preset((self.preset_idx + 1) % self.presets.len());
    }

    fn prev_preset(&mut self) {
        self.switch_preset((self.preset_idx + self.presets.len() - 1) % self.presets.len());
    }

    fn next_theme(&mut self) {
        self.theme_idx = (self.theme_idx + 1) % self.themes.len();
        self.theme = resolve_theme(&self.registry, &self.themes[self.theme_idx]);
    }

    fn prev_theme(&mut self) {
        self.theme_idx = (self.theme_idx + self.themes.len() - 1) % self.themes.len();
        self.theme = resolve_theme(&self.registry, &self.themes[self.theme_idx]);
    }

    fn next_focus(&mut self) {
        if let Some(rt) = &mut self.runtime {
            rt.focus_next();
        }
    }

    fn prev_focus(&mut self) {
        if let Some(rt) = &mut self.runtime {
            rt.focus_prev();
        }
    }

    // Spatial focus: move to nearest panel in a direction.
    // Falls back to sequential focus for presets that hide panels (monocle, deck).
    fn focus_direction(&mut self, dir: FocusDirection) {
        let Some(rt) = &mut self.runtime else { return };
        let spatial_ok = rt.focus_direction_current(dir).is_ok();
        if spatial_ok {
            return;
        }
        match dir {
            FocusDirection::Right | FocusDirection::Down => rt.focus_next(),
            FocusDirection::Left | FocusDirection::Up => rt.focus_prev(),
        }
    }

    // Runtime handles live add/remove — no rebuild needed
    fn add_panel(&mut self) {
        if !self.is_dynamic() {
            return;
        }
        let name: Arc<str> = format!("panel-{}", self.next_panel_id).into();
        self.next_panel_id += 1;
        self.panels.push(Arc::clone(&name));
        match self.runtime.is_some() {
            true => {
                self.snapshot_for_animation();
                let Some(rt) = self.runtime.as_mut() else {
                    return;
                };
                let _ = rt.add_panel(name);
            }
            false => self.rebuild_current(),
        }
    }

    fn remove_panel(&mut self) {
        if !self.is_dynamic() {
            return;
        }
        let Some((pid, kind)) = self.runtime.as_ref().and_then(|rt| {
            let pid = rt.focused()?;
            let kind = rt.focused_kind().map(String::from)?;
            Some((pid, kind))
        }) else {
            return;
        };
        self.snapshot_for_animation();
        let Some(rt) = self.runtime.as_mut() else {
            return;
        };
        let _ = rt.remove_panel(pid);
        self.panels.retain(|p| p.as_ref() != kind.as_str());
    }

    // Swap focused panel with its neighbor in the sequence
    fn swap_forward(&mut self) {
        self.snapshot_for_animation();
        if let Some(rt) = &mut self.runtime {
            rt.swap_next();
        }
    }

    fn swap_backward(&mut self) {
        self.snapshot_for_animation();
        if let Some(rt) = &mut self.runtime {
            rt.swap_prev();
        }
    }

    // Resize focused panel boundary
    fn resize_focused(&mut self, delta: f32) {
        let pid = match self.runtime.as_ref().and_then(|rt| rt.focused()) {
            Some(pid) => pid,
            None => return,
        };
        self.snapshot_for_animation();
        if let Some(rt) = &mut self.runtime {
            let _ = rt.resize_boundary(pid, delta);
        }
    }
}

// -- Rendering --

fn render_panels(
    frame: &mut Frame,
    resolved: &ResolvedLayout,
    origin: Rect,
    theme: &ResolvedTerminalTheme,
    focused_pid: Option<PanelId>,
) {
    let bg = theme.base.background;
    let fg = theme.base.foreground;
    let hi_bg = theme.base.background_highlight;
    let dim_border = theme.base.border;
    let bright_border = theme.surface.focus;
    // 12 chromatic ANSI colors for per-kind accent cycling
    let accent_colors = theme.terminal_ansi.chromatic();

    // Focus state comes from the iterator — no manual decoration matching
    for (entry, focused) in panes_ratatui::focused_panels_at(resolved, focused_pid, origin) {
        let r = entry.rect;
        if r.width == 0 || r.height == 0 {
            continue;
        }

        // kind_index groups panels by kind for stable accent assignment
        let accent = accent_colors[entry.kind_index % accent_colors.len()];
        let panel_bg = match focused {
            true => hi_bg,
            false => bg,
        };

        let (bdr, border_type, title_fg, title_mod) = match focused {
            true => (bright_border, BorderType::Double, accent, Modifier::BOLD),
            false => (dim_border, BorderType::Plain, fg, Modifier::empty()),
        };

        let label = format!(" {} ", entry.kind);
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(border_type)
            .border_style(style(bdr, panel_bg))
            .title(Span::styled(
                label,
                style(title_fg, panel_bg).add_modifier(title_mod),
            ))
            .style(style(fg, panel_bg));

        let inner = block.inner(r);
        frame.render_widget(block, r);

        let comment = theme.typography.comment;
        let dims = format!("{}×{}", r.width, r.height);
        let pos = format!("({},{})", r.x, r.y);
        let lines = vec![
            Line::from(Span::styled(dims, Style::default().fg(comment))),
            Line::from(Span::styled(pos, Style::default().fg(comment))),
        ];

        frame.render_widget(Paragraph::new(lines).style(style(fg, panel_bg)), inner);
    }
}

fn render_layout(
    frame: &mut Frame,
    runtime: &mut LayoutRuntime,
    area: Rect,
    theme: &ResolvedTerminalTheme,
    animation: &mut Option<Animation>,
    last_frame: &mut Option<PanesFrame>,
    last_area: &mut (f32, f32),
) {
    let error_style = Style::default().fg(Color::Red).bg(theme.base.background);
    let w = f32::from(area.width);
    let h = f32::from(area.height);

    let rt_frame = match runtime.resolve(w, h) {
        Ok(f) => f,
        Err(e) => {
            let msg = format!("resolve error: {e}");
            frame.render_widget(Paragraph::new(msg).style(error_style), area);
            return;
        }
    };

    let target = rt_frame.layout();

    // Lerp between old and new layout during animation
    let display = animation.as_mut().and_then(|anim| {
        let t = (anim.start.elapsed().as_secs_f32() / ANIM_DURATION.as_secs_f32()).min(1.0);
        let lerped = anim
            .from
            .layout()
            .lerp_into(target, ease_out(t), &mut anim.buf);
        (t < 1.0).then_some(lerped)
    });
    if display.is_none() {
        *animation = None;
    }

    let resolved = display.as_ref().unwrap_or(target);
    *last_area = (w, h);

    render_panels(frame, resolved, area, theme, runtime.focused());

    // Diff stats from runtime — computed automatically during resolve()
    let diff = runtime.last_diff();
    let diff_text = format!(
        "+{} -{} ~{} ={} >{}",
        diff.added.len(),
        diff.removed.len(),
        diff.resized.len(),
        diff.unchanged.len(),
        diff.moved.len(),
    );
    let text_width = (diff_text.len() as u16).min(area.width);
    let diff_area = Rect {
        x: area.x,
        y: area.y + area.height.saturating_sub(1),
        width: text_width,
        height: 1.min(area.height),
    };
    frame.render_widget(
        Paragraph::new(diff_text).style(style(theme.typography.comment, theme.base.background)),
        diff_area,
    );

    // Store frame for next animation snapshot (moved after all borrows of target end)
    *last_frame = Some(rt_frame);
}

fn render(frame: &mut Frame, app: &mut App) {
    let area = frame.area();

    frame.render_widget(
        Block::default().style(Style::default().bg(app.theme.base.background)),
        area,
    );

    // Chrome is itself a panes layout: resolve once, look up rects by kind
    let Some(ref mut chrome_rt) = app.chrome else {
        return;
    };
    let chrome_frame = match chrome_rt.resolve(f32::from(area.width), f32::from(area.height)) {
        Ok(f) => f,
        Err(_) => return,
    };
    let chrome = chrome_frame.layout();
    let chrome_rects = panes_ratatui::convert(chrome);

    let content_pid = chrome.by_kind("content")[0];
    let status_pid = chrome.by_kind("status")[0];
    let layout_area = chrome_rects[&content_pid];
    let status_area = chrome_rects[&status_pid];

    let Some(ref mut rt) = app.runtime else {
        let error_style = Style::default()
            .fg(Color::Red)
            .bg(app.theme.base.background);
        frame.render_widget(
            Paragraph::new("build error").style(error_style),
            layout_area,
        );
        render_status(frame, app, status_area);
        return;
    };
    render_layout(
        frame,
        rt,
        layout_area,
        &app.theme,
        &mut app.animation,
        &mut app.last_frame,
        &mut app.last_area,
    );

    render_status(frame, app, status_area);
}

// All colors come from the theme by semantic role — no hardcoded values
fn render_status(frame: &mut Frame, app: &App, area: Rect) {
    let fg = app.theme.base.foreground;
    let sb_bg = app.theme.surface.statusline;
    let info_fg = app.theme.semantic.info;
    let ok_fg = app.theme.semantic.success;
    let warn_fg = app.theme.semantic.warning;
    let muted = app.theme.typography.comment;

    let panel_count = app.panels.len();
    let dynamic_marker = match app.is_dynamic() {
        true => format!(" panels: {panel_count}"),
        false => " [fixed]".to_string(),
    };

    let preset = app.current_preset();
    let status_line = Line::from(vec![
        Span::styled(" preset: ", style(fg, sb_bg)),
        Span::styled(preset.name, style(info_fg, sb_bg)),
        Span::styled(
            format!(" ({}/{})", app.preset_idx + 1, app.presets.len()),
            style(muted, sb_bg),
        ),
        Span::styled(" │ theme: ", style(fg, sb_bg)),
        Span::styled(app.current_theme_name(), style(ok_fg, sb_bg)),
        Span::styled(
            format!(" [{}]", app.current_theme_style()),
            style(muted, sb_bg),
        ),
        Span::styled(
            format!(" ({}/{})", app.theme_idx + 1, app.themes.len()),
            style(muted, sb_bg),
        ),
        Span::styled(format!(" │{dynamic_marker}"), style(warn_fg, sb_bg)),
    ]);

    let focused_pid = app.focused_pid();
    let focused_kind = app.focused_kind();
    let focus_text = match (focused_pid, focused_kind.as_deref()) {
        (Some(_), Some(kind)) => kind.to_string(),
        (Some(pid), None) => format!("{pid}"),
        _ => "none".to_string(),
    };
    let focus_line = Line::from(vec![
        Span::styled(" focus: ", style(fg, sb_bg)),
        Span::styled(focus_text, style(warn_fg, sb_bg)),
    ]);

    let help =
        " ←/→ preset  ↑/↓ theme  Tab focus  HJKL spatial  a/d add/rm  [/] swap  +/- resize  q quit";
    let help_line = Line::from(vec![Span::styled(help, style(muted, sb_bg))]);

    frame.render_widget(
        Paragraph::new(vec![status_line, focus_line, help_line]).style(Style::default().bg(sb_bg)),
        area,
    );
}

fn handle_key(app: &mut App, key: event::KeyEvent) {
    match (key.kind, key.code) {
        (KeyEventKind::Press, KeyCode::Char('q') | KeyCode::Esc) => app.running = false,
        (KeyEventKind::Press, KeyCode::Right | KeyCode::Char('l')) => app.next_preset(),
        (KeyEventKind::Press, KeyCode::Left | KeyCode::Char('h')) => app.prev_preset(),
        (KeyEventKind::Press, KeyCode::Down | KeyCode::Char('j')) => app.next_theme(),
        (KeyEventKind::Press, KeyCode::Up | KeyCode::Char('k')) => app.prev_theme(),
        (KeyEventKind::Press, KeyCode::Tab) => app.next_focus(),
        (KeyEventKind::Press, KeyCode::BackTab) => app.prev_focus(),
        // Spatial focus via shift+hjkl
        (KeyEventKind::Press, KeyCode::Char('H')) => {
            app.focus_direction(FocusDirection::Left);
        }
        (KeyEventKind::Press, KeyCode::Char('J')) => {
            app.focus_direction(FocusDirection::Down);
        }
        (KeyEventKind::Press, KeyCode::Char('K')) => {
            app.focus_direction(FocusDirection::Up);
        }
        (KeyEventKind::Press, KeyCode::Char('L')) => {
            app.focus_direction(FocusDirection::Right);
        }
        (KeyEventKind::Press, KeyCode::Char('a')) => app.add_panel(),
        (KeyEventKind::Press, KeyCode::Char('d')) => app.remove_panel(),
        // Swap focused panel with neighbor
        (KeyEventKind::Press, KeyCode::Char('[')) => app.swap_backward(),
        (KeyEventKind::Press, KeyCode::Char(']')) => app.swap_forward(),
        // Resize focused panel boundary
        (KeyEventKind::Press, KeyCode::Char('+') | KeyCode::Char('=')) => {
            app.resize_focused(0.05);
        }
        (KeyEventKind::Press, KeyCode::Char('-')) => app.resize_focused(-0.05),
        _ => {}
    }
}

// -- Main loop --
// Polls with short timeout during animations for smooth 60fps rendering.
// Blocks on input when idle to avoid burning CPU.

fn main() -> io::Result<()> {
    enable_raw_mode()?;
    crossterm::execute!(io::stdout(), EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();

    while app.running {
        terminal.draw(|frame| render(frame, &mut app))?;

        // Animate at ~60fps; block when idle
        let timeout = match app.is_animating() {
            true => FRAME_BUDGET,
            false => Duration::from_secs(60),
        };

        let has_event = event::poll(timeout).unwrap_or(false);
        if !has_event {
            continue;
        }

        let mut ev = event::read()?;
        let mut resized = false;
        loop {
            match ev {
                Event::Key(key) => handle_key(&mut app, key),
                Event::Resize(_, _) => resized = true,
                _ => {}
            }
            let has_more = event::poll(Duration::ZERO).unwrap_or(false);
            if !has_more {
                break;
            }
            match event::read() {
                Ok(next) => ev = next,
                Err(_) => break,
            }
        }
        if resized {
            terminal.clear()?;
        }
    }

    disable_raw_mode()?;
    crossterm::execute!(io::stdout(), LeaveAlternateScreen)?;
    Ok(())
}
