use std::borrow::Cow;

use crate::DemoState;

/// Raw status values — renderers format these however they need.
pub struct StatusData<'a> {
    pub preset_name: &'a str,
    pub preset_idx: usize,
    pub preset_count: usize,
    pub theme_name: &'a str,
    pub theme_style: &'a str,
    pub theme_idx: usize,
    pub theme_count: usize,
    pub panel_count: usize,
    pub is_dynamic: bool,
    pub focus_text: Cow<'a, str>,
}

pub fn status_data(state: &DemoState) -> StatusData<'_> {
    let (theme_name, theme_style): (&str, &str) = match state.current_theme() {
        Some(info) => (info.name.as_ref(), info.style.as_ref()),
        None => ("unknown", "unknown"),
    };

    let focus_text = match (state.focused_pid(), state.focused_kind()) {
        (Some(_), Some(kind)) => Cow::Borrowed(kind),
        (Some(pid), None) => Cow::Owned(format!("{pid}")),
        _ => Cow::Borrowed("none"),
    };

    StatusData {
        preset_name: state.preset_name(),
        preset_idx: state.preset_idx(),
        preset_count: state.preset_count(),
        theme_name,
        theme_style,
        theme_idx: state.theme_idx(),
        theme_count: state.theme_count(),
        panel_count: state.panel_count(),
        is_dynamic: state.is_dynamic(),
        focus_text,
    }
}
