use panes::LayoutSnapshot;

/// Serializable snapshot pairing a preset name with a `LayoutSnapshot`.
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
