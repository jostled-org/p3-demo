use std::sync::Arc;

use panes::runtime::LayoutRuntime;
use panes::{
    CardSpan, Layout, LayoutTree, Overlay, OverlayDef, PaneError, PanelSequence, fixed, grow,
};

/// Viewport width breakpoint tiers for the adaptive preset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreakpointTier {
    /// < 80 wide: monocle (single panel)
    Narrow,
    /// 80–160 wide: master-stack
    Medium,
    /// > 160 wide: dwindle
    Wide,
}

const BREAKPOINT_MEDIUM: f32 = 80.0;
const BREAKPOINT_WIDE: f32 = 160.0;

pub fn breakpoint_tier(width: f32) -> BreakpointTier {
    match () {
        () if width >= BREAKPOINT_WIDE => BreakpointTier::Wide,
        () if width >= BREAKPOINT_MEDIUM => BreakpointTier::Medium,
        () => BreakpointTier::Narrow,
    }
}

/// Build a runtime for the adaptive preset at a given breakpoint tier.
pub fn build_adaptive(
    panels: &[Arc<str>],
    cell: f32,
    tier: BreakpointTier,
) -> Option<LayoutRuntime> {
    let name = match tier {
        BreakpointTier::Narrow => "monocle",
        BreakpointTier::Medium => "master-stack",
        BreakpointTier::Wide => "dwindle",
    };
    let presets = Layout::presets();
    let info = presets.iter().find(|p| p.name == name)?;
    crate::state::build_preset(info, panels, cell)
}

/// Content area + fixed-height status bar.
pub fn build_chrome() -> Option<LayoutRuntime> {
    let layout = Layout::build_col(|c| {
        c.panel("content");
        c.panel_with("status", fixed(3.0));
    })
    .ok()?;
    Some(LayoutRuntime::new(layout.into()))
}

pub fn help_overlay() -> Overlay {
    Overlay::center().percent_width(60.0).height(14.0)
}

pub const HELP_OVERLAY_KIND: &str = "help";

/// Flat row of panels with no tiling strategy (hyprland-style auto-tiling).
pub fn build_default(panels: &[Arc<str>], gap: f32) -> Result<LayoutRuntime, PaneError> {
    let mut tree = LayoutTree::new();
    let mut sequence = PanelSequence::default();
    let mut nids = Vec::with_capacity(panels.len());
    for p in panels {
        let (pid, nid) = tree.add_panel(Arc::clone(p), grow(1.0))?;
        sequence.push(pid);
        nids.push(nid);
    }
    let root = tree.add_row(gap, nids)?;
    tree.set_root(root);
    Ok(LayoutRuntime::from_tree_and_sequence(tree, sequence))
}

/// Build overlay definitions for the CSS dashboard.
pub fn build_css_overlay_defs(layout: Layout) -> Result<Box<[OverlayDef]>, PaneError> {
    let mut rt = LayoutRuntime::from(layout);
    rt.add_overlay(HELP_OVERLAY_KIND, help_overlay())?;
    Ok(rt.overlays().to_vec().into_boxed_slice())
}

/// CSS showcase dashboard layout with overlay definitions.
pub fn build_css_dashboard_with_overlays() -> Result<(Layout, Box<[OverlayDef]>), PaneError> {
    let overlay_defs = build_css_overlay_defs(build_css_dashboard()?)?;
    let layout = build_css_dashboard()?;
    Ok((layout, overlay_defs))
}

pub fn build_css_dashboard() -> Result<Layout, PaneError> {
    Layout::dashboard([
        ("base-colors", CardSpan::Columns(2)),
        ("semantic-colors", CardSpan::Columns(1)),
        ("surface-colors", CardSpan::Columns(1)),
        ("typography", CardSpan::Columns(2)),
        ("syntax", CardSpan::FullWidth),
        ("editor", CardSpan::Columns(2)),
        ("diff", CardSpan::FullWidth),
        ("ansi-terminal", CardSpan::FullWidth),
    ])
    .auto_fill(380.0)
    .auto_rows()
    .gap(20.0)
    .build()
}
