//! `shell_apps` of types.toml: silica.assets keeps only the shell's own app icons.
#![cfg(feature = "compile")]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

mod common;

use common::*;
use silica_assets::*;

#[test]
fn shell_apps_limit_the_compiled_app_icons() {
    let fixture = fixture();
    let table = fixture.root().join("shell-types.toml");
    std::fs::write(
        &table,
        "shell_apps = [\"mail\"]\n[app]\n\"dev.frascella.Mail\" = \"mail\"\n",
    )
    .unwrap();
    let mut options = fixture.options.clone();
    options.types = Some(table);
    let (bytes, report) = silica_assets::compile::compile(&options).unwrap();
    let assets = Assets::from_bytes(bytes).unwrap();
    assert!(assets.lookup(KeySpace::App, "mail").is_some());
    assert!(assets.lookup(KeySpace::App, "dev.frascella.Mail").is_some());
    assert!(assets.lookup(KeySpace::App, "clock").is_none());
    assert!(assets.lookup(KeySpace::App, "calendar").is_none());
    assert_eq!(report.apps, 1);
}

#[test]
fn without_the_key_every_app_icon_is_compiled() {
    let fixture = fixture();
    let assets = Assets::from_bytes(compile_fixture(&fixture)).unwrap();
    for id in ["mail", "clock", "calendar"] {
        assert!(assets.lookup(KeySpace::App, id).is_some(), "{id}");
    }
}

#[test]
fn the_shipped_table_keeps_only_the_shells_app_icons() {
    let table: toml::Table = silica_assets::compile::DEFAULT_TYPES.parse().unwrap();
    let shell: Vec<&str> = table["shell_apps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(shell, ["apps", "mission-control", "screenshot", "silica"]);
    for target in table["app"].as_table().unwrap().values() {
        assert!(shell.contains(&target.as_str().unwrap()), "{target}");
    }
}
