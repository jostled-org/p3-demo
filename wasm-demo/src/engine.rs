use demo_presets::{DemoState, HELP_BINDINGS_GUI};
use panes::{Layout, PanelInputKind};
use panes_wasm::WasmRect;
use serde_json::json;
use wasm_bindgen::prelude::*;

use crate::catalog::PresetDesc;
use crate::diff::DiffCounts;
use crate::overlay::OverlayRect;
use crate::types::{BaseRect, PanelRect};

fn to_js_err(e: impl std::fmt::Display) -> JsValue {
    JsValue::from_str(&e.to_string())
}

fn base_rect(id: u32, kind: &str, rect: WasmRect) -> BaseRect {
    BaseRect {
        id,
        kind: kind.into(),
        x: rect.x,
        y: rect.y,
        w: rect.w,
        h: rect.h,
    }
}

#[wasm_bindgen]
pub struct JsLayoutEngine {
    state: DemoState,
}

#[wasm_bindgen]
impl JsLayoutEngine {
    #[wasm_bindgen(constructor)]
    pub fn new(preset_name: &str, cell: f32) -> Result<JsLayoutEngine, JsValue> {
        let mut state = DemoState::new(cell).map_err(to_js_err)?;
        state.set_help_binding_count(HELP_BINDINGS_GUI.len());
        state.switch_preset(preset_name).map_err(to_js_err)?;
        Ok(Self { state })
    }

    /// Returns JSON: [{id, kind, x, y, w, h, focused, kind_index}]
    pub fn resolve(&mut self, width: f32, height: f32) -> Result<String, JsValue> {
        let frame = self.state.resolve(width, height).map_err(to_js_err)?;
        let layout = frame.layout();
        let focused = self.state.focused_pid();

        let rects: Vec<PanelRect> = panes_wasm::panels(layout)
            .map(|entry| PanelRect {
                base: base_rect(entry.id.raw(), entry.kind, entry.rect),
                focused: focused == Some(entry.id),
                kind_index: entry.kind_index,
            })
            .collect();

        serde_json::to_string(&rects).map_err(to_js_err)
    }

    /// Returns JSON: {added, removed, resized, moved, unchanged}
    pub fn diff_counts(&self) -> String {
        let diff = self.state.last_diff();
        let counts = DiffCounts {
            added: diff.added.len(),
            removed: diff.removed.len(),
            resized: diff.resized.len(),
            moved: diff.moved.len(),
            unchanged: diff.unchanged.len(),
        };
        // Fallback: valid empty JSON object if serialization fails (no web_sys console access)
        serde_json::to_string(&counts).unwrap_or_else(|_| "{}".to_string())
    }

    pub fn switch_preset(&mut self, name: &str) -> Result<(), JsValue> {
        self.state.switch_preset(name).map_err(to_js_err)
    }

    pub fn focus_next(&mut self) {
        self.state.focus_next();
    }

    pub fn focus_prev(&mut self) {
        self.state.focus_prev();
    }

    #[wasm_bindgen(js_name = "focusAt")]
    pub fn focus_at(&mut self, viewport_x: f32, viewport_y: f32) -> bool {
        self.state.focus_at(viewport_x, viewport_y)
    }

    pub fn focus_direction(&mut self, dir: &str) -> Result<(), JsValue> {
        self.state.focus_direction_str(dir).map_err(to_js_err)
    }

    pub fn add_panel(&mut self) -> Result<(), JsValue> {
        self.state.add_panel().map_err(to_js_err)
    }

    pub fn remove_panel(&mut self) -> Result<(), JsValue> {
        self.state.remove_panel().map_err(to_js_err)
    }

    #[wasm_bindgen(js_name = "scrollBy")]
    pub fn scroll_by(&mut self, delta: f32) {
        self.state.scroll_by(delta);
    }

    pub fn swap_next(&mut self) {
        self.state.swap_next();
    }

    pub fn swap_prev(&mut self) {
        self.state.swap_prev();
    }

    #[wasm_bindgen(js_name = "resizeHorizontal")]
    pub fn resize_horizontal(&mut self, delta: f32) {
        self.state.resize_horizontal(delta);
    }

    #[wasm_bindgen(js_name = "resizeVertical")]
    pub fn resize_vertical(&mut self, delta: f32) {
        self.state.resize_vertical(delta);
    }

    pub fn toggle_collapsed(&mut self) -> Result<(), JsValue> {
        self.state.toggle_collapsed().map_err(to_js_err)
    }

    /// Returns JSON: [{id, kind, x, y, w, h}]
    ///
    /// Uses the layout from the last `resolve()` call. Call `resolve()` first.
    #[wasm_bindgen(js_name = "resolveOverlays")]
    pub fn resolve_overlays(&self) -> Result<String, JsValue> {
        let frame = self.state.last_frame().ok_or_else(|| {
            JsValue::from_str("resolve() must be called before resolveOverlays()")
        })?;
        let layout = frame.layout();

        let rects: Vec<OverlayRect> = panes_wasm::overlays(layout)
            .map(|entry| OverlayRect {
                base: base_rect(entry.id.raw(), entry.kind, entry.rect),
            })
            .collect();

        serde_json::to_string(&rects).map_err(to_js_err)
    }

    /// Returns a JSON snapshot of the current layout state for localStorage.
    pub fn snapshot(&self) -> Result<String, JsValue> {
        let snap = self
            .state
            .snapshot()
            .ok_or_else(|| JsValue::from_str("no runtime available for snapshot"))?;
        serde_json::to_string(&snap).map_err(to_js_err)
    }

    /// Restores layout state from a JSON snapshot string.
    pub fn restore(&mut self, json: &str) -> Result<(), JsValue> {
        let snap = serde_json::from_str(json).map_err(to_js_err)?;
        self.state.restore(snap).map_err(to_js_err)
    }

    #[wasm_bindgen(js_name = "toggleHelp")]
    pub fn toggle_help(&mut self) {
        self.state.toggle_help();
    }

    #[wasm_bindgen(js_name = "helpVisible")]
    pub fn help_visible(&self) -> bool {
        self.state.help_visible()
    }

    pub fn is_dynamic(&self) -> bool {
        self.state.is_dynamic()
    }

    pub fn preset_name(&self) -> String {
        self.state.preset_name().to_string()
    }

    pub fn focused_kind(&self) -> Option<String> {
        self.state.focused_kind().map(str::to_string)
    }

    pub fn panel_count(&self) -> usize {
        self.state.panel_count()
    }

    /// Returns JSON: [{id, name, style}]
    #[wasm_bindgen(js_name = "themeList")]
    pub fn theme_list(&self) -> String {
        let themes: Vec<serde_json::Value> = self
            .state
            .themes()
            .iter()
            .map(|t| {
                json!({
                    "id": t.id.as_ref(),
                    "name": t.name.as_ref(),
                    "style": t.style.as_ref(),
                })
            })
            .collect();
        // Fallback: valid empty JSON array if serialization fails (no web_sys console access)
        serde_json::to_string(&themes).unwrap_or_else(|_| "[]".to_string())
    }

    /// Returns a CSS block (`:root { --bg: ...; ... }`) for the given theme ID.
    #[wasm_bindgen(js_name = "themeCss")]
    pub fn theme_css(&self, id: &str) -> Result<String, JsValue> {
        let palette = self.state.load_palette(id).map_err(to_js_err)?;
        Ok(palette.to_css())
    }
}

/// Returns JSON: [{key, action}]
#[wasm_bindgen(js_name = "helpBindings")]
pub fn help_bindings() -> String {
    let bindings: Vec<serde_json::Value> = HELP_BINDINGS_GUI
        .iter()
        .map(|b| json!({ "key": b.key, "action": b.action }))
        .collect();
    // Fallback: valid empty JSON array if serialization fails (no web_sys console access)
    serde_json::to_string(&bindings).unwrap_or_else(|_| "[]".to_string())
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
    // Fallback: valid empty JSON array if serialization fails (no web_sys console access)
    serde_json::to_string(&presets).unwrap_or_else(|_| "[]".to_string())
}
