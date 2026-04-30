use std::collections::HashMap;
use std::path::Path;

use super::*;
use crate::test_util::settings::initialize_settings_for_tests;
use settings::Setting;
use warpui::{App, SingletonEntity};

fn assignments(entries: &[(&str, TabGroupAssignment)]) -> TabGroupAssignments {
    let map: HashMap<String, TabGroupAssignment> = entries
        .iter()
        .map(|(path, a)| ((*path).to_string(), a.clone()))
        .collect();
    TabGroupAssignments(map)
}

fn gid(s: &str) -> TabGroupId {
    TabGroupId(s.to_string())
}

#[test]
fn group_for_directory_returns_none_when_empty() {
    let a = TabGroupAssignments::default();
    assert!(a
        .group_for_directory(Path::new("/nonexistent/intavia"))
        .is_none());
}

#[test]
fn group_for_directory_matches_broader_in_group_prefix() {
    let a = assignments(&[(
        "/nonexistent/intavia",
        TabGroupAssignment::InGroup(gid("g-intavia")),
    )]);
    assert_eq!(
        a.group_for_directory(Path::new("/nonexistent/intavia/crates/app")),
        Some(gid("g-intavia"))
    );
}

#[test]
fn group_for_directory_excluded_shadows_broader_in_group() {
    // Critical correctness test: longest-prefix-then-interpret.
    // If we filter Excluded *before* the max (like DirectoryTabColors does), the
    // child Excluded is dropped and the broader InGroup wrongly wins.
    let a = assignments(&[
        (
            "/nonexistent/intavia",
            TabGroupAssignment::InGroup(gid("g-intavia")),
        ),
        ("/nonexistent/intavia/scratch", TabGroupAssignment::Excluded),
    ]);

    // A path inside the excluded child resolves to ungrouped.
    assert_eq!(
        a.group_for_directory(Path::new("/nonexistent/intavia/scratch")),
        None
    );
    assert_eq!(
        a.group_for_directory(Path::new("/nonexistent/intavia/scratch/sub")),
        None
    );

    // A sibling inside the parent (not under the excluded child) still gets the
    // broader InGroup.
    assert_eq!(
        a.group_for_directory(Path::new("/nonexistent/intavia/crates/app")),
        Some(gid("g-intavia"))
    );
}

#[test]
fn group_for_directory_longer_in_group_overrides_shorter() {
    let a = assignments(&[
        (
            "/nonexistent/projects",
            TabGroupAssignment::InGroup(gid("g-all")),
        ),
        (
            "/nonexistent/projects/intavia",
            TabGroupAssignment::InGroup(gid("g-intavia")),
        ),
    ]);
    assert_eq!(
        a.group_for_directory(Path::new("/nonexistent/projects/intavia/cli")),
        Some(gid("g-intavia"))
    );
    assert_eq!(
        a.group_for_directory(Path::new("/nonexistent/projects/nova")),
        Some(gid("g-all"))
    );
}

#[test]
fn group_for_directory_equal_length_tiebreak_is_deterministic() {
    // Two distinct keys of equal length that both happen to be prefixes of the
    // same dir would be a misconfiguration in practice, but the resolver must
    // still be deterministic: lexicographically larger key wins.
    let a = assignments(&[
        ("/nonexistent/aaa", TabGroupAssignment::InGroup(gid("g-a"))),
        ("/nonexistent/aab", TabGroupAssignment::InGroup(gid("g-b"))),
    ]);

    // Each subdirectory has only one matching prefix, so this is mostly a
    // sanity check; the meaningful assertion is determinism on rerun.
    let r1 = a.group_for_directory(Path::new("/nonexistent/aaa/sub"));
    let r2 = a.group_for_directory(Path::new("/nonexistent/aaa/sub"));
    assert_eq!(r1, r2);
    assert_eq!(r1, Some(gid("g-a")));
}

#[test]
fn with_assignment_inserts_canonicalized_key() {
    let a = TabGroupAssignments::default();
    let updated = a.with_assignment(
        Path::new("/nonexistent/intavia"),
        TabGroupAssignment::InGroup(gid("g-intavia")),
    );
    assert_eq!(
        updated.group_for_directory(Path::new("/nonexistent/intavia")),
        Some(gid("g-intavia"))
    );
}

#[test]
fn tab_groups_uses_synced_toml_path() {
    assert_eq!(TabGroups::toml_path(), Some("appearance.tabs.tab_groups"));
}

#[test]
fn tab_group_assignments_uses_local_only_toml_path() {
    assert_eq!(
        TabGroupAssignments::toml_path(),
        Some("appearance.tabs.tab_group_assignments")
    );
}

#[test]
fn tab_groups_with_collapsed_updates_only_matching_id() {
    let g1 = TabGroup {
        id: gid("g1"),
        name: "Intavia".into(),
        color: None,
        collapsed: false,
    };
    let g2 = TabGroup {
        id: gid("g2"),
        name: "Nova".into(),
        color: None,
        collapsed: false,
    };
    let groups = TabGroups(vec![g1.clone(), g2.clone()]);

    let updated = groups.with_collapsed(&gid("g1"), true);
    assert!(updated.get(&gid("g1")).unwrap().collapsed);
    assert!(!updated.get(&gid("g2")).unwrap().collapsed);

    // Unknown id is a no-op.
    let unchanged = groups.with_collapsed(&gid("missing"), true);
    assert_eq!(unchanged, groups);
}

#[test]
fn use_latest_user_prompt_as_conversation_title_in_tab_names_defaults_to_false() {
    App::test((), |mut app| async move {
        initialize_settings_for_tests(&mut app);

        TabSettings::handle(&app).read(&app, |settings, _ctx| {
            assert!(!*settings.use_latest_user_prompt_as_conversation_title_in_tab_names);
        });
    });
}

#[test]
fn use_latest_user_prompt_as_conversation_title_in_tab_names_uses_vertical_tabs_path() {
    assert_eq!(
        UseLatestUserPromptAsConversationTitleInTabNames::toml_path(),
        Some("appearance.vertical_tabs.use_latest_prompt_as_title")
    );
    assert_eq!(
        UseLatestUserPromptAsConversationTitleInTabNames::hierarchy(),
        Some("appearance.vertical_tabs")
    );
    assert_eq!(
        UseLatestUserPromptAsConversationTitleInTabNames::toml_key(),
        "use_latest_prompt_as_title"
    );
}

#[test]
fn show_vertical_tab_panel_in_restored_windows_defaults_to_false() {
    App::test((), |mut app| async move {
        initialize_settings_for_tests(&mut app);

        TabSettings::handle(&app).read(&app, |settings, _ctx| {
            assert!(!*settings.show_vertical_tab_panel_in_restored_windows);
        });
    });
}

#[test]
fn show_vertical_tab_panel_in_restored_windows_uses_vertical_tabs_path() {
    assert_eq!(
        ShowVerticalTabPanelInRestoredWindows::toml_path(),
        Some("appearance.vertical_tabs.show_panel_in_restored_windows")
    );
    assert_eq!(
        ShowVerticalTabPanelInRestoredWindows::hierarchy(),
        Some("appearance.vertical_tabs")
    );
    assert_eq!(
        ShowVerticalTabPanelInRestoredWindows::toml_key(),
        "show_panel_in_restored_windows"
    );
}
