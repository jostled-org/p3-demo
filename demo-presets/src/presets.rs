use std::sync::Arc;

use panes::runtime::LayoutRuntime;
use panes::{CardSpan, Layout, LayoutTree, Overlay, PaneError, PanelSequence, fixed, grow};

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

/// Determine the breakpoint tier for a given viewport width.
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
    let iter = || panels.iter().map(Arc::clone);
    let gap = cell;
    let rt = match tier {
        BreakpointTier::Narrow => Layout::monocle(iter()).into_runtime(),
        BreakpointTier::Medium => Layout::master_stack(iter())
            .master_ratio(0.6)
            .gap(gap)
            .into_runtime(),
        BreakpointTier::Wide => Layout::dwindle(iter()).ratio(0.5).gap(gap).into_runtime(),
    };
    rt.ok()
}

/// Content+status chrome layout: content area grows, status bar is fixed height.
pub fn build_chrome() -> Option<LayoutRuntime> {
    let layout = Layout::build_col(|c| {
        c.panel("content");
        c.panel_with("status", fixed(3.0));
    })
    .ok()?;
    Some(LayoutRuntime::new(layout.into()))
}

/// Help overlay: centered, 60% wide, fixed height for content.
pub fn help_overlay() -> Overlay {
    Overlay::center().percent_width(60.0).height(14.0)
}

/// The overlay kind used for the help panel.
pub const HELP_OVERLAY_KIND: &str = "help";

/// No-strategy runtime with hyprland-style auto-tiling.
///
/// Panels are seeded in a flat row. Adding panels splits the focused panel
/// based on aspect ratio. Resize works on all sibling boundaries.
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

/// CSS showcase dashboard layout.
pub fn build_css_dashboard() -> Result<Layout, PaneError> {
    Layout::dashboard([
        ("base-colors", CardSpan::FullWidth),
        ("semantic-colors", CardSpan::FullWidth),
        ("surface-colors", CardSpan::FullWidth),
        ("typography", CardSpan::FullWidth),
        ("syntax", CardSpan::FullWidth),
        ("editor", CardSpan::FullWidth),
        ("diff", CardSpan::FullWidth),
        ("ansi-terminal", CardSpan::FullWidth),
    ])
    .auto_fill(380.0)
    .gap(20.0)
    .build()
}
