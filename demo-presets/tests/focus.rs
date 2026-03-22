use demo_presets::{Action, DemoState};

#[test]
fn focus_at_selects_panel_under_point() {
    let mut state = DemoState::new(1.0).unwrap();
    state.switch_preset("master-stack").unwrap();

    let frame = state.resolve(100.0, 50.0).unwrap();
    let layout = frame.layout();

    // Collect panels so we can target the second one
    let panels: Vec<_> = layout.panels().collect();
    assert!(panels.len() >= 2, "need at least 2 panels");

    let second = &panels[1];
    let target_pid = second.id;
    let center_x = second.rect.x + second.rect.w / 2.0;
    let center_y = second.rect.y + second.rect.h / 2.0;

    let changed = state.focus_at(center_x, center_y);
    assert!(changed, "focus_at should return true when a panel is hit");
    assert_eq!(
        state.focused_pid(),
        Some(target_pid),
        "focused panel should be the one under the click point"
    );
}

#[test]
fn focus_at_returns_false_for_empty_space() {
    let mut state = DemoState::new(1.0).unwrap();
    state.switch_preset("master-stack").unwrap();

    let _ = state.resolve(100.0, 50.0).unwrap();

    let before = state.focused_pid();

    // Point far outside any panel
    let changed = state.focus_at(-100.0, -100.0);
    assert!(!changed, "focus_at should return false for empty space");
    assert_eq!(
        state.focused_pid(),
        before,
        "focused panel should be unchanged"
    );
}

#[test]
fn action_focus_at_dispatches() {
    let mut state = DemoState::new(1.0).unwrap();
    state.switch_preset("master-stack").unwrap();

    let frame = state.resolve(100.0, 50.0).unwrap();
    let layout = frame.layout();

    let panels: Vec<_> = layout.panels().collect();
    assert!(panels.len() >= 2, "need at least 2 panels");

    let second = &panels[1];
    let center_x = second.rect.x + second.rect.w / 2.0;
    let center_y = second.rect.y + second.rect.h / 2.0;
    let target_pid = second.id;

    let changed = state.apply(Action::FocusAt(center_x, center_y));
    assert!(changed, "FocusAt action should report layout changed");
    assert_eq!(
        state.focused_pid(),
        Some(target_pid),
        "focused panel should match the clicked panel"
    );
}
