use panes::LayoutSnapshot;

/// Serializable snapshot of the full demo state.
///
/// Wraps a panes `LayoutSnapshot` with the preset name so that
/// `DemoState::restore` can set the correct preset index.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DemoSnapshot {
    preset: Box<str>,
    layout: LayoutSnapshot,
}

impl DemoSnapshot {
    pub(crate) fn new(preset: Box<str>, layout: LayoutSnapshot) -> Self {
        Self { preset, layout }
    }

    pub fn preset(&self) -> &str {
        &self.preset
    }

    pub fn into_layout(self) -> (Box<str>, LayoutSnapshot) {
        (self.preset, self.layout)
    }
}
