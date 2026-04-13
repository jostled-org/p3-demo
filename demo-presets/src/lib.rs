mod action;
mod anim;
mod color_util;
mod diff_fmt;
mod help;
mod persist;
mod presets;
mod resize;
mod snapshot;
mod state;
mod status;

pub use action::Action;
pub use anim::{ANIM_DURATION_SECS, ease_out};
pub use color_util::{chromatic_colors, text_on_color};
pub use diff_fmt::format_diff_counts;
pub use help::{HELP_BINDINGS_GUI, HELP_BINDINGS_TUI, HelpBinding, build_help_line};
pub use persist::{load_snapshot, save_snapshot};
pub use presets::{
    BreakpointTier, DemoPresetInfo, HELP_OVERLAY_KIND, build_adaptive, build_chrome,
    build_css_dashboard, build_css_dashboard_with_overlays, build_css_overlay_defs, build_default,
    build_grid_showcase, demo_presets, help_overlay,
};
pub use snapshot::DemoSnapshot;
pub use state::{DemoError, DemoState, build_preset};
pub use status::{StatusData, status_data};
