use std::sync::Arc;

use panes::runtime::LayoutRuntime;
use panes::{Layout, PresetInfo};

/// Build a `LayoutRuntime` for a named preset with demo-specific defaults.
///
/// Ratios, gaps, and column counts are tuned for the p3-demo showcase.
/// Returns `None` for unrecognized preset names.
pub fn build_preset(info: &PresetInfo, panels: &[Arc<str>]) -> Option<LayoutRuntime> {
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
        "split" => {
            let first = panels.first().map(Arc::clone)?;
            let second = panels.get(1).map_or_else(|| Arc::from("empty"), Arc::clone);
            Layout::split(first, second)
                .ratio(0.5)
                .gap(1.0)
                .into_runtime()
        }
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
