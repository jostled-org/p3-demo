use demo_presets::{Action, DemoState, build_chrome, build_css_dashboard, build_default};

#[test]
fn build_chrome_returns_some() {
    assert!(build_chrome().is_some());
}

#[test]
fn build_css_dashboard_returns_ok() {
    assert!(build_css_dashboard().is_ok());
}

#[test]
fn build_default_returns_ok() {
    let panels = vec!["a".into(), "b".into(), "c".into()];
    let rt = build_default(&panels, 1.0).unwrap();
    assert_eq!(rt.sequence().len(), 3);
}

#[test]
fn default_layout_add_and_remove() {
    let mut state = DemoState::new(1.0).unwrap();
    state.switch_preset("default").unwrap();
    assert!(state.is_dynamic());
    assert_eq!(state.preset_name(), "default");

    let before = state.panel_count();
    state.add_panel().unwrap();
    assert_eq!(state.panel_count(), before + 1);

    state.remove_panel().unwrap();
    assert_eq!(state.panel_count(), before);
}

#[test]
fn default_layout_resize_works() {
    let mut state = DemoState::new(1.0).unwrap();
    state.switch_preset("default").unwrap();
    state.resolve(100.0, 50.0).unwrap();
    // resize_boundary should succeed on flat sibling panels
    state.resize_horizontal(0.05);
    state.resolve(100.0, 50.0).unwrap();
}

#[test]
fn help_overlay_toggles() {
    let mut state = DemoState::new(1.0).unwrap();
    assert!(!state.help_visible());
    state.toggle_help();
    assert!(state.help_visible());

    // Overlay appears in resolved layout
    let frame = state.resolve(100.0, 50.0).unwrap();
    let overlays: Vec<_> = frame.layout().overlays().collect();
    assert_eq!(overlays.len(), 1);
    assert_eq!(overlays[0].kind, "help");

    state.toggle_help();
    assert!(!state.help_visible());

    // Overlay hidden after toggle off
    let frame = state.resolve(100.0, 50.0).unwrap();
    let overlays: Vec<_> = frame.layout().overlays().collect();
    assert!(overlays.is_empty());
}

#[test]
fn action_apply_next_preset_changes_layout() {
    let mut state = DemoState::new(1.0).unwrap();
    let before = state.preset_idx();
    let changed = state.apply(Action::NextPreset);
    assert!(changed);
    assert_ne!(state.preset_idx(), before);
}

#[test]
fn action_apply_theme_does_not_change_layout() {
    let mut state = DemoState::new(1.0).unwrap();
    let before = state.theme_idx();
    let changed = state.apply(Action::NextTheme);
    assert!(!changed);
    assert_ne!(state.theme_idx(), before);
}

#[test]
fn action_changes_layout_reflects_variant() {
    assert!(Action::NextPreset.changes_layout());
    assert!(Action::AddPanel.changes_layout());
    assert!(Action::ToggleHelp.changes_layout());
    assert!(!Action::NextTheme.changes_layout());
    assert!(!Action::PrevTheme.changes_layout());
}

#[test]
fn help_overlay_survives_preset_switch() {
    let mut state = DemoState::new(1.0).unwrap();
    state.toggle_help();
    state.next_preset();

    let frame = state.resolve(100.0, 50.0).unwrap();
    let overlays: Vec<_> = frame.layout().overlays().collect();
    assert_eq!(overlays.len(), 1);
    assert_eq!(overlays[0].kind, "help");
}
