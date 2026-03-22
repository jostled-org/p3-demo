use demo_presets::{ANIM_DURATION_SECS, ease_out};

#[test]
fn ease_out_at_zero_is_zero() {
    assert!((ease_out(0.0) - 0.0).abs() < f32::EPSILON);
}

#[test]
fn ease_out_at_one_is_one() {
    assert!((ease_out(1.0) - 1.0).abs() < f32::EPSILON);
}

#[test]
fn ease_out_is_monotonic() {
    let samples: Vec<f32> = (0..=100).map(|i| ease_out(i as f32 / 100.0)).collect();
    for pair in samples.windows(2) {
        assert!(
            pair[1] >= pair[0],
            "ease_out must be monotonically increasing"
        );
    }
}

#[test]
fn anim_duration_is_positive() {
    assert!(ANIM_DURATION_SECS > 0.0);
}
