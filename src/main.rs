use std::io::{self, Write as _};
use std::process::ExitCode;
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEventKind, MouseEventKind};
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use demo_presets::{
    Action, DemoError, DemoState, HELP_BINDINGS_TUI, build_chrome, build_help_line, ease_out,
    format_diff_counts, load_snapshot, save_snapshot, status_data,
};
use palette_core::terminal::{ResolvedTerminalTheme, style, to_resolved_terminal_theme};
use panes::runtime::{Frame as PanesFrame, LayoutRuntime};
use panes::{FocusDirection, OverlayEntry, ResolvedLayout};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};
use ratatui::{Frame, Terminal};

// -- Constants --

const ANIM_DURATION: Duration =
    Duration::from_millis((demo_presets::ANIM_DURATION_SECS * 1000.0) as u64);
const FRAME_BUDGET: Duration = Duration::from_millis(16);

// -- Error type --

#[derive(thiserror::Error, Debug)]
enum Error {
    #[error("{0}")]
    Demo(#[from] DemoError),
    #[error("{0}")]
    Io(#[from] io::Error),
}

// -- Animation state --
// Smooth transitions via lerp between layout snapshots.

struct Animation {
    from: PanesFrame,
    start: Instant,
    buf: Vec<Option<panes::Rect>>,
}

// -- App state --

struct App {
    state: DemoState,
    chrome: Option<LayoutRuntime>,
    theme: ResolvedTerminalTheme,
    animation: Option<Animation>,
    last_frame: Option<PanesFrame>,
    last_area: (f32, f32),
    running: bool,
    /// Cached status-bar help line (static content, built once)
    cached_help_line: Box<str>,
    /// Cached help overlay lines (static text, styled per-frame with theme colors)
    cached_overlay_lines: Box<[Box<str>]>,
}

impl App {
    fn new() -> Result<Self, Error> {
        let state = DemoState::new(1.0)?;
        let palette = state.load_current_palette()?;
        let theme = to_resolved_terminal_theme(&palette.resolve());
        let chrome = build_chrome();

        let cached_help_line = build_help_line(HELP_BINDINGS_TUI);
        let cached_overlay_lines = build_overlay_lines();

        Ok(Self {
            state,
            chrome,
            theme,
            animation: None,
            last_frame: None,
            last_area: (0.0, 0.0),
            running: true,
            cached_help_line,
            cached_overlay_lines,
        })
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

    fn reload_theme(&mut self) -> bool {
        let Ok(palette) = self.state.load_current_palette() else {
            return false;
        };
        self.theme = to_resolved_terminal_theme(&palette.resolve());
        true
    }
}

/// Pre-format help overlay binding strings (called once; styled per-frame with theme colors).
fn build_overlay_lines() -> Box<[Box<str>]> {
    HELP_BINDINGS_TUI
        .iter()
        .map(|b| format!("  {:14}{}", b.key, b.action).into_boxed_str())
        .collect()
}

// -- Rendering --

fn render_panels(
    frame: &mut Frame,
    resolved: &ResolvedLayout,
    origin: Rect,
    theme: &ResolvedTerminalTheme,
    focused_pid: Option<panes::PanelId>,
) {
    let bg = theme.base.background;
    let fg = theme.base.foreground;
    let hi_bg = theme.base.background_highlight;
    let dim_border = theme.base.border;
    let bright_border = theme.surface.focus;
    // 12 chromatic ANSI colors for per-kind accent cycling
    let accent_colors = theme.terminal.chromatic();

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

fn render_overlay(
    frame: &mut Frame,
    overlay: OverlayEntry<'_, Rect>,
    theme: &ResolvedTerminalTheme,
    cached_lines: &[Box<str>],
) {
    let r = overlay.rect;
    if r.width == 0 || r.height == 0 {
        return;
    }

    let overlay_bg = theme.surface.float;
    let fg = theme.base.foreground;
    let border_fg = theme.surface.focus;
    let muted = theme.typography.comment;

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(style(border_fg, overlay_bg))
        .title(Span::styled(
            format!(" {} ", overlay.kind),
            style(border_fg, overlay_bg).add_modifier(Modifier::BOLD),
        ))
        .style(style(fg, overlay_bg));

    let inner = block.inner(r);
    frame.render_widget(block, r);

    // Text is pre-formatted; only styling varies with theme
    let lines: Vec<Line> = cached_lines
        .iter()
        .map(|text| Line::from(Span::styled(&**text, style(muted, overlay_bg))))
        .collect();
    frame.render_widget(Paragraph::new(lines).style(style(fg, overlay_bg)), inner);
}

fn render_layout(frame: &mut Frame, app: &mut App, area: Rect) {
    let error_style = Style::default()
        .fg(Color::Red)
        .bg(app.theme.base.background);
    let w = f32::from(area.width);
    let h = f32::from(area.height);

    let rt_frame = match app.state.resolve(w, h) {
        Ok(f) => f,
        Err(e) => {
            let msg = format!("resolve error: {e}");
            frame.render_widget(Paragraph::new(msg).style(error_style), area);
            return;
        }
    };

    let target = rt_frame.layout();

    // Lerp between old and new layout during animation
    let display = app.animation.as_mut().and_then(|anim| {
        let t = (anim.start.elapsed().as_secs_f32() / ANIM_DURATION.as_secs_f32()).min(1.0);
        let lerped = anim
            .from
            .layout()
            .lerp_into(target, ease_out(t), &mut anim.buf);
        (t < 1.0).then_some(lerped)
    });
    if display.is_none() {
        app.animation = None;
    }

    let resolved = display.as_ref().unwrap_or(target);
    app.last_area = (w, h);

    render_panels(frame, resolved, area, &app.theme, app.state.focused_pid());

    // Overlays (e.g. help panel) rendered on top of panels — auto-clears underlying cells
    let theme = &app.theme;
    let cached_lines = &app.cached_overlay_lines;
    panes_ratatui::render_overlays_at(frame, resolved, area, |f, entry| {
        render_overlay(f, entry, theme, cached_lines);
    });

    // Diff stats from runtime — computed automatically during resolve()
    let diff = app.state.last_diff();
    let diff_text = format_diff_counts(&diff);
    let text_width = (diff_text.len() as u16).min(area.width);
    let diff_area = Rect {
        x: area.x,
        y: area.y + area.height.saturating_sub(1),
        width: text_width,
        height: 1.min(area.height),
    };
    frame.render_widget(
        Paragraph::new(&*diff_text).style(style(
            app.theme.typography.comment,
            app.theme.base.background,
        )),
        diff_area,
    );

    // Store frame for next animation snapshot (moved after all borrows of target end)
    app.last_frame = Some(rt_frame);
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

    // Zero-alloc chrome rect lookup via by_kind + get
    let mut content_area = None;
    let mut status_area = None;
    for entry in panes_ratatui::panels_at(chrome, area) {
        match entry.kind {
            "content" => content_area = Some(entry.rect),
            "status" => status_area = Some(entry.rect),
            _ => {}
        }
    }
    let (Some(layout_area), Some(status_area)) = (content_area, status_area) else {
        return;
    };

    match app.state.runtime() {
        Some(_) => render_layout(frame, app, layout_area),
        None => {
            let error_style = Style::default()
                .fg(Color::Red)
                .bg(app.theme.base.background);
            frame.render_widget(
                Paragraph::new("build error").style(error_style),
                layout_area,
            );
        }
    }

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

    let sd = status_data(&app.state);

    let preset_pos = format!(" ({}/{})", sd.preset_idx + 1, sd.preset_count);
    let theme_style_tag = format!(" [{}]", sd.theme_style);
    let theme_pos = format!(" ({}/{})", sd.theme_idx + 1, sd.theme_count);
    let panel_marker = match sd.is_dynamic {
        true => format!(" │ panels: {}", sd.panel_count),
        false => String::from(" │ [fixed]"),
    };

    let status_line = Line::from(vec![
        Span::styled(" preset: ", style(fg, sb_bg)),
        Span::styled(sd.preset_name, style(info_fg, sb_bg)),
        Span::styled(preset_pos, style(muted, sb_bg)),
        Span::styled(" │ theme: ", style(fg, sb_bg)),
        Span::styled(sd.theme_name, style(ok_fg, sb_bg)),
        Span::styled(theme_style_tag, style(muted, sb_bg)),
        Span::styled(theme_pos, style(muted, sb_bg)),
        Span::styled(panel_marker, style(warn_fg, sb_bg)),
    ]);

    let focus_line = Line::from(vec![
        Span::styled(" focus: ", style(fg, sb_bg)),
        Span::styled(&*sd.focus_text, style(warn_fg, sb_bg)),
    ]);

    let help_line = Line::from(vec![Span::styled(
        &*app.cached_help_line,
        style(muted, sb_bg),
    )]);

    frame.render_widget(
        Paragraph::new(vec![status_line, focus_line, help_line]).style(Style::default().bg(sb_bg)),
        area,
    );
}

/// Map a crossterm key code to a renderer-agnostic `Action`.
fn key_to_action(code: KeyCode) -> Option<Action> {
    match code {
        KeyCode::Right => Some(Action::NextPreset),
        KeyCode::Left => Some(Action::PrevPreset),
        KeyCode::Down => Some(Action::NextTheme),
        KeyCode::Up => Some(Action::PrevTheme),
        KeyCode::Tab => Some(Action::FocusNext),
        KeyCode::BackTab => Some(Action::FocusPrev),
        KeyCode::Char('H') => Some(Action::FocusDirection(FocusDirection::Left)),
        KeyCode::Char('J') => Some(Action::FocusDirection(FocusDirection::Down)),
        KeyCode::Char('K') => Some(Action::FocusDirection(FocusDirection::Up)),
        KeyCode::Char('L') => Some(Action::FocusDirection(FocusDirection::Right)),
        KeyCode::Char('a') => Some(Action::AddPanel),
        KeyCode::Char('d') => Some(Action::RemovePanel),
        KeyCode::Char('c') => Some(Action::ToggleCollapsed),
        KeyCode::Char('[') => Some(Action::SwapPrev),
        KeyCode::Char(']') => Some(Action::SwapNext),
        KeyCode::Char('=') => Some(Action::ResizeHorizontal(0.05)),
        KeyCode::Char('-') => Some(Action::ResizeHorizontal(-0.05)),
        KeyCode::Char('+') => Some(Action::ResizeVertical(0.05)),
        KeyCode::Char('_') => Some(Action::ResizeVertical(-0.05)),
        KeyCode::Char('?') => Some(Action::ToggleHelp),
        _ => None,
    }
}

fn handle_key(app: &mut App, key: event::KeyEvent) {
    if !matches!(key.kind, KeyEventKind::Press) {
        return;
    }
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => {
            app.running = false;
            return;
        }
        _ => {}
    }
    let Some(action) = key_to_action(key.code) else {
        return;
    };
    // Snapshot before mutation so animation lerps from old to new layout
    if action.changes_layout() {
        app.snapshot_for_animation();
    }
    let layout_changed = app.state.apply(action);
    // Theme cycling needs a palette reload
    if !layout_changed {
        app.reload_theme();
    }
}

fn handle_mouse(app: &mut App, mouse: event::MouseEvent) {
    let ptr_x = f32::from(mouse.column);
    let ptr_y = f32::from(mouse.row);
    let dragging = app.state.is_dragging();
    let on_boundary = app.state.boundary_hover(ptr_x, ptr_y).is_some();
    let action = match (mouse.kind, dragging, on_boundary) {
        (MouseEventKind::ScrollDown, _, _) => Action::ScrollBy(1.0),
        (MouseEventKind::ScrollUp, _, _) => Action::ScrollBy(-1.0),
        (MouseEventKind::Down(event::MouseButton::Left), _, true) => {
            Action::DragStart(ptr_x, ptr_y)
        }
        (MouseEventKind::Down(event::MouseButton::Left), _, false) => Action::FocusAt(ptr_x, ptr_y),
        (MouseEventKind::Drag(event::MouseButton::Left), true, _) => Action::DragMove(ptr_x, ptr_y),
        (MouseEventKind::Up(event::MouseButton::Left), true, _) => Action::DragEnd,
        _ => return,
    };
    if action.changes_layout() {
        app.snapshot_for_animation();
    }
    app.state.apply(action);
}

// -- Main loop --
// Polls with short timeout during animations for smooth 60fps rendering.
// Blocks on input when idle to avoid burning CPU.

fn run() -> Result<(), Error> {
    enable_raw_mode()?;
    crossterm::execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new()?;
    load_snapshot(&mut app.state);

    while app.running {
        terminal.draw(|frame| render(frame, &mut app))?;

        // Animate at ~60fps; block when idle
        let timeout = match app.is_animating() {
            true => FRAME_BUDGET,
            false => Duration::from_secs(60),
        };

        if !event::poll(timeout)? {
            continue;
        }

        let mut ev = event::read()?;
        let mut resized = false;
        loop {
            match ev {
                Event::Key(key) => handle_key(&mut app, key),
                Event::Mouse(mouse) => handle_mouse(&mut app, mouse),
                Event::Resize(_, _) => resized = true,
                _ => {}
            }
            if !event::poll(Duration::ZERO)? {
                break;
            }
            ev = event::read()?;
        }
        if resized {
            terminal.clear()?;
        }
    }

    save_snapshot(&app.state);

    disable_raw_mode()?;
    crossterm::execute!(io::stdout(), DisableMouseCapture, LeaveAlternateScreen)?;
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
