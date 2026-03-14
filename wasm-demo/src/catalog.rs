use serde::Serialize;

#[derive(Serialize)]
pub struct PresetDesc {
    pub name: &'static str,
    pub input: &'static str,
    pub description: &'static str,
}
