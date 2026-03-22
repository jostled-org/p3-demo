use std::sync::Arc;

use demo_presets::{
    Action, DemoError, DemoState, HELP_BINDINGS_GUI, chromatic_colors, ease_out,
    format_diff_counts, load_snapshot, save_snapshot,
};
use eframe::egui;
use palette_core::Palette;
use palette_core::egui::{to_color32, to_egui_visuals};
use palette_core::resolved::ResolvedPalette;
use panes::{FocusDirection, PanelId};
use rustc_hash::FxHashMap;

const ANIM_DURATION_SECS: f64 = demo_presets::ANIM_DURATION_SECS as f64;

type InputSnapshot = (
    Vec<(egui::Key, egui::Modifiers, f64)>,
    f32,
    Option<(f64, egui::Pos2)>,
);

/// Pre-format help binding strings (called once at startup).
fn build_help_lines() -> Box<[Box<str>]> {
    HELP_BINDINGS_GUI
        .iter()
        .map(|b| format!("{:16}{}", b.key, b.action).into_boxed_str())
        .collect()
}

fn lerp_rect(a: egui::Rect, b: egui::Rect, t: f32) -> egui::Rect {
    egui::Rect::from_min_max(
        egui::pos2(
            a.min.x + (b.min.x - a.min.x) * t,
            a.min.y + (b.min.y - a.min.y) * t,
        ),
        egui::pos2(
            a.max.x + (b.max.x - a.max.x) * t,
            a.max.y + (b.max.y - a.max.y) * t,
        ),
    )
}

fn chromatic_accents(resolved: &ResolvedPalette) -> [egui::Color32; 12] {
    chromatic_colors(resolved).map(to_color32)
}

struct OverlayRect {
    kind: Box<str>,
    rect: egui::Rect,
}

struct DemoApp {
    state: DemoState,
    palette: Palette,
    resolved: ResolvedPalette,
    accents: [egui::Color32; 12],
    prev_rects: FxHashMap<PanelId, egui::Rect>,
    current_rects: FxHashMap<PanelId, egui::Rect>,
    anim_from: FxHashMap<PanelId, egui::Rect>,
    anim_start: Option<f64>,
    needs_theme_reload: bool,
    pending_overlays: Vec<OverlayRect>,
    /// Pre-formatted help binding strings (static content, built once)
    cached_help_lines: Box<[Box<str>]>,
}

impl DemoApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Result<Self, DemoError> {
        let mut state = DemoState::new(8.0)?;
        state.set_help_binding_count(HELP_BINDINGS_GUI.len());
        load_snapshot(&mut state);
        let palette = state.load_current_palette()?;
        let resolved = palette.resolve();
        let accents = chromatic_accents(&resolved);
        cc.egui_ctx.set_visuals(to_egui_visuals(&palette));

        let cached_help_lines = build_help_lines();

        Ok(Self {
            state,
            palette,
            resolved,
            accents,
            prev_rects: FxHashMap::default(),
            current_rects: FxHashMap::default(),
            anim_from: FxHashMap::default(),
            anim_start: None,
            needs_theme_reload: false,
            pending_overlays: Vec::new(),
            cached_help_lines,
        })
    }

    fn reload_theme(&mut self, ctx: &egui::Context) -> bool {
        let Ok(palette) = self.state.load_current_palette() else {
            return false;
        };
        ctx.set_visuals(to_egui_visuals(&palette));
        self.resolved = palette.resolve();
        self.accents = chromatic_accents(&self.resolved);
        self.palette = palette;
        true
    }

    fn snapshot_and_animate(&mut self, time: f64) {
        self.anim_from = std::mem::take(&mut self.prev_rects);
        self.anim_start = Some(time);
    }

    fn is_animating(&self, time: f64) -> bool {
        match self.anim_start {
            Some(start) => (time - start) < ANIM_DURATION_SECS,
            None => false,
        }
    }

    fn handle_input(&mut self, ctx: &egui::Context) {
        // Extract key-press tuples, scroll delta, and primary click position
        let (keys, scroll_delta, click_pos): InputSnapshot = ctx.input(|i| {
            let keys = i
                .events
                .iter()
                .filter_map(|ev| match ev {
                    egui::Event::Key {
                        key,
                        pressed: true,
                        modifiers,
                        ..
                    } => Some((*key, *modifiers, i.time)),
                    _ => None,
                })
                .collect();
            let click = i
                .pointer
                .button_clicked(egui::PointerButton::Primary)
                .then(|| i.pointer.interact_pos())
                .flatten()
                .map(|pos| (i.time, pos));
            (keys, i.smooth_scroll_delta.y, click)
        });
        for (key, mods, time) in keys {
            self.handle_key(key, mods, time);
        }
        if scroll_delta.abs() > 0.1 {
            self.state.scroll_by(-scroll_delta);
        }
        if let Some((time, pos)) = click_pos {
            self.snapshot_and_animate(time);
            self.state.apply(Action::FocusAt(pos.x, pos.y));
        }
    }

    fn handle_key(&mut self, key: egui::Key, mods: egui::Modifiers, time: f64) {
        let Some(action) = egui_key_to_action(key, mods.shift) else {
            return;
        };
        if action.changes_layout() {
            self.snapshot_and_animate(time);
        }
        let layout_changed = self.state.apply(action);
        if !layout_changed {
            self.needs_theme_reload = true;
        }
    }
}

/// Map an egui key event to a renderer-agnostic `Action`.
fn egui_key_to_action(key: egui::Key, shift: bool) -> Option<Action> {
    match (key, shift) {
        (egui::Key::ArrowRight, false) => Some(Action::NextPreset),
        (egui::Key::ArrowLeft, false) => Some(Action::PrevPreset),
        (egui::Key::ArrowDown, false) => Some(Action::NextTheme),
        (egui::Key::ArrowUp, false) => Some(Action::PrevTheme),
        (egui::Key::Tab, false) => Some(Action::FocusNext),
        (egui::Key::Tab, true) => Some(Action::FocusPrev),
        (egui::Key::H, true) => Some(Action::FocusDirection(FocusDirection::Left)),
        (egui::Key::J, true) => Some(Action::FocusDirection(FocusDirection::Down)),
        (egui::Key::K, true) => Some(Action::FocusDirection(FocusDirection::Up)),
        (egui::Key::L, true) => Some(Action::FocusDirection(FocusDirection::Right)),
        (egui::Key::A, false) => Some(Action::AddPanel),
        (egui::Key::D, false) => Some(Action::RemovePanel),
        (egui::Key::C, false) => Some(Action::ToggleCollapsed),
        (egui::Key::OpenBracket, false) => Some(Action::SwapPrev),
        (egui::Key::CloseBracket, false) => Some(Action::SwapNext),
        (egui::Key::Equals | egui::Key::Plus, false) => Some(Action::ResizeHorizontal(0.05)),
        (egui::Key::Minus, false) => Some(Action::ResizeHorizontal(-0.05)),
        (egui::Key::Equals | egui::Key::Plus, true) => Some(Action::ResizeVertical(0.05)),
        (egui::Key::Minus, true) => Some(Action::ResizeVertical(-0.05)),
        (egui::Key::Questionmark, _) => Some(Action::ToggleHelp),
        _ => None,
    }
}

fn render_header(app: &mut DemoApp, ctx: &egui::Context, time: f64) {
    egui::TopBottomPanel::top("header").show(ctx, |ui| {
        ui.horizontal(|ui| {
            render_preset_combo(app, ui, time);
            ui.separator();
            render_theme_combo(app, ui, ctx);
        });
    });
}

fn render_preset_combo(app: &mut DemoApp, ui: &mut egui::Ui, time: f64) {
    ui.label("Preset:");
    let preset_name = app.state.preset_name().to_string();
    let _response = egui::ComboBox::from_id_salt("preset")
        .selected_text(&preset_name)
        .show_ui(ui, |ui| {
            for preset in app.state.presets() {
                if ui
                    .selectable_label(preset.name == preset_name, preset.name)
                    .clicked()
                {
                    app.snapshot_and_animate(time);
                    let _ = app.state.switch_preset(preset.name);
                }
            }
        });
}

fn render_theme_combo(app: &mut DemoApp, ui: &mut egui::Ui, ctx: &egui::Context) {
    ui.label("Theme:");
    // Clone Arc<str> (cheap ref-count bump) so we can mutate app inside the closure
    let Some(current_info) = app.state.current_theme() else {
        return;
    };
    let current_name = Arc::clone(&current_info.name);
    // Collect Arc<str> refs — no String allocation per theme, just Arc ref-count bumps
    let names: Vec<Arc<str>> = app
        .state
        .themes()
        .iter()
        .map(|t| Arc::clone(&t.name))
        .collect();
    egui::ComboBox::from_id_salt("theme")
        .selected_text(&*current_name)
        .show_ui(ui, |ui| {
            for name in &names {
                if ui
                    .selectable_label(**name == *current_name, &**name)
                    .clicked()
                {
                    let _ = app.state.switch_theme(name);
                    app.reload_theme(ctx);
                }
            }
        });
}

fn render_status(app: &DemoApp, ctx: &egui::Context) {
    egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
        render_status_lines(app, ui);
    });
}

fn render_status_lines(app: &DemoApp, ui: &mut egui::Ui) {
    let sd = demo_presets::status_data(&app.state);
    ui.label(format!(
        "preset: {}{} │ theme: {}{}{}{}",
        sd.preset_name,
        sd.preset_position,
        sd.theme_name,
        sd.theme_style,
        sd.theme_position,
        sd.panel_marker,
    ));

    ui.horizontal(|ui| {
        ui.label(format!("focus: {}", sd.focus_text));
        ui.separator();
        ui.label("←/→ preset  ↑/↓ theme  Tab focus  Shift+HJKL spatial  A/D add/rm  C collapse  [/] swap  +/- resize horiz  S+/S- resize vert  ? help");
    });
}

fn render_viewport(app: &mut DemoApp, ctx: &egui::Context) {
    egui::CentralPanel::default().show(ctx, |ui| {
        let available = ui.available_rect_before_wrap();
        render_panels(app, ui, available);
    });
}

fn render_panels(app: &mut DemoApp, ui: &mut egui::Ui, available: egui::Rect) {
    let frame_result = app.state.resolve(available.width(), available.height());
    let rt_frame = match frame_result {
        Ok(f) => f,
        Err(e) => {
            ui.colored_label(egui::Color32::RED, format!("resolve error: {e}"));
            return;
        }
    };

    let layout = rt_frame.layout();
    let focused_pid = app.state.focused_pid();
    let painter = ui.painter();
    let time = ui.input(|i| i.time);

    let bg_color = to_color32(&app.resolved.base.background);
    let fg_color = to_color32(&app.resolved.base.foreground);
    let hi_bg = to_color32(&app.resolved.base.background_highlight);
    let border_color = to_color32(&app.resolved.base.border);
    let focus_border = to_color32(&app.resolved.surface.focus);
    let comment_color = to_color32(&app.resolved.typography.comment);

    app.current_rects.clear();

    for entry in panes_egui::panels(layout) {
        let rect = entry.rect.translate(available.min.to_vec2());
        app.current_rects.insert(entry.id, rect);
        let is_focused = focused_pid == Some(entry.id);

        let display_rect = anim_rect(app, entry.id, rect, time);
        let panel_bg = match is_focused {
            true => hi_bg,
            false => bg_color,
        };

        painter.rect_filled(display_rect, 0.0, panel_bg);

        let (stroke_color, stroke_width) = match is_focused {
            true => (focus_border, 2.0),
            false => (border_color, 1.0),
        };
        painter.rect_stroke(
            display_rect,
            0.0,
            egui::Stroke::new(stroke_width, stroke_color),
            egui::StrokeKind::Inside,
        );

        let label_color = match is_focused {
            true => app.accents[entry.kind_index % app.accents.len()],
            false => fg_color,
        };
        paint_label(
            painter,
            display_rect,
            entry.kind,
            label_color,
            comment_color,
        );
    }

    std::mem::swap(&mut app.prev_rects, &mut app.current_rects);

    // Stash overlay rects from panes for rendering as egui::Area after CentralPanel
    app.pending_overlays.clear();
    for overlay in panes_egui::overlays_at(layout, available.min) {
        app.pending_overlays.push(OverlayRect {
            kind: Box::from(overlay.kind),
            rect: overlay.rect,
        });
    }

    paint_diff_overlay(app, painter, available, comment_color);
}

fn anim_rect(app: &DemoApp, id: PanelId, target: egui::Rect, time: f64) -> egui::Rect {
    let (Some(start), Some(from)) = (app.anim_start, app.anim_from.get(&id)) else {
        return target;
    };
    let t = ((time - start) as f32 / ANIM_DURATION_SECS as f32).min(1.0);
    lerp_rect(*from, target, ease_out(t))
}

fn paint_label(
    painter: &egui::Painter,
    rect: egui::Rect,
    kind: &str,
    label_color: egui::Color32,
    comment_color: egui::Color32,
) {
    let galley = painter.layout_no_wrap(
        kind.to_string(),
        egui::FontId::proportional(14.0),
        label_color,
    );
    let text_pos = egui::pos2(
        rect.center().x - galley.size().x / 2.0,
        rect.center().y - galley.size().y / 2.0 - 8.0,
    );
    painter.galley(text_pos, galley, label_color);

    let dims = format!("{:.0}×{:.0}", rect.width(), rect.height());
    let dims_galley = painter.layout_no_wrap(dims, egui::FontId::proportional(11.0), comment_color);
    let dims_pos = egui::pos2(
        rect.center().x - dims_galley.size().x / 2.0,
        rect.center().y - dims_galley.size().y / 2.0 + 8.0,
    );
    painter.galley(dims_pos, dims_galley, comment_color);
}

fn render_overlays(app: &DemoApp, ctx: &egui::Context) {
    for overlay in &app.pending_overlays {
        render_single_overlay(app, ctx, overlay);
    }
}

fn render_single_overlay(app: &DemoApp, ctx: &egui::Context, overlay: &OverlayRect) {
    let overlay_bg = to_color32(&app.resolved.surface.float);
    let border_color = to_color32(&app.resolved.surface.focus);

    let frame = egui::Frame::new()
        .fill(overlay_bg)
        .stroke(egui::Stroke::new(2.0, border_color))
        .corner_radius(6.0)
        .inner_margin(12.0);

    egui::Area::new(egui::Id::new(&overlay.kind))
        .fixed_pos(overlay.rect.min)
        .constrain(false)
        .show(ctx, |ui| {
            ui.set_min_size(overlay.rect.size());
            ui.set_max_size(overlay.rect.size());
            frame.show(ui, |ui| paint_help_content(ui, &overlay.kind, app));
        });
}

fn paint_help_content(ui: &mut egui::Ui, title: &str, app: &DemoApp) {
    let muted = to_color32(&app.resolved.typography.comment);
    ui.heading(title);
    ui.add_space(4.0);
    // Text is pre-formatted; only styling varies with theme
    for line in &*app.cached_help_lines {
        ui.label(
            egui::RichText::new(&**line)
                .monospace()
                .size(12.0)
                .color(muted),
        );
    }
}

fn paint_diff_overlay(
    app: &DemoApp,
    painter: &egui::Painter,
    available: egui::Rect,
    comment_color: egui::Color32,
) {
    let diff = app.state.last_diff();
    let diff_text = format_diff_counts(&diff);
    let galley = painter.layout_no_wrap(diff_text, egui::FontId::proportional(11.0), comment_color);
    let pos = egui::pos2(
        available.min.x + 4.0,
        available.max.y - galley.size().y - 4.0,
    );
    painter.galley(pos, galley, comment_color);
}

impl eframe::App for DemoApp {
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        save_snapshot(&self.state);
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_input(ctx);

        if self.needs_theme_reload {
            self.needs_theme_reload = false;
            self.reload_theme(ctx);
        }

        let time = ctx.input(|i| i.time);

        render_header(self, ctx, time);
        render_status(self, ctx);
        render_viewport(self, ctx);
        render_overlays(self, ctx);

        // Clear animation after duration
        match self.anim_start {
            Some(start) if (time - start) >= ANIM_DURATION_SECS => {
                self.anim_start = None;
                self.anim_from.clear();
            }
            _ => {}
        }

        if self.is_animating(time) {
            ctx.request_repaint();
        }
    }
}

fn run() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([900.0, 600.0]),
        ..Default::default()
    };
    eframe::run_native(
        "panes-egui-demo",
        options,
        Box::new(|cc| {
            let app = DemoApp::new(cc).map_err(Box::new)?;
            Ok(Box::new(app))
        }),
    )
}

fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            let _ = std::io::Write::write_fmt(&mut std::io::stderr(), format_args!("error: {e}\n"));
            std::process::ExitCode::FAILURE
        }
    }
}
