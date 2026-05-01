// On Windows, we don't want to display a console window when the application is running in release
// builds. See https://doc.rust-lang.org/reference/runtime.html#the-windows_subsystem-attribute.
#![cfg_attr(feature = "release_bundle", windows_subsystem = "windows")]

#[cfg(not(feature = "personal_dev_config"))]
#[path = "channel_config.rs"]
mod channel_config;

use anyhow::Result;
use warp_core::{
    channel::{Channel, ChannelConfig, ChannelState, OzConfig, WarpServerConfig},
    features,
    AppId,
};

#[cfg(feature = "personal_dev_config")]
fn dev_channel_config() -> ChannelConfig {
    ChannelConfig {
        app_id: AppId::new("dev", "warp", "WarpDev"),
        logfile_name: "warp-dev.log".into(),
        server_config: WarpServerConfig::production(),
        oz_config: OzConfig::production(),
        telemetry_config: None,
        crash_reporting_config: None,
        autoupdate_config: None,
        mcp_static_config: None,
    }
}

#[cfg(not(feature = "personal_dev_config"))]
fn dev_channel_config() -> ChannelConfig {
    channel_config::load_config!("dev")
}

// Simple wrapper around warp::run() for dev channel builds.
fn main() -> Result<()> {
    ChannelState::set(
        ChannelState::new(Channel::Dev, dev_channel_config())
            .with_additional_features(features::DEBUG_FLAGS)
            .with_additional_features(features::DOGFOOD_FLAGS)
            .with_additional_features(features::PREVIEW_FLAGS),
    );

    warp::run()
}
