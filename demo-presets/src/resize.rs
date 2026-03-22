use panes::{Constraints, Node, NodeId, PaneError, PanelId};

/// Check whether a tree node is a vertical container (Col or column-direction
/// TaffyPassthrough).
fn is_vertical_container(node: &Node) -> bool {
    match node {
        Node::Col { .. } => true,
        Node::TaffyPassthrough { style, .. } => matches!(
            style.flex_direction,
            taffy::FlexDirection::Column | taffy::FlexDirection::ColumnReverse
        ),
        _ => false,
    }
}

/// Walk up from `pid` to find the nearest vertical ancestor container.
///
/// Returns `(container_nid, target_child)` where `target_child` is the direct
/// child of the container that is (or contains) the panel.
pub(crate) fn find_vertical_ancestor(
    tree: &panes::LayoutTree,
    pid: PanelId,
) -> Option<(NodeId, NodeId)> {
    let start = tree.node_for_panel(pid)?;
    let mut child = start;
    let mut current = tree.parent(start).ok()??;
    loop {
        let node = tree.node(current)?;
        if is_vertical_container(node) {
            return Some((current, child));
        }
        child = current;
        current = tree.parent(current).ok()??;
    }
}

/// Redistribute grow weights among panel children of `container`, giving
/// `delta` more share to `target_child`.
///
/// Only handles containers whose direct children are all `Node::Panel`.
/// Returns `Err` if any child is a container (caller should silently ignore).
pub(crate) fn redistribute_panel_grow(
    tree: &mut panes::LayoutTree,
    container: NodeId,
    target_child: NodeId,
    delta: f32,
) -> Result<(), PaneError> {
    const EPSILON: f32 = 0.001;

    let siblings: Vec<(NodeId, PanelId, f32)> = tree
        .children(container)?
        .iter()
        .copied()
        .map(|nid| {
            let node = tree.node(nid).ok_or(PaneError::NodeNotFound(nid))?;
            match node {
                Node::Panel {
                    id, constraints, ..
                } => Ok((nid, *id, constraints.grow.unwrap_or(1.0))),
                _ => Err(PaneError::NodeNotFound(nid)),
            }
        })
        .collect::<Result<Vec<_>, _>>()?;

    let total_grow: f32 = siblings.iter().map(|&(_, _, g)| g).sum();
    let target = siblings.iter().find(|&&(nid, _, _)| nid == target_child);
    let Some(&(_, _, target_grow)) = target else {
        return Ok(());
    };

    let current_share = target_grow / total_grow;
    let max_share = 1.0 - EPSILON * (siblings.len() - 1) as f32;
    if current_share >= max_share {
        return Ok(());
    }

    let new_share = (current_share + delta).clamp(EPSILON, max_share);
    let scale = (1.0 - new_share) / (1.0 - current_share);

    for &(nid, pid, grow) in &siblings {
        let new_grow = match nid == target_child {
            true => new_share * total_grow,
            false => (grow * scale).max(EPSILON),
        };
        let c = tree.panel_constraints(pid)?;
        tree.set_constraints(
            pid,
            Constraints {
                grow: Some(new_grow),
                min: c.min,
                max: c.max,
                ..Constraints::default()
            },
        )?;
    }

    Ok(())
}
