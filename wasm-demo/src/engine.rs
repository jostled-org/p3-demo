use std::sync::Arc;

use panes::runtime::LayoutRuntime;
use panes::{FocusDirection, Layout, PanelInputKind, PresetInfo};
use wasm_bindgen::prelude::*;

use crate::catalog::PresetDesc;
use crate::diff::DiffCounts;
use crate::error::EngineError;
use crate::types::PanelRect;
use demo_presets::build_preset;

const DEFAULT_PANELS: &[&str] = &["editor", "terminal", "logs"];

fn parse_direction(dir: &str) -> Result<FocusDirection, EngineError> {
    match dir {
        "left" => Ok(FocusDirection::Left),
        "right" => Ok(FocusDirection::Right),
        "up" => Ok(FocusDirection::Up),
        "down" => Ok(FocusDirection::Down),
        other => Err(EngineError::UnknownDirection(other.to_string())),
    }
}

#[wasm_bindgen]
pub struct JsLayoutEngine {
    runtime: LayoutRuntime,
    panels: Vec<Arc<str>>,
    next_panel_id: usize,
    presets: &'static [PresetInfo],
    preset_idx: usize,
}

#[wasm_bindgen]
impl JsLayoutEngine {
    #[wasm_bindgen(constructor)]
    pub fn new(preset_name: &str) -> Result<JsLayoutEngine, JsValue> {
        let presets = Layout::presets();
        let (idx, info) = presets
            .iter()
            .enumerate()
            .find(|(_, p)| p.name == preset_name)
            .ok_or_else(|| EngineError::UnknownPreset(preset_name.to_string()))?;

        let panels: Vec<Arc<str>> = DEFAULT_PANELS.iter().map(|s| Arc::from(*s)).collect();
        let runtime = build_preset(info, &panels)
            .ok_or_else(|| EngineError::UnknownPreset(preset_name.to_string()))?;

        Ok(Self {
            runtime,
            panels,
            next_panel_id: DEFAULT_PANELS.len() + 1,
            presets,
            preset_idx: idx,
        })
    }

    /// Returns JSON: [{id, kind, x, y, w, h, focused, kind_index}]
    pub fn resolve(&mut self, width: f32, height: f32) -> Result<String, JsValue> {
        let frame = self
            .runtime
            .resolve(width, height)
            .map_err(EngineError::from)?;
        let layout = frame.layout();
        let focused = self.runtime.focused();

        let rects: Vec<PanelRect> = panes_wasm::panels(layout)
            .map(|entry| PanelRect {
                id: entry.id.raw(),
                kind: entry.kind.to_string(),
                x: entry.rect.x,
                y: entry.rect.y,
                w: entry.rect.w,
                h: entry.rect.h,
                focused: focused == Some(entry.id),
                kind_index: entry.kind_index,
            })
            .collect();

        serde_json::to_string(&rects).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Returns JSON: {added, removed, resized, moved, unchanged}
    pub fn diff_counts(&self) -> String {
        let diff = self.runtime.last_diff();
        let counts = DiffCounts {
            added: diff.added.len(),
            removed: diff.removed.len(),
            resized: diff.resized.len(),
            moved: diff.moved.len(),
            unchanged: diff.unchanged.len(),
        };
        serde_json::to_string(&counts).unwrap_or_default()
    }

    pub fn switch_preset(&mut self, name: &str) -> Result<(), JsValue> {
        let (idx, info) = self
            .presets
            .iter()
            .enumerate()
            .find(|(_, p)| p.name == name)
            .ok_or_else(|| EngineError::UnknownPreset(name.to_string()))?;

        let rt = build_preset(info, &self.panels)
            .ok_or_else(|| EngineError::UnknownPreset(name.to_string()))?;
        self.runtime = rt;
        self.preset_idx = idx;
        Ok(())
    }

    pub fn focus_next(&mut self) {
        self.runtime.focus_next();
    }

    pub fn focus_prev(&mut self) {
        self.runtime.focus_prev();
    }

    pub fn focus_direction(&mut self, dir: &str) -> Result<(), JsValue> {
        let direction = parse_direction(dir)?;
        let spatial_result = self.runtime.focus_direction_current(direction);
        match (spatial_result, direction) {
            (Ok(_), _) => {}
            (Err(_), FocusDirection::Right | FocusDirection::Down) => self.runtime.focus_next(),
            (Err(_), FocusDirection::Left | FocusDirection::Up) => self.runtime.focus_prev(),
        }
        Ok(())
    }

    pub fn add_panel(&mut self) -> Result<(), JsValue> {
        if !self.is_dynamic() {
            return Err(EngineError::NotDynamic.into());
        }
        let name: Arc<str> = format!("panel-{}", self.next_panel_id).into();
        self.next_panel_id += 1;
        self.panels.push(Arc::clone(&name));
        self.runtime.add_panel(name).map_err(EngineError::from)?;
        Ok(())
    }

    pub fn remove_panel(&mut self) -> Result<(), JsValue> {
        if !self.is_dynamic() {
            return Err(EngineError::NotDynamic.into());
        }
        let (pid, kind) = match self.runtime.focused().and_then(|pid| {
            let kind = self.runtime.focused_kind()?.to_owned();
            Some((pid, kind))
        }) {
            Some(pair) => pair,
            None => return Ok(()),
        };
        let _ = self.runtime.remove_panel(pid).map_err(EngineError::from)?;
        self.panels.retain(|p| p.as_ref() != kind.as_str());
        Ok(())
    }

    pub fn swap_next(&mut self) {
        self.runtime.swap_next();
    }

    pub fn swap_prev(&mut self) {
        self.runtime.swap_prev();
    }

    pub fn resize_focused(&mut self, delta: f32) {
        let Some(pid) = self.runtime.focused() else {
            return;
        };
        let _ = self.runtime.resize_boundary(pid, delta);
    }

    pub fn is_dynamic(&self) -> bool {
        self.presets[self.preset_idx].input == PanelInputKind::DynamicList
    }

    pub fn preset_name(&self) -> String {
        self.presets[self.preset_idx].name.to_string()
    }

    pub fn focused_kind(&self) -> Option<String> {
        self.runtime.focused_kind().map(str::to_string)
    }

    pub fn panel_count(&self) -> usize {
        self.panels.len()
    }
}

/// Returns JSON: [{name, input, description}]
#[wasm_bindgen(js_name = "layoutPresets")]
pub fn layout_presets() -> String {
    let presets: Vec<PresetDesc> = Layout::presets()
        .iter()
        .map(|p| PresetDesc {
            name: p.name,
            input: match p.input {
                PanelInputKind::DynamicList => "dynamic",
                PanelInputKind::FixedSlots => "fixed",
            },
            description: p.description,
        })
        .collect();
    serde_json::to_string(&presets).unwrap_or_default()
}
