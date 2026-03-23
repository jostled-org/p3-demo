use std::sync::Arc;

use palette_core::registry::{Registry, ThemeInfo};
use palette_core::{Palette, PaletteError};
use panes::diff::{LayoutDiff, OverlayDiff};
use panes::runtime::{Frame as PanesFrame, LayoutRuntime};
use panes::{
    BoundaryAxis, BoundaryHit, FocusDirection, Layout, Node, PaneError, PanelId, PanelInputKind,
    PresetInfo,
};

use crate::action::Action;
use crate::help;
use crate::presets;
use crate::presets::BreakpointTier;
use crate::resize;
use crate::snapshot::DemoSnapshot;

pub(crate) const DEFAULT_PANELS: &[&str] = &["editor", "terminal", "logs"];

#[derive(thiserror::Error, Debug)]
pub enum DemoError {
    #[error("no themes found in registry")]
    NoThemes,
    #[error("no presets available")]
    NoPresets,
    #[error("preset is not dynamic")]
    NotDynamic,
    #[error("no runtime available")]
    NoRuntime,
    #[error("unknown preset: {0}")]
    UnknownPreset(Box<str>),
    #[error("unknown theme: {0}")]
    UnknownTheme(Box<str>),
    #[error("unknown direction: {0}")]
    UnknownDirection(Box<str>),
    #[error("failed to load theme '{0}': {1}")]
    ThemeLoad(Arc<str>, PaletteError),
    #[error("layout error: {0}")]
    Panes(#[from] PaneError),
}

/// Build a `LayoutRuntime` for a named preset with demo-specific defaults.
///
/// Ratios, gaps, and column counts are tuned for the p3-demo showcase.
/// `cell` scales absolute values (gaps, widths, heights) to the renderer's
/// coordinate system. Ratatui passes `1.0` (terminal cells), egui passes
/// a cell-to-pixel ratio (e.g. `8.0`).
/// Returns `None` for unrecognized preset names.
pub fn build_preset(info: &PresetInfo, panels: &[Arc<str>], cell: f32) -> Option<LayoutRuntime> {
    let iter = || panels.iter().map(Arc::clone);
    let cards = || -> Vec<(Arc<str>, usize)> { iter().map(|p| (p, 1)).collect() };
    let gap = cell;
    let rt = match info.name {
        "master-stack" => Layout::master_stack(iter())
            .master_ratio(0.6)
            .gap(gap)
            .into_runtime(),
        "centered-master" => Layout::centered_master(iter())
            .master_ratio(0.5)
            .gap(gap)
            .into_runtime(),
        "monocle" => Layout::monocle(iter()).into_runtime(),
        "scrollable" => Layout::scrollable(iter()).gap(gap).into_runtime(),
        "dwindle" => Layout::dwindle(iter()).ratio(0.5).gap(gap).into_runtime(),
        "spiral" => Layout::spiral(iter()).ratio(0.5).gap(gap).into_runtime(),
        "columns" | "grid" => Layout::dashboard(cards())
            .columns(3)
            .gap(gap)
            .into_runtime(),
        "deck" => Layout::deck(iter())
            .master_ratio(0.7)
            .gap(gap)
            .into_runtime(),
        "tabbed" => Layout::tabbed(iter()).bar_height(3.0 * cell).into_runtime(),
        "stacked" => Layout::stacked(iter()).bar_height(cell).into_runtime(),
        "dashboard" => Layout::dashboard(cards())
            .auto_fill(30.0 * cell)
            .gap(gap)
            .into_runtime(),
        "split" => {
            let first = panels.first().map(Arc::clone)?;
            let second = panels.get(1).map_or_else(|| Arc::from("empty"), Arc::clone);
            Layout::split(first, second)
                .ratio(0.5)
                .gap(gap)
                .into_runtime()
        }
        "sidebar" => Layout::sidebar("nav", "content")
            .sidebar_width(20.0 * cell)
            .gap(gap)
            .into_runtime(),
        "holy-grail" => Layout::holy_grail("header", "footer", "left", "main", "right")
            .header_height(3.0 * cell)
            .footer_height(3.0 * cell)
            .sidebar_width(15.0 * cell)
            .gap(gap)
            .into_runtime(),
        "default" => return presets::build_default(panels, gap).ok(),
        _ => return None,
    };
    rt.ok()
}

fn attach_help_overlay(
    runtime: &mut Option<LayoutRuntime>,
    visible: bool,
    help_binding_count: usize,
    cell: f32,
) {
    let Some(rt) = runtime else { return };
    let _ = rt.add_overlay(presets::HELP_OVERLAY_KIND, presets::help_overlay());
    rt.set_overlay_visible(presets::HELP_OVERLAY_KIND, visible);
    let height = help_overlay_height(help_binding_count, cell);
    let _ = rt.set_overlay_height(presets::HELP_OVERLAY_KIND, height);
}

/// Compute help overlay height from binding count and cell size.
///
/// Each binding occupies one line (`cell` height). Two additional lines
/// account for the top/bottom border.
fn help_overlay_height(binding_count: usize, cell: f32) -> f32 {
    (binding_count as f32 + 2.0) * cell
}

/// Pick a `PanelId` from a `BoundaryHit` for resize.
///
/// Returns the panel id and a sign multiplier (+1 or -1) so that a positive
/// pixel delta in the boundary's axis maps to a positive resize delta.
/// `sides.0` is the before-sibling (left/top); if it is a panel, sign is +1.
/// Otherwise falls back to `sides.1` (right/bottom) with sign -1.
fn boundary_panel_id(rt: &LayoutRuntime, hit: BoundaryHit) -> Option<(PanelId, f32)> {
    let tree = rt.tree();
    if let Some(Node::Panel { id, .. }) = tree.node(hit.sides.0) {
        return Some((*id, 1.0));
    }
    if let Some(Node::Panel { id, .. }) = tree.node(hit.sides.1) {
        return Some((*id, -1.0));
    }
    None
}

#[derive(Debug, Clone, Copy)]
struct DragState {
    boundary: BoundaryHit,
    last: (f32, f32),
}

/// Shared state machine for all p3-demo renderers.
///
/// Manages presets, themes, panels, focus, and layout resolution.
/// Animation and rendering are left to each renderer.
pub struct DemoState {
    runtime: Option<LayoutRuntime>,
    last_frame: Option<PanesFrame>,
    last_viewport: (f32, f32),
    drag: Option<DragState>,
    panels: Vec<Arc<str>>,
    next_panel_id: usize,
    presets: &'static [PresetInfo],
    preset_idx: usize,
    adaptive_tier: Option<BreakpointTier>,
    registry: Registry,
    themes: Box<[ThemeInfo]>,
    theme_idx: usize,
    help_visible: bool,
    help_binding_count: usize,
    cell: f32,
}

impl DemoState {
    /// Create a new demo state.
    ///
    /// `cell` scales absolute layout values to the renderer's coordinate system.
    /// Pass `1.0` for terminal cells (ratatui), or a cell-to-pixel ratio for
    /// pixel-based renderers (egui, wasm).
    pub fn new(cell: f32) -> Result<Self, DemoError> {
        let registry = Registry::new();
        let themes: Box<[ThemeInfo]> = registry.list().cloned().collect();
        if themes.is_empty() {
            return Err(DemoError::NoThemes);
        }

        let presets = Layout::presets();
        if presets.is_empty() {
            return Err(DemoError::NoPresets);
        }

        let panels: Vec<Arc<str>> = DEFAULT_PANELS.iter().map(|s| Arc::from(*s)).collect();
        let help_binding_count = help::HELP_BINDINGS_TUI.len();
        let mut runtime = build_preset(&presets[0], &panels, cell);
        attach_help_overlay(&mut runtime, false, help_binding_count, cell);

        Ok(Self {
            runtime,
            last_frame: None,
            last_viewport: (0.0, 0.0),
            drag: None,
            panels,
            next_panel_id: DEFAULT_PANELS.len() + 1,
            presets,
            preset_idx: 0,
            adaptive_tier: None,
            registry,
            themes,
            theme_idx: 0,
            help_visible: false,
            help_binding_count,
            cell,
        })
    }

    // -- Preset navigation --

    pub fn presets(&self) -> &'static [PresetInfo] {
        self.presets
    }

    pub fn current_preset(&self) -> Option<&PresetInfo> {
        self.presets.get(self.preset_idx)
    }

    pub fn preset_name(&self) -> &str {
        match self.preset_idx.checked_sub(self.presets.len()) {
            None => self.presets[self.preset_idx].name,
            Some(0) => "default",
            Some(_) => "adaptive",
        }
    }

    pub fn is_default_layout(&self) -> bool {
        self.preset_idx == self.presets.len()
    }

    pub fn is_adaptive(&self) -> bool {
        self.preset_idx == self.presets.len() + 1
    }

    pub fn preset_idx(&self) -> usize {
        self.preset_idx
    }

    pub fn preset_count(&self) -> usize {
        self.presets.len() + 2 // +1 for "default", +1 for "adaptive"
    }

    pub fn next_preset(&mut self) {
        let total = self.preset_count();
        let idx = (self.preset_idx + 1) % total;
        self.set_preset(idx);
    }

    pub fn prev_preset(&mut self) {
        let total = self.preset_count();
        let idx = (self.preset_idx + total - 1) % total;
        self.set_preset(idx);
    }

    pub fn switch_preset(&mut self, name: &str) -> Result<(), DemoError> {
        let idx = self.preset_index_for_name(name)?;
        self.set_preset(idx);
        Ok(())
    }

    fn preset_index_for_name(&self, name: &str) -> Result<usize, DemoError> {
        match name {
            "default" => Ok(self.presets.len()),
            "adaptive" => Ok(self.presets.len() + 1),
            _ => self
                .presets
                .iter()
                .position(|p| p.name == name)
                .ok_or_else(|| DemoError::UnknownPreset(Box::from(name))),
        }
    }

    fn set_preset(&mut self, idx: usize) {
        self.preset_idx = idx;
        self.adaptive_tier = None;
        self.runtime = match idx.checked_sub(self.presets.len()) {
            None => build_preset(&self.presets[idx], &self.panels, self.cell),
            Some(0) => presets::build_default(&self.panels, self.cell).ok(),
            Some(_) => None, // adaptive: built lazily on first resolve()
        };
        attach_help_overlay(
            &mut self.runtime,
            self.help_visible,
            self.help_binding_count,
            self.cell,
        );
    }

    // -- Theme navigation --

    pub fn themes(&self) -> &[ThemeInfo] {
        &self.themes
    }

    pub fn current_theme(&self) -> Option<&ThemeInfo> {
        self.themes.get(self.theme_idx)
    }

    pub fn theme_idx(&self) -> usize {
        self.theme_idx
    }

    pub fn theme_count(&self) -> usize {
        self.themes.len()
    }

    pub fn next_theme(&mut self) {
        self.theme_idx = (self.theme_idx + 1) % self.themes.len();
    }

    pub fn prev_theme(&mut self) {
        self.theme_idx = (self.theme_idx + self.themes.len() - 1) % self.themes.len();
    }

    pub fn switch_theme(&mut self, name: &str) -> Result<(), DemoError> {
        let idx = self
            .themes
            .iter()
            .position(|t| t.name.as_ref() == name)
            .ok_or_else(|| DemoError::UnknownTheme(Box::from(name)))?;
        self.theme_idx = idx;
        Ok(())
    }

    pub fn load_palette(&self, id: &str) -> Result<Palette, DemoError> {
        self.registry
            .load(id)
            .map_err(|e| DemoError::ThemeLoad(Arc::from(id), e))
    }

    pub fn load_current_palette(&self) -> Result<Palette, DemoError> {
        let info = self.current_theme().ok_or(DemoError::NoThemes)?;
        self.registry
            .load(&info.id)
            .map_err(|e| DemoError::ThemeLoad(Arc::clone(&info.id), e))
    }

    // -- Focus --

    pub fn focus_next(&mut self) {
        if let Some(rt) = &mut self.runtime {
            rt.focus_next();
        }
    }

    pub fn focus_prev(&mut self) {
        if let Some(rt) = &mut self.runtime {
            rt.focus_prev();
        }
    }

    pub fn focus_direction(&mut self, dir: FocusDirection) {
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

    pub fn focus_direction_str(&mut self, dir: &str) -> Result<(), DemoError> {
        let direction = match dir {
            "left" => FocusDirection::Left,
            "right" => FocusDirection::Right,
            "up" => FocusDirection::Up,
            "down" => FocusDirection::Down,
            other => return Err(DemoError::UnknownDirection(Box::from(other))),
        };
        self.focus_direction(direction);
        Ok(())
    }

    pub fn focused_kind(&self) -> Option<&str> {
        self.runtime.as_ref()?.focused_kind()
    }

    pub fn focused_pid(&self) -> Option<PanelId> {
        self.runtime.as_ref().and_then(|rt| rt.focused())
    }

    /// Focus the panel at viewport coordinates, if any.
    ///
    /// Uses `panel_at_point` from the last resolved layout for hit-testing.
    /// Returns `true` if a panel was focused.
    ///
    /// Unlike `focus_next`/`focus_prev`/`focus_direction`, which delegate to
    /// `LayoutRuntime` methods that return `()`, this is a `DemoState`-only
    /// hit-test that can meaningfully report whether a panel was found.
    pub fn focus_at(&mut self, viewport_x: f32, viewport_y: f32) -> bool {
        let (Some(frame), Some(rt)) = (&self.last_frame, &mut self.runtime) else {
            return false;
        };
        let Some(pid) = frame.layout().panel_at_point(viewport_x, viewport_y) else {
            return false;
        };
        rt.focus(pid)
    }

    // -- Drag-to-resize --

    /// Check if a resize boundary is near the point. Returns axis for cursor styling.
    pub fn boundary_hover(&self, x: f32, y: f32) -> Option<BoundaryAxis> {
        let frame = self.last_frame.as_ref()?;
        let tolerance = 4.0 * self.cell;
        let hit = frame.layout().boundary_at_point(x, y, tolerance)?;
        Some(hit.axis)
    }

    /// Begin a drag if pointer is on a boundary. Returns true if drag started.
    pub fn drag_start(&mut self, x: f32, y: f32) -> bool {
        let Some(frame) = &self.last_frame else {
            return false;
        };
        let tolerance = 4.0 * self.cell;
        let Some(boundary) = frame.layout().boundary_at_point(x, y, tolerance) else {
            return false;
        };
        self.drag = Some(DragState {
            boundary,
            last: (x, y),
        });
        true
    }

    /// Continue a drag, applying resize delta. Returns true if resize occurred.
    pub fn drag_move(&mut self, x: f32, y: f32) -> bool {
        let Some(drag) = &mut self.drag else {
            return false;
        };
        let delta = match drag.boundary.axis {
            BoundaryAxis::Vertical => x - drag.last.0,
            BoundaryAxis::Horizontal => y - drag.last.1,
        };
        let axis = drag.boundary.axis;
        let boundary = drag.boundary;
        drag.last = (x, y);

        let Some(rt) = &mut self.runtime else {
            return false;
        };
        let Some((pid, sign)) = boundary_panel_id(rt, boundary) else {
            return false;
        };
        let viewport = match axis {
            BoundaryAxis::Vertical => self.last_viewport.0,
            BoundaryAxis::Horizontal => self.last_viewport.1,
        };
        let frac = match viewport > 0.0 {
            true => delta * sign / viewport,
            false => 0.0,
        };
        rt.resize_boundary(pid, frac).is_ok()
    }

    /// Whether a drag is currently active.
    pub fn is_dragging(&self) -> bool {
        self.drag.is_some()
    }

    /// End the current drag.
    pub fn drag_end(&mut self) {
        self.drag = None;
    }

    // -- Panel mutations --

    pub fn is_dynamic(&self) -> bool {
        match self.current_preset() {
            Some(info) => info.input == PanelInputKind::DynamicList,
            None => true,
        }
    }

    pub fn panel_count(&self) -> usize {
        self.panels.len()
    }

    pub fn add_panel(&mut self) -> Result<(), DemoError> {
        if !self.is_dynamic() {
            return Err(DemoError::NotDynamic);
        }
        let name: Arc<str> = format!("panel-{}", self.next_panel_id).into();
        self.next_panel_id += 1;
        self.panels.push(Arc::clone(&name));
        match self.runtime.as_mut() {
            Some(rt) => {
                rt.add_panel(name)?;
            }
            None => self.set_preset(self.preset_idx),
        }
        Ok(())
    }

    pub fn remove_panel(&mut self) -> Result<(), DemoError> {
        if !self.is_dynamic() {
            return Err(DemoError::NotDynamic);
        }
        let is_default = self.is_default_layout();
        let Some(rt_ref) = self.runtime.as_ref() else {
            return Ok(());
        };
        let Some(pid) = rt_ref.focused() else {
            return Ok(());
        };
        let kind = rt_ref
            .focused_kind()
            .and_then(|k| self.panels.iter().find(|p| p.as_ref() == k))
            .cloned();
        let rt = self.runtime.as_mut().ok_or(DemoError::NoRuntime)?;
        match is_default {
            true => {
                rt.focus_next();
                rt.tree_mut().remove_panel(pid)?;
            }
            false => {
                rt.remove_panel(pid)?;
            }
        }
        if let Some(kind) = &kind {
            self.panels.retain(|p| !Arc::ptr_eq(p, kind));
        }
        Ok(())
    }

    pub fn swap_next(&mut self) {
        if let Some(rt) = &mut self.runtime {
            rt.swap_next();
        }
    }

    pub fn swap_prev(&mut self) {
        if let Some(rt) = &mut self.runtime {
            rt.swap_prev();
        }
    }

    pub fn resize_horizontal(&mut self, delta: f32) {
        let Some(pid) = self.runtime.as_ref().and_then(|rt| rt.focused()) else {
            return;
        };
        let Some(rt) = &mut self.runtime else { return };
        let _ = rt.resize_boundary(pid, delta);
    }

    pub fn resize_vertical(&mut self, delta: f32) {
        let Some(rt) = &mut self.runtime else { return };
        let Some(pid) = rt.focused() else { return };
        let ancestor = resize::find_vertical_ancestor(rt.tree(), pid);
        let Some((container, target_child)) = ancestor else {
            return;
        };
        let _ = resize::redistribute_panel_grow(rt.tree_mut(), container, target_child, delta);
    }

    pub fn toggle_collapsed(&mut self) -> Result<(), DemoError> {
        let Some(pid) = self.focused_pid() else {
            return Ok(());
        };
        let rt = self.runtime.as_mut().ok_or(DemoError::NoRuntime)?;
        rt.toggle_collapsed(pid)?;
        Ok(())
    }

    // -- Scroll --

    pub fn scroll_by(&mut self, delta: f32) {
        if self.preset_name() == "scrollable" {
            let Some(rt) = &mut self.runtime else { return };
            let _ = rt.scroll_by(delta);
        }
    }

    // -- Snapshot --

    /// Save layout state for persistence.
    ///
    /// Returns `None` if no runtime is available (e.g. adaptive preset
    /// before first resolve) or if the runtime cannot produce a snapshot.
    pub fn snapshot(&self) -> Option<DemoSnapshot> {
        let rt = self.runtime.as_ref()?;
        let layout = rt.snapshot().ok()?;
        Some(DemoSnapshot::new(Box::from(self.preset_name()), layout))
    }

    /// Restore layout state from a snapshot.
    ///
    /// Rebuilds the runtime from the snapshot and sets the preset index
    /// to match the saved preset name.
    pub fn restore(&mut self, snap: DemoSnapshot) -> Result<(), DemoError> {
        let idx = self.preset_index_for_name(snap.preset())?;
        let (_, layout) = snap.into_layout();
        let rt = LayoutRuntime::from_snapshot(layout)?;
        self.preset_idx = idx;
        self.adaptive_tier = None;
        self.runtime = Some(rt);
        attach_help_overlay(
            &mut self.runtime,
            self.help_visible,
            self.help_binding_count,
            self.cell,
        );
        Ok(())
    }

    // -- Overlay --

    pub fn help_visible(&self) -> bool {
        self.help_visible
    }

    /// Set the number of help bindings for dynamic overlay sizing.
    ///
    /// Call this after construction if the renderer uses a different
    /// binding set (e.g. `HELP_BINDINGS_GUI`). The overlay height is
    /// recomputed on the next `toggle_help` or preset switch.
    pub fn set_help_binding_count(&mut self, count: usize) {
        self.help_binding_count = count;
        let Some(rt) = &mut self.runtime else { return };
        let height = help_overlay_height(count, self.cell);
        let _ = rt.set_overlay_height(presets::HELP_OVERLAY_KIND, height);
    }

    pub fn toggle_help(&mut self) {
        self.help_visible = !self.help_visible;
        let Some(rt) = &mut self.runtime else { return };
        rt.set_overlay_visible(presets::HELP_OVERLAY_KIND, self.help_visible);
        let height = help_overlay_height(self.help_binding_count, self.cell);
        let _ = rt.set_overlay_height(presets::HELP_OVERLAY_KIND, height);
    }

    /// The overlay diff from the most recent `resolve()` call.
    pub fn last_overlay_diff(&self) -> OverlayDiff<'_> {
        match self.runtime.as_ref() {
            Some(rt) => rt.last_overlay_diff(),
            None => OverlayDiff {
                added: &[],
                removed: &[],
                moved: &[],
                resized: &[],
                unchanged: &[],
            },
        }
    }

    // -- Action dispatch --

    /// Apply an input action and return whether the layout may have changed.
    ///
    /// Renderers use the return value to decide whether to snapshot for
    /// animation. Theme cycling returns `false` — only visual, no geometry.
    pub fn apply(&mut self, action: Action) -> bool {
        let changed = action.changes_layout();
        match action {
            Action::NextPreset => self.next_preset(),
            Action::PrevPreset => self.prev_preset(),
            Action::NextTheme => self.next_theme(),
            Action::PrevTheme => self.prev_theme(),
            Action::FocusNext => self.focus_next(),
            Action::FocusPrev => self.focus_prev(),
            Action::FocusDirection(dir) => self.focus_direction(dir),
            Action::AddPanel => {
                let _ = self.add_panel();
            }
            Action::RemovePanel => {
                let _ = self.remove_panel();
            }
            Action::ToggleCollapsed => {
                let _ = self.toggle_collapsed();
            }
            Action::SwapNext => self.swap_next(),
            Action::SwapPrev => self.swap_prev(),
            Action::ResizeHorizontal(d) => self.resize_horizontal(d),
            Action::ResizeVertical(d) => self.resize_vertical(d),
            Action::ScrollBy(d) => self.scroll_by(d),
            Action::FocusAt(x, y) => {
                let _ = self.focus_at(x, y);
            }
            Action::DragStart(x, y) => {
                let _ = self.drag_start(x, y);
            }
            Action::DragMove(x, y) => {
                let _ = self.drag_move(x, y);
            }
            Action::DragEnd => self.drag_end(),
            Action::ToggleHelp => self.toggle_help(),
        }
        changed
    }

    // -- Layout resolution --

    pub fn resolve(&mut self, width: f32, height: f32) -> Result<PanesFrame, DemoError> {
        if self.is_adaptive() {
            self.maybe_rebuild_adaptive(width);
        }
        let rt = self.runtime.as_mut().ok_or(DemoError::NoRuntime)?;
        let frame = rt.resolve(width, height)?;
        self.last_frame = Some(frame.clone());
        self.last_viewport = (width, height);
        Ok(frame)
    }

    /// Rebuild the adaptive runtime if the viewport crossed a breakpoint.
    fn maybe_rebuild_adaptive(&mut self, width: f32) {
        let tier = presets::breakpoint_tier(width);
        match self.adaptive_tier {
            Some(current) if current == tier => return,
            _ => {}
        }
        self.adaptive_tier = Some(tier);
        self.runtime = presets::build_adaptive(&self.panels, self.cell, tier);
        attach_help_overlay(
            &mut self.runtime,
            self.help_visible,
            self.help_binding_count,
            self.cell,
        );
    }

    /// The frame produced by the most recent `resolve()` call.
    ///
    /// Renderers that need the layout between resolve cycles (e.g. for
    /// overlay resolution) can borrow this instead of keeping their own clone.
    pub fn last_frame(&self) -> Option<&PanesFrame> {
        self.last_frame.as_ref()
    }

    pub fn last_diff(&self) -> LayoutDiff<'_> {
        match self.runtime.as_ref() {
            Some(rt) => rt.last_diff(),
            None => LayoutDiff {
                added: &[],
                removed: &[],
                moved: &[],
                resized: &[],
                unchanged: &[],
            },
        }
    }

    /// Direct access to the runtime for renderer-specific operations.
    pub fn runtime(&self) -> Option<&LayoutRuntime> {
        self.runtime.as_ref()
    }

    /// Mutable access to the runtime for renderer-specific operations.
    pub fn runtime_mut(&mut self) -> Option<&mut LayoutRuntime> {
        self.runtime.as_mut()
    }
}
