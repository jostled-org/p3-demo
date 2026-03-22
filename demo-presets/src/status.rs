use std::borrow::Cow;

use crate::DemoState;

pub struct StatusData<'a> {
    pub preset_name: &'a str,
    /// Pre-formatted with leading space: " (N/M)"
    pub preset_position: Box<str>,
    pub theme_name: &'a str,
    /// Pre-formatted with brackets: " [style]"
    pub theme_style: Box<str>,
    /// Pre-formatted with leading space: " (N/M)"
    pub theme_position: Box<str>,
    /// Pre-formatted with leading separator: " │ panels: N" or " │ [fixed]"
    pub panel_marker: Box<str>,
    pub focus_text: Cow<'a, str>,
}

pub fn status_data(state: &DemoState) -> StatusData<'_> {
    let (theme_name, theme_style): (&str, &str) = match state.current_theme() {
        Some(info) => (info.name.as_ref(), info.style.as_ref()),
        None => ("unknown", "unknown"),
    };

    let panel_marker: Box<str> = match state.is_dynamic() {
        true => format!(" │ panels: {}", state.panel_count()).into_boxed_str(),
        false => Box::from(" │ [fixed]"),
    };

    let focus_text = match (state.focused_pid(), state.focused_kind()) {
        (Some(_), Some(kind)) => Cow::Borrowed(kind),
        (Some(pid), None) => Cow::Owned(format!("{pid}")),
        _ => Cow::Borrowed("none"),
    };

    StatusData {
        preset_name: state.preset_name(),
        preset_position: format!(" ({}/{})", state.preset_idx() + 1, state.preset_count())
            .into_boxed_str(),
        theme_name,
        theme_style: format!(" [{}]", theme_style).into_boxed_str(),
        theme_position: format!(" ({}/{})", state.theme_idx() + 1, state.theme_count())
            .into_boxed_str(),
        panel_marker,
        focus_text,
    }
}
