use demo_presets::DemoState;

#[test]
fn add_panel_adjacent_splits_focused() {
    let mut state = DemoState::new(1.0).unwrap();
    state.switch_preset("default").unwrap();

    let frame1 = state.resolve(100.0, 50.0).unwrap();
    let count_before = frame1.layout().panels().count();
    let focused_before = state.focused_pid().expect("should have focused panel");

    state.add_panel().unwrap();

    let frame2 = state.resolve(100.0, 50.0).unwrap();
    let panels_after: Vec<_> = frame2.layout().panels().collect();
    let count_after = panels_after.len();

    assert_eq!(
        count_after,
        count_before + 1,
        "panel count should increase by 1"
    );

    // The new panel should be adjacent to the previously focused panel.
    // Find the focused panel's position and the new panel's position in the list.
    let focused_pos = panels_after
        .iter()
        .position(|p| p.id == focused_before)
        .expect("original focused panel should still exist");

    // The new panel should be the one right after or sharing a boundary with the focused panel.
    let focused_rect = &panels_after[focused_pos].rect;
    let new_panel = panels_after.iter().find(|p| {
        p.id != focused_before && {
            // Adjacent means sharing an edge (right/bottom boundary matches left/top of new panel)
            let shares_right = (focused_rect.x + focused_rect.w - p.rect.x).abs() < 2.0;
            let shares_bottom = (focused_rect.y + focused_rect.h - p.rect.y).abs() < 2.0;
            shares_right || shares_bottom
        }
    });
    assert!(
        new_panel.is_some(),
        "new panel should be adjacent to the previously focused panel"
    );
}

#[test]
fn add_panel_adjacent_respects_aspect_ratio() {
    let mut state = DemoState::new(1.0).unwrap();
    state.switch_preset("default").unwrap();

    // Wide viewport — focused panel will be wider than tall → horizontal split
    let frame1 = state.resolve(200.0, 50.0).unwrap();
    let focused_pid = state.focused_pid().expect("should have focused panel");
    let focused_width_before = frame1
        .layout()
        .panels()
        .find(|p| p.id == focused_pid)
        .expect("focused panel should exist")
        .rect
        .w;

    state.add_panel().unwrap();

    let frame2 = state.resolve(200.0, 50.0).unwrap();
    let focused_width_after = frame2
        .layout()
        .panels()
        .find(|p| p.id == focused_pid)
        .expect("focused panel should still exist")
        .rect
        .w;

    assert!(
        focused_width_after < focused_width_before,
        "focused panel should be narrower after horizontal split: before={focused_width_before}, after={focused_width_after}"
    );
}

#[test]
fn remove_panel_after_adjacent_add() {
    let mut state = DemoState::new(1.0).unwrap();
    state.switch_preset("default").unwrap();

    let frame1 = state.resolve(100.0, 50.0).unwrap();
    let count_before = frame1.layout().panels().count();

    // Add then remove — should return to original count
    state.add_panel().unwrap();
    let frame2 = state.resolve(100.0, 50.0).unwrap();
    assert_eq!(frame2.layout().panels().count(), count_before + 1);

    state.remove_panel().unwrap();
    let frame3 = state.resolve(100.0, 50.0).unwrap();
    assert_eq!(
        frame3.layout().panels().count(),
        count_before,
        "panel count should return to original after remove"
    );
}
