use panes::diff::LayoutDiff;

pub fn format_diff_counts(diff: &LayoutDiff<'_>) -> Box<str> {
    format!(
        "+{} -{} ~{} ={} >{}",
        diff.added.len(),
        diff.removed.len(),
        diff.resized.len(),
        diff.unchanged.len(),
        diff.moved.len(),
    )
    .into_boxed_str()
}
