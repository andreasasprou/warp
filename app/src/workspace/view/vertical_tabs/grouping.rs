//! Pure helper that projects the visible-tabs list into ungrouped rows and
//! per-group sections.
//!
//! This is the seam between `Workspace.tabs` (the model order) and the
//! grouped sidebar render. It is intentionally **pure**: no `AppContext`,
//! no rendering, so it can be unit-tested without a Warp app instance.
//!
//! The input is the same `Vec<(usize, Option<Vec<PaneId>>)>` that
//! [`render_groups`](super::render_groups) already builds for panes-mode
//! search filtering, so search continues to work transparently inside groups.
//! See `app/src/workspace/view/vertical_tabs.rs:1517` for how `visible_tabs`
//! is constructed and why each entry carries an optional set of matching
//! pane ids.

use crate::pane_group::PaneId;
use crate::tab::TabData;
use crate::workspace::tab_settings::{TabGroupId, TabGroups};

/// One row of the visible-tabs list. Mirrors the existing tuple in
/// [`render_groups`](super::render_groups): an index into `Workspace.tabs`
/// plus, when search is active, the set of pane ids that matched.
pub type VisibleTab = (usize, Option<Vec<PaneId>>);

/// One rendered item in the grouped tabs list.
///
/// A group section is emitted at the position of its first visible tab in
/// `Workspace.tabs`; later tabs in the same group are gathered into that
/// section and skipped as standalone top-level items. This preserves a single
/// source of truth for ordering: moving tabs in `Workspace.tabs` changes where
/// group sections sit relative to ungrouped tabs.
#[derive(Debug, Clone, PartialEq)]
pub enum GroupedTabListItem {
    Ungrouped(VisibleTab),
    Group {
        id: TabGroupId,
        tabs: Vec<VisibleTab>,
    },
}

/// Output of [`partition_visible_tabs`].
///
/// Indices remain indices into the original `Workspace.tabs` slice. Adjacent
/// entries inside a group's `Vec<VisibleTab>` are not necessarily adjacent in
/// `Workspace.tabs`; the rendered section is a projection of all visible tabs
/// that resolve to that group.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct GroupedTabs {
    /// Tabs not assigned to any group, in their original `Workspace.tabs`
    /// order.
    pub ungrouped: Vec<VisibleTab>,
    /// Tabs assigned to each group, in the order groups appear in
    /// `TabGroups`. Empty groups are included so newly-created groups remain visible.
    pub groups: Vec<(TabGroupId, Vec<VisibleTab>)>,
    /// Top-level render order. Ungrouped tabs and group sections are
    /// interleaved according to `Workspace.tabs`; empty groups are appended in
    /// `TabGroups` order.
    pub items: Vec<GroupedTabListItem>,
}

/// Partition `visible_tabs` into ungrouped + per-group buckets.
///
/// `resolve_group` is a side-injected callback that decides each tab's group.
/// The caller is responsible for resolving manual per-tab overrides and
/// directory assignments into a single answer. Keeping the helper agnostic to
/// settings lookup keeps both rendering and tests straightforward.
///
/// Group ids returned by `resolve_group` that are not present in `tab_groups`
/// are treated as ungrouped (defensive against stale references
/// left over after a group is deleted).
pub fn partition_visible_tabs<F>(
    visible_tabs: Vec<VisibleTab>,
    workspace_tabs: &[TabData],
    tab_groups: &TabGroups,
    mut resolve_group: F,
) -> GroupedTabs
where
    F: FnMut(&TabData) -> Option<TabGroupId>,
{
    let visible_tabs: Vec<_> = visible_tabs
        .into_iter()
        .filter(|(tab_index, _)| workspace_tabs.get(*tab_index).is_some())
        .collect();

    partition_visible_tabs_by_index(visible_tabs, tab_groups, |tab_index| {
        resolve_group(&workspace_tabs[tab_index])
    })
}

fn partition_visible_tabs_by_index<F>(
    visible_tabs: Vec<VisibleTab>,
    tab_groups: &TabGroups,
    mut resolve_group: F,
) -> GroupedTabs
where
    F: FnMut(usize) -> Option<TabGroupId>,
{
    let mut ungrouped: Vec<VisibleTab> = Vec::new();
    let mut buckets: std::collections::HashMap<TabGroupId, Vec<VisibleTab>> =
        std::collections::HashMap::new();
    let mut entries: Vec<(VisibleTab, Option<TabGroupId>)> = Vec::new();

    for entry in visible_tabs {
        let (tab_index, _) = &entry;
        let group_id = resolve_group(*tab_index).filter(|id| tab_groups.contains(id));

        match &group_id {
            Some(id) => buckets.entry(id.clone()).or_default().push(entry.clone()),
            None => ungrouped.push(entry.clone()),
        }
        entries.push((entry, group_id));
    }

    let groups: Vec<(TabGroupId, Vec<VisibleTab>)> = tab_groups
        .iter()
        .map(|g| {
            (
                g.id.clone(),
                buckets.get(&g.id).cloned().unwrap_or_default(),
            )
        })
        .collect();

    let mut rendered_groups: std::collections::HashSet<TabGroupId> =
        std::collections::HashSet::new();
    let mut items = Vec::new();
    for (entry, group_id) in entries {
        match group_id {
            Some(id) => {
                if rendered_groups.insert(id.clone()) {
                    items.push(GroupedTabListItem::Group {
                        id: id.clone(),
                        tabs: buckets.get(&id).cloned().unwrap_or_default(),
                    });
                }
            }
            None => items.push(GroupedTabListItem::Ungrouped(entry)),
        }
    }

    // Empty groups are appended so a freshly-created group with no assignment
    // remains visible; otherwise users think the create action did nothing.
    for group in tab_groups.iter() {
        if rendered_groups.insert(group.id.clone()) {
            items.push(GroupedTabListItem::Group {
                id: group.id.clone(),
                tabs: Vec::new(),
            });
        }
    }

    GroupedTabs {
        ungrouped,
        groups,
        items,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pane_group::pane::TerminalPaneId;
    use crate::workspace::tab_settings::{TabGroup, TabGroupAssignment, TabGroupAssignments};
    use std::collections::HashMap;
    use std::path::PathBuf;

    /// Test-only variant that bypasses TabData entirely by routing through a
    /// closure keyed on tab index. `TabData` is not constructible outside a
    /// full Warp app context, so the public helper is exercised through this
    /// inline reimplementation that mirrors its logic exactly.
    ///
    /// Mirrors the directory-resolution logic the caller uses in production.
    fn partition_for_test(
        visible_tabs: Vec<VisibleTab>,
        tab_groups: &TabGroups,
        assignments: &TabGroupAssignments,
        index_to_dir: &HashMap<usize, PathBuf>,
        index_to_override: &HashMap<usize, TabGroupId>,
    ) -> GroupedTabs {
        partition_visible_tabs_by_index(visible_tabs, tab_groups, |tab_index| {
            index_to_override.get(&tab_index).cloned().or_else(|| {
                index_to_dir
                    .get(&tab_index)
                    .map(|d| d.as_path())
                    .and_then(|d| assignments.group_for_directory(d))
            })
        })
    }

    fn gid(s: &str) -> TabGroupId {
        TabGroupId(s.to_string())
    }

    fn group(id: &str, name: &str) -> TabGroup {
        TabGroup {
            id: gid(id),
            name: name.into(),
            color: None,
            collapsed: false,
        }
    }

    fn assignments(entries: &[(&str, TabGroupAssignment)]) -> TabGroupAssignments {
        let map = entries
            .iter()
            .map(|(p, a)| ((*p).to_string(), a.clone()))
            .collect();
        TabGroupAssignments(map)
    }

    fn dirs(entries: &[(usize, &str)]) -> HashMap<usize, PathBuf> {
        entries
            .iter()
            .map(|(i, p)| (*i, PathBuf::from(*p)))
            .collect()
    }

    #[test]
    fn empty_visible_tabs_yields_empty_partition() {
        let g = partition_for_test(
            vec![],
            &TabGroups::default(),
            &TabGroupAssignments::default(),
            &HashMap::new(),
            &HashMap::new(),
        );
        assert!(g.ungrouped.is_empty());
        assert!(g.groups.is_empty());
    }

    #[test]
    fn no_groups_means_everything_ungrouped() {
        let groups = TabGroups::default();
        let assigns = TabGroupAssignments::default();
        let dirs = dirs(&[(0, "/x/a"), (1, "/x/b")]);
        let g = partition_for_test(
            vec![(0, None), (1, None)],
            &groups,
            &assigns,
            &dirs,
            &HashMap::new(),
        );
        assert_eq!(g.ungrouped, vec![(0, None), (1, None)]);
        assert!(g.groups.is_empty());
    }

    #[test]
    fn tabs_partition_into_groups_in_tab_groups_order() {
        let groups = TabGroups(vec![group("intavia", "Intavia"), group("nova", "Nova")]);
        let assigns = assignments(&[
            (
                "/projects/intavia",
                TabGroupAssignment::InGroup(gid("intavia")),
            ),
            ("/projects/nova", TabGroupAssignment::InGroup(gid("nova"))),
        ]);
        let dirs = dirs(&[
            (0, "/projects/intavia/crates/app"),
            (1, "/projects/nova"),
            (2, "/projects/intavia/crates/cli"),
            (3, "/scratch"),
        ]);

        let g = partition_for_test(
            vec![(0, None), (1, None), (2, None), (3, None)],
            &groups,
            &assigns,
            &dirs,
            &HashMap::new(),
        );

        // Ungrouped preserves model order.
        assert_eq!(g.ungrouped, vec![(3, None)]);

        // Groups appear in TabGroups order, not insertion order from
        // assignments. Tabs within a group preserve their visible_tabs order.
        assert_eq!(
            g.groups,
            vec![
                (gid("intavia"), vec![(0, None), (2, None)]),
                (gid("nova"), vec![(1, None)]),
            ]
        );
        assert_eq!(
            g.items,
            vec![
                GroupedTabListItem::Group {
                    id: gid("intavia"),
                    tabs: vec![(0, None), (2, None)],
                },
                GroupedTabListItem::Group {
                    id: gid("nova"),
                    tabs: vec![(1, None)],
                },
                GroupedTabListItem::Ungrouped((3, None)),
            ]
        );
    }

    #[test]
    fn top_level_items_interleave_ungrouped_tabs_and_group_sections() {
        let groups = TabGroups(vec![group("intavia", "Intavia"), group("nova", "Nova")]);
        let assigns = assignments(&[
            (
                "/projects/intavia",
                TabGroupAssignment::InGroup(gid("intavia")),
            ),
            ("/projects/nova", TabGroupAssignment::InGroup(gid("nova"))),
        ]);
        let dirs = dirs(&[
            (0, "/scratch/one"),
            (1, "/projects/intavia/crates/app"),
            (2, "/scratch/two"),
            (3, "/projects/intavia/crates/cli"),
            (4, "/projects/nova"),
        ]);

        let g = partition_for_test(
            vec![(0, None), (1, None), (2, None), (3, None), (4, None)],
            &groups,
            &assigns,
            &dirs,
            &HashMap::new(),
        );

        assert_eq!(
            g.items,
            vec![
                GroupedTabListItem::Ungrouped((0, None)),
                GroupedTabListItem::Group {
                    id: gid("intavia"),
                    tabs: vec![(1, None), (3, None)],
                },
                GroupedTabListItem::Ungrouped((2, None)),
                GroupedTabListItem::Group {
                    id: gid("nova"),
                    tabs: vec![(4, None)],
                },
            ]
        );
    }

    #[test]
    fn empty_groups_remain_visible_in_output() {
        // Empty groups stay so the user can see freshly-created groups that
        // have no auto-assignment yet. Rendering surfaces a "(0)" count.
        let groups = TabGroups(vec![group("intavia", "Intavia"), group("nova", "Nova")]);
        let assigns = assignments(&[(
            "/projects/intavia",
            TabGroupAssignment::InGroup(gid("intavia")),
        )]);
        let dirs = dirs(&[(0, "/projects/intavia")]);

        let g = partition_for_test(vec![(0, None)], &groups, &assigns, &dirs, &HashMap::new());

        // Both groups appear, in TabGroups order. Nova is empty.
        assert_eq!(g.groups.len(), 2);
        assert_eq!(g.groups[0].0, gid("intavia"));
        assert_eq!(g.groups[0].1, vec![(0, None)]);
        assert_eq!(g.groups[1].0, gid("nova"));
        assert!(g.groups[1].1.is_empty());
    }

    #[test]
    fn unknown_group_ids_in_assignments_fall_back_to_ungrouped() {
        // Stale assignment pointing at a group that no longer exists in
        // TabGroups (e.g. user deleted the group but assignments weren't
        // cleaned up). The helper must defensively treat this as ungrouped.
        // the *tab* lands in the ungrouped bucket. The known-but-empty
        // `intavia` group still appears in the output (empty groups are
        // visible now, see `empty_groups_remain_visible_in_output`).
        let groups = TabGroups(vec![group("intavia", "Intavia")]);
        let assigns = assignments(&[(
            "/projects/deleted",
            TabGroupAssignment::InGroup(gid("nonexistent")),
        )]);
        let dirs = dirs(&[(0, "/projects/deleted")]);

        let g = partition_for_test(vec![(0, None)], &groups, &assigns, &dirs, &HashMap::new());
        assert_eq!(g.ungrouped, vec![(0, None)]);
        assert_eq!(g.groups.len(), 1);
        assert_eq!(g.groups[0].0, gid("intavia"));
        assert!(g.groups[0].1.is_empty());
    }

    #[test]
    fn search_payload_round_trips_through_partition() {
        // Critical: `Option<Vec<PaneId>>` is the search-filter payload that
        // panes-mode rendering needs. The helper must not lose it.
        let groups = TabGroups(vec![group("intavia", "Intavia")]);
        let assigns = assignments(&[(
            "/projects/intavia",
            TabGroupAssignment::InGroup(gid("intavia")),
        )]);
        let dirs = dirs(&[(0, "/projects/intavia"), (1, "/scratch")]);

        let pane_filter = Some(vec![PaneId::from(TerminalPaneId::dummy_terminal_pane_id())]);
        let g = partition_for_test(
            vec![(0, pane_filter.clone()), (1, None)],
            &groups,
            &assigns,
            &dirs,
            &HashMap::new(),
        );

        assert_eq!(g.groups[0].1, vec![(0, pane_filter)]);
        assert_eq!(g.ungrouped, vec![(1, None)]);
    }

    #[test]
    fn collapse_flag_does_not_affect_partition() {
        // Collapse is a render concern; partitioning should ignore it.
        let mut g_intavia = group("intavia", "Intavia");
        g_intavia.collapsed = true;
        let groups = TabGroups(vec![g_intavia]);
        let assigns = assignments(&[(
            "/projects/intavia",
            TabGroupAssignment::InGroup(gid("intavia")),
        )]);
        let dirs = dirs(&[(0, "/projects/intavia/crates/app")]);

        let g = partition_for_test(vec![(0, None)], &groups, &assigns, &dirs, &HashMap::new());
        assert_eq!(g.groups.len(), 1);
        assert_eq!(g.groups[0].1, vec![(0, None)]);
    }

    #[test]
    fn excluded_child_directory_lands_ungrouped_even_with_parent_in_group() {
        let groups = TabGroups(vec![group("intavia", "Intavia")]);
        let assigns = assignments(&[
            (
                "/projects/intavia",
                TabGroupAssignment::InGroup(gid("intavia")),
            ),
            ("/projects/intavia/scratch", TabGroupAssignment::Excluded),
        ]);
        let dirs = dirs(&[
            (0, "/projects/intavia/crates/app"),
            (1, "/projects/intavia/scratch/foo"),
        ]);

        let g = partition_for_test(
            vec![(0, None), (1, None)],
            &groups,
            &assigns,
            &dirs,
            &HashMap::new(),
        );

        assert_eq!(g.ungrouped, vec![(1, None)]);
        assert_eq!(g.groups[0].1, vec![(0, None)]);
    }

    #[test]
    fn per_tab_override_wins_over_directory_assignment() {
        let groups = TabGroups(vec![group("intavia", "Intavia"), group("nova", "Nova")]);
        let assigns = assignments(&[(
            "/projects/intavia",
            TabGroupAssignment::InGroup(gid("intavia")),
        )]);
        let dirs = dirs(&[(0, "/projects/intavia"), (1, "/projects/intavia")]);
        let overrides = HashMap::from([(1, gid("nova"))]);

        let g = partition_for_test(
            vec![(0, None), (1, None)],
            &groups,
            &assigns,
            &dirs,
            &overrides,
        );

        assert_eq!(g.groups[0].1, vec![(0, None)]);
        assert_eq!(g.groups[1].1, vec![(1, None)]);
    }
}
