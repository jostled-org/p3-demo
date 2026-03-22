use demo_presets::{Action, DemoState};

#[test]
fn scroll_by_offsets_panels() {
    let mut state = DemoState::new(1.0).unwrap();
    state.switch_preset("scrollable").unwrap();

    // Resolve to establish initial positions
    let frame1 = state.resolve(100.0, 50.0).unwrap();
    let first_x_before = frame1
        .layout()
        .panels()
        .next()
        .expect("at least one panel")
        .rect
        .x;

    // Scroll forward
    state.scroll_by(10.0);

    let frame2 = state.resolve(100.0, 50.0).unwrap();
    let first_x_after = frame2
        .layout()
        .panels()
        .next()
        .expect("at least one panel")
        .rect
        .x;

    // Scrolling shifts panels in the scroll direction
    assert!(
        (first_x_after - first_x_before).abs() > f32::EPSILON,
        "expected x to change after scroll_by(10.0): before={first_x_before}, after={first_x_after}"
    );
}

#[test]
fn scroll_by_noop_on_non_scrollable() {
    let mut state = DemoState::new(1.0).unwrap();
    state.switch_preset("master-stack").unwrap();

    let frame1 = state.resolve(100.0, 50.0).unwrap();
    let positions_before: Vec<(f32, f32)> = frame1
        .layout()
        .panels()
        .map(|e| (e.rect.x, e.rect.y))
        .collect();

    // scroll_by on a non-scrollable preset should be a no-op
    state.scroll_by(10.0);

    let frame2 = state.resolve(100.0, 50.0).unwrap();
    let positions_after: Vec<(f32, f32)> = frame2
        .layout()
        .panels()
        .map(|e| (e.rect.x, e.rect.y))
        .collect();

    assert_eq!(positions_before, positions_after);
}

#[test]
fn action_scroll_by_dispatches() {
    let mut state = DemoState::new(1.0).unwrap();
    state.switch_preset("scrollable").unwrap();

    // Resolve to establish initial state
    let _ = state.resolve(100.0, 50.0).unwrap();

    let changed = state.apply(Action::ScrollBy(10.0));
    assert!(changed, "ScrollBy should report layout changed");
}
