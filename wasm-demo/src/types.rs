use serde::Serialize;
use serde::ser::SerializeStruct;

#[derive(Serialize)]
pub struct BaseRect {
    pub id: u32,
    pub kind: Box<str>,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

pub struct PanelRect {
    pub base: BaseRect,
    pub focused: bool,
    pub kind_index: usize,
}

impl Serialize for PanelRect {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut s = serializer.serialize_struct("PanelRect", 8)?;
        s.serialize_field("id", &self.base.id)?;
        s.serialize_field("kind", &self.base.kind)?;
        s.serialize_field("x", &self.base.x)?;
        s.serialize_field("y", &self.base.y)?;
        s.serialize_field("w", &self.base.w)?;
        s.serialize_field("h", &self.base.h)?;
        s.serialize_field("focused", &self.focused)?;
        s.serialize_field("kind_index", &self.kind_index)?;
        s.end()
    }
}
