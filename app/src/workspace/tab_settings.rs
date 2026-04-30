use std::collections::HashMap;
use std::path::Path;

use settings::{
    macros::define_settings_group, RespectUserSyncSetting, SupportedPlatforms, SyncToCloud,
};
use warp_core::ui::theme::AnsiColorIdentifier;

#[derive(
    Default,
    Debug,
    serde::Serialize,
    serde::Deserialize,
    PartialEq,
    Copy,
    Clone,
    schemars::JsonSchema,
    settings_value::SettingsValue,
)]
#[schemars(
    description = "Where new tabs are placed in the tab bar.",
    rename_all = "snake_case"
)]
pub enum NewTabPlacement {
    #[default]
    AfterCurrentTab,
    AfterAllTabs,
}

settings::macros::implement_setting_for_enum!(
    NewTabPlacement,
    TabSettings,
    SupportedPlatforms::ALL,
    SyncToCloud::Never,
    private: false,
    toml_path: "general.new_tab_placement",
    description: "Where new tabs are placed in the tab bar.",
);

#[derive(
    Default,
    Debug,
    serde::Serialize,
    serde::Deserialize,
    PartialEq,
    Copy,
    Clone,
    schemars::JsonSchema,
    settings_value::SettingsValue,
)]
#[schemars(
    description = "Position of the close button on tabs.",
    rename_all = "snake_case"
)]
pub enum TabCloseButtonPosition {
    #[default]
    Right,
    Left,
}

settings::macros::implement_setting_for_enum!(
    TabCloseButtonPosition,
    TabSettings,
    SupportedPlatforms::ALL,
    SyncToCloud::Globally(RespectUserSyncSetting::Yes),
    private: false,
    toml_path: "appearance.tabs.tab_close_button_position",
    description: "Position of the close button on tabs.",
);

/// Visibility options for workspace decorations like the tab bar.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Eq,
    PartialEq,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
    settings_value::SettingsValue,
)]
#[schemars(
    description = "When workspace decorations such as the tab bar are visible.",
    rename_all = "snake_case"
)]
pub enum WorkspaceDecorationVisibility {
    /// Always show workspace decorations.
    AlwaysShow,
    /// Hide workspace decorations if fullscreen.
    #[default]
    HideFullscreen,
    /// Only show workspace decorations on hover.
    OnHover,
}

settings::macros::implement_setting_for_enum!(
    WorkspaceDecorationVisibility,
    TabSettings,
    SupportedPlatforms::ALL,
    SyncToCloud::Globally(RespectUserSyncSetting::Yes),
    private: false,
    toml_path: "appearance.tabs.workspace_decoration_visibility",
    description: "When workspace decorations such as the tab bar are visible.",
);

impl WorkspaceDecorationVisibility {
    /// Choose a visibility setting that's logically opposite from this one.
    pub fn toggled(self) -> Self {
        // If we add other variants, there should still be logical opposites for each. For example,
        // toggling from any form of hidden workspace decorations should re-enable them.
        match self {
            WorkspaceDecorationVisibility::AlwaysShow => WorkspaceDecorationVisibility::OnHover,
            WorkspaceDecorationVisibility::OnHover => WorkspaceDecorationVisibility::HideFullscreen,
            WorkspaceDecorationVisibility::HideFullscreen => WorkspaceDecorationVisibility::OnHover,
        }
    }

    /// True if this is a setting where workspace decorations are hidden by default.
    pub fn hides_decorations_by_default(self) -> bool {
        matches!(self, WorkspaceDecorationVisibility::OnHover,)
    }

    /// True if *window* decorations should be shown.
    pub fn show_window_decorations(self) -> bool {
        !matches!(self, WorkspaceDecorationVisibility::OnHover)
    }
}

/// Represents the color state for a directory entry in the tab-color settings.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
    settings_value::SettingsValue,
)]
#[schemars(
    description = "Color assignment state for a directory's tab.",
    rename_all = "snake_case"
)]
pub enum DirectoryTabColor {
    /// User explicitly removed this directory. Retained for backwards compatibility with settings files written by older versions.
    #[schemars(description = "The directory was explicitly removed from tab coloring.")]
    Suppressed,
    /// Directory is tracked but has no assigned color.
    #[schemars(description = "The directory is tracked but has no assigned color.")]
    Unassigned,
    /// Directory is tracked with a specific color.
    #[schemars(description = "The directory is assigned a specific color.")]
    Color(AnsiColorIdentifier),
}

impl DirectoryTabColor {
    pub(crate) fn ansi_color(self) -> Option<AnsiColorIdentifier> {
        match self {
            DirectoryTabColor::Color(c) => Some(c),
            DirectoryTabColor::Suppressed | DirectoryTabColor::Unassigned => None,
        }
    }
}

/// User-configured directory→color mappings for tab coloring.
///
/// Keys are directory paths (as strings). Values indicate the color state:
/// - `Suppressed`: directory was explicitly removed by the user via the per-row X button.
///   Retained so `color_for_directory` can shadow broader prefix matches, and for
///   backwards compatibility with settings files written by older versions.
/// - `Unassigned`: directory is tracked but has no specific color.
/// - `Color(c)`: directory is tracked with the given color.
#[derive(
    Default,
    Debug,
    Clone,
    serde::Serialize,
    serde::Deserialize,
    PartialEq,
    Eq,
    schemars::JsonSchema,
    settings_value::SettingsValue,
)]
#[schemars(description = "Mapping of directory paths to their tab color assignments.")]
pub struct DirectoryTabColors(pub(crate) HashMap<String, DirectoryTabColor>);

settings::macros::implement_setting_for_enum!(
    DirectoryTabColors,
    TabSettings,
    SupportedPlatforms::ALL,
    SyncToCloud::Never,
    private: false,
    toml_path: "appearance.tabs.directory_tab_colors",
    max_table_depth: 0,
    description: "Mapping of directory paths to their tab color assignments.",
    feature_flag: warp_core::features::FeatureFlag::DirectoryTabColors,
);

impl DirectoryTabColors {
    /// Returns the configured tab color for a directory using longest-prefix matching.
    /// Returns `None` if no configured directory is a prefix of `dir`.
    pub fn color_for_directory(&self, dir: &Path) -> Option<DirectoryTabColor> {
        let canonical_dir = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
        self.0
            .iter()
            .filter_map(|(configured_path, color)| {
                let configured = Path::new(configured_path);
                match color {
                    DirectoryTabColor::Suppressed => None,
                    _ => canonical_dir
                        .starts_with(configured)
                        .then_some((configured, *color)),
                }
            })
            .max_by_key(|(configured, _)| configured.as_os_str().len())
            .map(|(_, color)| color)
    }

    /// Returns a new value with the given directory's color updated.
    pub fn with_color(&self, path: &Path, color: DirectoryTabColor) -> Self {
        let mut map = self.0.clone();

        let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        map.insert(canonical.to_string_lossy().to_string(), color);
        Self(map)
    }
}

// =====================================================================
// Tab Groups
//
// Two-setting model deliberately split by sync semantics:
//   - `TabGroups` (cloud-synced) holds *definitions only* (id, name, color,
//     collapsed). No path data, so it is portable across machines.
//   - `TabGroupAssignments` (local-only) maps canonical absolute paths to a
//     group id. Absolute paths are not portable across machines, which is why
//     `DirectoryTabColors` is `SyncToCloud::Never`; same reasoning here.
//
// Assignments are directory-level: every tab whose focused-pane directory has
// the longest matching prefix in `TabGroupAssignments` lands in the same
// group. Manual per-tab membership for broad/no-directory tabs is stored on
// restored workspace tabs instead of in cloud-synced settings.
// =====================================================================

/// Stable identifier for a tab group.
///
/// Stored as a string (UUID v4 generated at creation time). Using a string
/// keeps the settings/serde derive plumbing simple; `Uuid` itself does not
/// derive `settings_value::SettingsValue` cleanly through the macros today.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
    settings_value::SettingsValue,
)]
#[schemars(description = "Stable identifier for a tab group.")]
pub struct TabGroupId(pub String);

impl TabGroupId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for TabGroupId {
    fn default() -> Self {
        Self::new()
    }
}

/// A single tab-group definition. Cloud-synced; holds no absolute paths.
#[derive(
    Debug,
    Clone,
    PartialEq,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
    settings_value::SettingsValue,
)]
#[schemars(description = "Definition of a single tab group.")]
pub struct TabGroup {
    /// Stable id used by `TabGroupAssignments` to point at this group.
    pub id: TabGroupId,
    /// Human-readable name shown in the sidebar header.
    pub name: String,
    /// Optional accent color for the group header.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<AnsiColorIdentifier>,
    /// Whether the group is collapsed in the sidebar. Persists across restarts.
    #[serde(default)]
    pub collapsed: bool,
}

/// Ordered list of tab-group definitions. Cloud-synced.
#[derive(
    Default,
    Debug,
    Clone,
    PartialEq,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
    settings_value::SettingsValue,
)]
#[schemars(description = "User-configured tab groups (definitions only; no path data).")]
pub struct TabGroups(pub Vec<TabGroup>);

impl TabGroups {
    pub fn iter(&self) -> std::slice::Iter<'_, TabGroup> {
        self.0.iter()
    }

    pub fn get(&self, id: &TabGroupId) -> Option<&TabGroup> {
        self.0.iter().find(|g| &g.id == id)
    }

    pub fn contains(&self, id: &TabGroupId) -> bool {
        self.get(id).is_some()
    }

    /// Returns a new `TabGroups` with the given group appended.
    pub fn with_added(&self, group: TabGroup) -> Self {
        let mut groups = self.0.clone();
        groups.push(group);
        Self(groups)
    }

    /// Returns a new `TabGroups` with the matching group's collapsed flag set.
    /// If the id is unknown, returns an unchanged clone.
    pub fn with_collapsed(&self, id: &TabGroupId, collapsed: bool) -> Self {
        let groups = self
            .0
            .iter()
            .map(|g| {
                if &g.id == id {
                    TabGroup {
                        collapsed,
                        ..g.clone()
                    }
                } else {
                    g.clone()
                }
            })
            .collect();
        Self(groups)
    }
}

settings::macros::implement_setting_for_enum!(
    TabGroups,
    TabSettings,
    SupportedPlatforms::ALL,
    SyncToCloud::Globally(RespectUserSyncSetting::Yes),
    private: false,
    toml_path: "appearance.tabs.tab_groups",
    description: "User-configured tab groups (definitions only; no path data).",
    feature_flag: warp_core::features::FeatureFlag::TabGroups,
);

/// Assignment of a directory to a tab-group state.
///
/// `Excluded` is the equivalent of `DirectoryTabColor::Suppressed`, but with
/// *different shadowing semantics* (see `group_for_directory`).
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
    settings_value::SettingsValue,
)]
#[schemars(
    description = "Assignment state for a directory in tab-group settings.",
    rename_all = "snake_case"
)]
pub enum TabGroupAssignment {
    /// Directory is assigned to a tab group.
    InGroup(TabGroupId),
    /// Directory is explicitly excluded and shadows broader `InGroup` matches.
    Excluded,
}

/// User-configured directory-to-tab-group assignments.
///
/// **Local-only** (`SyncToCloud::Never`). Absolute paths are not portable
/// across machines, mirroring the rationale for `DirectoryTabColors`.
///
/// Resolution is *directory-level*: when two tabs share the same canonical
/// directory, they will land in the same group unless a tab has local
/// per-tab membership stored in its workspace snapshot.
#[derive(
    Default,
    Debug,
    Clone,
    serde::Serialize,
    serde::Deserialize,
    PartialEq,
    Eq,
    schemars::JsonSchema,
    settings_value::SettingsValue,
)]
#[schemars(description = "Mapping of directory paths to their tab-group assignments.")]
pub struct TabGroupAssignments(pub(crate) HashMap<String, TabGroupAssignment>);

settings::macros::implement_setting_for_enum!(
    TabGroupAssignments,
    TabSettings,
    SupportedPlatforms::ALL,
    SyncToCloud::Never,
    private: false,
    toml_path: "appearance.tabs.tab_group_assignments",
    max_table_depth: 0,
    description: "Mapping of directory paths to their tab-group assignments.",
    feature_flag: warp_core::features::FeatureFlag::TabGroups,
);

impl TabGroupAssignments {
    /// Returns the group id (if any) that `dir` resolves to, using
    /// **longest-prefix-then-interpret** semantics.
    ///
    /// Important: this is *not* the same algorithm as
    /// `DirectoryTabColors::color_for_directory`. The colors lookup filters
    /// `Suppressed` entries *before* selecting the longest prefix, which means
    /// a suppressed child cannot shadow a colored parent. Tab groups need the
    /// opposite: a more-specific `Excluded` *must* shadow a broader `InGroup`
    /// so users can carve a directory out of an inherited group.
    pub fn group_for_directory(&self, dir: &Path) -> Option<TabGroupId> {
        let canonical_dir = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
        let winner = self
            .0
            .iter()
            .filter_map(|(configured_path, assignment)| {
                let configured = Path::new(configured_path);
                canonical_dir
                    .starts_with(configured)
                    .then_some((configured, assignment))
            })
            // Longest prefix wins. Tiebreak: lexicographically larger path,
            // for determinism when two equal-length keys both match.
            .max_by(|(a_path, _), (b_path, _)| {
                a_path
                    .as_os_str()
                    .len()
                    .cmp(&b_path.as_os_str().len())
                    .then_with(|| a_path.as_os_str().cmp(b_path.as_os_str()))
            });

        match winner {
            Some((_, TabGroupAssignment::InGroup(id))) => Some(id.clone()),
            // `Excluded` wins over a broader InGroup, so the tab is ungrouped.
            Some((_, TabGroupAssignment::Excluded)) => None,
            None => None,
        }
    }

    /// Returns a new value with the given directory's assignment set.
    pub fn with_assignment(&self, path: &Path, assignment: TabGroupAssignment) -> Self {
        let mut map = self.0.clone();
        let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        map.insert(canonical.to_string_lossy().to_string(), assignment);
        Self(map)
    }
}

#[derive(
    Clone,
    Debug,
    Default,
    serde::Serialize,
    serde::Deserialize,
    PartialEq,
    Eq,
    schemars::JsonSchema,
    settings_value::SettingsValue,
)]
#[schemars(
    description = "Configuration for the header toolbar chips in the vertical tab panel header.",
    rename_all = "snake_case"
)]
pub enum HeaderToolbarChipSelection {
    #[default]
    Default,
    Custom {
        left: Vec<super::header_toolbar_item::HeaderToolbarItemKind>,
        right: Vec<super::header_toolbar_item::HeaderToolbarItemKind>,
    },
}

impl HeaderToolbarChipSelection {
    pub fn left_items(&self) -> Vec<super::header_toolbar_item::HeaderToolbarItemKind> {
        use super::header_toolbar_item::HeaderToolbarItemKind;
        match self {
            Self::Default => HeaderToolbarItemKind::default_left(),
            Self::Custom { left, .. } => left.clone(),
        }
    }

    pub fn right_items(&self) -> Vec<super::header_toolbar_item::HeaderToolbarItemKind> {
        use super::header_toolbar_item::HeaderToolbarItemKind;
        match self {
            Self::Default => HeaderToolbarItemKind::default_right(),
            Self::Custom { right, .. } => right.clone(),
        }
    }
}

settings::macros::implement_setting_for_enum!(
    HeaderToolbarChipSelection,
    TabSettings,
    SupportedPlatforms::ALL,
    SyncToCloud::Globally(RespectUserSyncSetting::Yes),
    private: false,
    toml_path: "appearance.tabs.header_toolbar_chip_selection",
    description: "Configuration for the header toolbar chips in the vertical tab panel header.",
);

#[derive(
    Default,
    Debug,
    serde::Serialize,
    serde::Deserialize,
    PartialEq,
    Copy,
    Clone,
    schemars::JsonSchema,
    settings_value::SettingsValue,
)]
#[schemars(
    description = "Display mode for the vertical tab bar.",
    rename_all = "snake_case"
)]
pub enum VerticalTabsViewMode {
    #[default]
    Compact,
    Expanded,
}

settings::macros::implement_setting_for_enum!(
    VerticalTabsViewMode,
    TabSettings,
    SupportedPlatforms::ALL,
    SyncToCloud::Globally(RespectUserSyncSetting::Yes),
    private: false,
    toml_path: "appearance.vertical_tabs.view_mode",
    description: "Display mode for the vertical tab bar.",
);

#[derive(
    Default,
    Debug,
    serde::Serialize,
    serde::Deserialize,
    PartialEq,
    Copy,
    Clone,
    schemars::JsonSchema,
    settings_value::SettingsValue,
)]
#[schemars(
    description = "Granularity of rows displayed in the vertical tabs panel.",
    rename_all = "snake_case"
)]
pub enum VerticalTabsDisplayGranularity {
    #[default]
    Panes,
    Tabs,
}

settings::macros::implement_setting_for_enum!(
    VerticalTabsDisplayGranularity,
    TabSettings,
    SupportedPlatforms::ALL,
    SyncToCloud::Globally(RespectUserSyncSetting::Yes),
    private: false,
    toml_path: "appearance.vertical_tabs.display_granularity",
    description: "Granularity of rows displayed in the vertical tabs panel.",
);

#[derive(
    Default,
    Debug,
    serde::Serialize,
    serde::Deserialize,
    PartialEq,
    Copy,
    Clone,
    schemars::JsonSchema,
    settings_value::SettingsValue,
)]
#[schemars(
    description = "Tab item display mode in vertical tabs.",
    rename_all = "snake_case"
)]
pub enum VerticalTabsTabItemMode {
    #[default]
    FocusedSession,
    Summary,
}

settings::macros::implement_setting_for_enum!(
    VerticalTabsTabItemMode,
    TabSettings,
    SupportedPlatforms::ALL,
    SyncToCloud::Globally(RespectUserSyncSetting::Yes),
    private: false,
    toml_path: "appearance.vertical_tabs.tab_item_mode",
    description: "Tab item display mode in vertical tabs.",
);

#[derive(
    Default,
    Debug,
    serde::Serialize,
    serde::Deserialize,
    PartialEq,
    Copy,
    Clone,
    schemars::JsonSchema,
    settings_value::SettingsValue,
)]
#[schemars(
    description = "Primary information displayed on vertical tabs.",
    rename_all = "snake_case"
)]
pub enum VerticalTabsPrimaryInfo {
    #[default]
    Command,
    WorkingDirectory,
    Branch,
}

settings::macros::implement_setting_for_enum!(
    VerticalTabsPrimaryInfo,
    TabSettings,
    SupportedPlatforms::ALL,
    SyncToCloud::Globally(RespectUserSyncSetting::Yes),
    private: false,
    toml_path: "appearance.vertical_tabs.primary_info",
    description: "The primary information displayed on vertical tabs.",
);

#[derive(
    Default,
    Debug,
    serde::Serialize,
    serde::Deserialize,
    PartialEq,
    Copy,
    Clone,
    schemars::JsonSchema,
    settings_value::SettingsValue,
)]
#[schemars(
    description = "Subtitle shown on compact vertical tabs.",
    rename_all = "snake_case"
)]
pub enum VerticalTabsCompactSubtitle {
    #[default]
    Branch,
    WorkingDirectory,
    Command,
}

settings::macros::implement_setting_for_enum!(
    VerticalTabsCompactSubtitle,
    TabSettings,
    SupportedPlatforms::ALL,
    SyncToCloud::Globally(RespectUserSyncSetting::Yes),
    private: false,
    toml_path: "appearance.vertical_tabs.compact_subtitle",
    description: "Subtitle shown on compact vertical tabs.",
);

define_settings_group!(TabSettings, settings: [
    show_indicators: ShowIndicatorsButton {
        type: bool,
        default: true,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "appearance.tabs.show_indicators_button",
        description: "Whether to show activity indicators on tabs.",
    },
    show_code_review_button: ShowCodeReviewButton {
        type: bool,
        default: true,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "code.editor.show_code_review_button",
        description: "Whether to show the code review button on tabs.",
    },
    show_code_review_diff_stats: ShowCodeReviewDiffStats {
        type: bool,
        default: true,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "code.editor.show_code_review_diff_stats",
        description: "Whether to show lines added/removed counts on the code review button.",
    },
    preserve_active_tab_color: PreserveActiveTabColor {
        type: bool,
        default: false,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "appearance.tabs.preserve_active_tab_color",
        description: "Whether to preserve the active tab's color when switching tabs.",
    },
    use_vertical_tabs: UseVerticalTabs {
        type: bool,
        default: false,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "appearance.vertical_tabs.enabled",
        description: "Whether to display tabs vertically instead of horizontally.",
    },
    show_vertical_tab_panel_in_restored_windows: ShowVerticalTabPanelInRestoredWindows {
        type: bool,
        default: false,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "appearance.vertical_tabs.show_panel_in_restored_windows",
        description: "When restoring a window, open the vertical tabs panel even if it was closed when the session was saved.",
    },
    use_latest_user_prompt_as_conversation_title_in_tab_names: UseLatestUserPromptAsConversationTitleInTabNames {
        type: bool,
        default: false,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "appearance.vertical_tabs.use_latest_prompt_as_title",
        description: "Whether vertical tab names for agent conversations use the latest user prompt.",
    },
    vertical_tabs_display_granularity: VerticalTabsDisplayGranularity,
    vertical_tabs_tab_item_mode: VerticalTabsTabItemMode,
    vertical_tabs_view_mode: VerticalTabsViewMode,
    vertical_tabs_primary_info: VerticalTabsPrimaryInfo,
    vertical_tabs_compact_subtitle: VerticalTabsCompactSubtitle,
    vertical_tabs_show_pr_link: VerticalTabsShowPrLink {
        type: bool,
        default: true,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "appearance.vertical_tabs.show_pr_link",
        description: "Whether to show PR links on vertical tabs.",
    },
    vertical_tabs_show_diff_stats: VerticalTabsShowDiffStats {
        type: bool,
        default: true,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "appearance.vertical_tabs.show_diff_stats",
        description: "Whether to show diff stats on vertical tabs.",
    },
    vertical_tabs_show_details_on_hover: VerticalTabsShowDetailsOnHover {
        type: bool,
        default: true,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "appearance.vertical_tabs.show_details_on_hover",
        description: "Whether to show a details sidecar when hovering over a vertical tab.",
    },
    header_toolbar_chip_selection: HeaderToolbarChipSelection,
    new_tab_placement: NewTabPlacement,
    workspace_decoration_visibility: WorkspaceDecorationVisibility,
    close_button_position: TabCloseButtonPosition,
    directory_tab_colors: DirectoryTabColors,
    tab_groups: TabGroups,
    tab_group_assignments: TabGroupAssignments,
]);

#[cfg(test)]
#[path = "tab_settings_tests.rs"]
mod tests;
