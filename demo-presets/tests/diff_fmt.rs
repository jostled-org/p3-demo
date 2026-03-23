use demo_presets::format_diff_counts;
use panes::diff::LayoutDiff;

#[test]
fn empty_diff_formats_all_zeros() {
    let diff = LayoutDiff {
        added: &[],
        removed: &[],
        moved: &[],
        resized: &[],
        unchanged: &[],
    };
    assert_eq!(&*format_diff_counts(&diff), "+0 -0 ~0 =0 >0");
}
