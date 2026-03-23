pub const ANIM_DURATION_SECS: f32 = 0.25;

pub fn ease_out(t: f32) -> f32 {
    let inv = 1.0 - t;
    1.0 - inv * inv * inv
}
