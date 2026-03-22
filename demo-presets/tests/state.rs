use demo_presets::{DemoError, DemoState};

#[test]
fn construction_succeeds() {
    let state = DemoState::new(1.0).unwrap();
    assert!(state.preset_count() > 0);
    assert!(state.theme_count() > 0);
    assert_eq!(state.preset_idx(), 0);
    assert_eq!(state.theme_idx(), 0);
}

#[test]
fn preset_cycling_wraps_forward() {
    let mut state = DemoState::new(1.0).unwrap();
    let count = state.preset_count();
    for _ in 0..count {
        state.next_preset();
    }
    assert_eq!(state.preset_idx(), 0);
}

#[test]
fn preset_cycling_wraps_backward() {
    let mut state = DemoState::new(1.0).unwrap();
    state.prev_preset();
    assert_eq!(state.preset_idx(), state.preset_count() - 1);
}

#[test]
fn theme_cycling_wraps_forward() {
    let mut state = DemoState::new(1.0).unwrap();
    let count = state.theme_count();
    for _ in 0..count {
        state.next_theme();
    }
    assert_eq!(state.theme_idx(), 0);
}

#[test]
fn theme_cycling_wraps_backward() {
    let mut state = DemoState::new(1.0).unwrap();
    state.prev_theme();
    assert_eq!(state.theme_idx(), state.theme_count() - 1);
}

#[test]
fn add_panel_on_dynamic_preset() {
    let mut state = DemoState::new(1.0).unwrap();
    // Find a dynamic preset
    let dynamic_idx = state
        .presets()
        .iter()
        .position(|p| p.input == panes::PanelInputKind::DynamicList)
        .unwrap();
    for _ in 0..dynamic_idx {
        state.next_preset();
    }
    assert!(state.is_dynamic());

    let before = state.panel_count();
    state.add_panel().unwrap();
    assert_eq!(state.panel_count(), before + 1);
}

#[test]
fn add_panel_on_fixed_preset_returns_not_dynamic() {
    let mut state = DemoState::new(1.0).unwrap();
    // Find a fixed preset
    let fixed_idx = state
        .presets()
        .iter()
        .position(|p| p.input == panes::PanelInputKind::FixedSlots)
        .unwrap();
    for _ in 0..fixed_idx {
        state.next_preset();
    }
    assert!(!state.is_dynamic());

    let result = state.add_panel();
    assert!(matches!(result, Err(DemoError::NotDynamic)));
}

#[test]
fn resolve_produces_frame() {
    let mut state = DemoState::new(1.0).unwrap();
    let frame = state.resolve(100.0, 50.0).unwrap();
    let layout = frame.layout();
    // Should have at least one panel
    assert!(layout.panels().count() > 0);
}

#[test]
fn load_current_palette_succeeds() {
    let state = DemoState::new(1.0).unwrap();
    let palette = state.load_current_palette().unwrap();
    // Base background should be populated
    assert!(palette.base.background.is_some());
}

#[test]
fn switch_preset_by_name() {
    let mut state = DemoState::new(1.0).unwrap();
    state.switch_preset("monocle").unwrap();
    assert_eq!(state.preset_name(), "monocle");
}

#[test]
fn switch_preset_unknown_returns_error() {
    let mut state = DemoState::new(1.0).unwrap();
    let result = state.switch_preset("nonexistent");
    assert!(matches!(result, Err(DemoError::UnknownPreset(_))));
}
