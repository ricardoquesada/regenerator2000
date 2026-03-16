#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
/// Config serialization tests
///
/// Verifies that `SystemConfig` serializes/deserializes correctly,
/// handles missing fields with defaults, and survives round-trips.
use regenerator2000_core::config::SystemConfig;
use std::path::PathBuf;

#[test]
fn default_config_values() {
    let config = SystemConfig::default();
    assert!(config.open_last_project);
    assert!(config.last_project_path.is_none());
    assert_eq!(config.theme, "Dracula");
    assert!(config.sync_blocks_view);
    assert!(config.sync_hex_dump);
    assert!(!config.sync_charset_view);
    assert!(!config.sync_sprites_view);
    assert!(!config.sync_bitmap_view);
    assert!((config.entropy_threshold - 7.5).abs() < f32::EPSILON);
    assert!(config.recent_projects.is_empty());
    assert!(config.check_for_updates);
}

#[test]
fn serialize_deserialize_roundtrip() {
    let config = SystemConfig {
        theme: "Nord".to_string(),
        open_last_project: false,
        last_project_path: Some(PathBuf::from("/tmp/test.regen2000proj")),
        sync_charset_view: true,
        entropy_threshold: 6.0,
        recent_projects: vec![
            PathBuf::from("/tmp/a.regen2000proj"),
            PathBuf::from("/tmp/b.regen2000proj"),
        ],
        check_for_updates: false,
        ..Default::default()
    };

    let json = serde_json::to_string_pretty(&config).unwrap();
    let deserialized: SystemConfig = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.theme, "Nord");
    assert!(!deserialized.open_last_project);
    assert_eq!(
        deserialized.last_project_path,
        Some(PathBuf::from("/tmp/test.regen2000proj"))
    );
    assert!(deserialized.sync_charset_view);
    assert!((deserialized.entropy_threshold - 6.0).abs() < f32::EPSILON);
    assert_eq!(deserialized.recent_projects.len(), 2);
    assert!(!deserialized.check_for_updates);
}

#[test]
fn missing_fields_use_defaults() {
    // Simulate an old config file that only has basic fields
    let json = r#"{
        "open_last_project": true,
        "last_project_path": null
    }"#;
    let config: SystemConfig = serde_json::from_str(json).unwrap();
    assert_eq!(config.theme, "Dracula");
    assert!(config.sync_blocks_view);
    assert!(config.sync_hex_dump);
    assert!(!config.sync_charset_view);
    assert!(!config.sync_sprites_view);
    assert!(!config.sync_bitmap_view);
    assert!((config.entropy_threshold - 7.5).abs() < f32::EPSILON);
    assert!(config.recent_projects.is_empty());
    assert!(config.check_for_updates);
}

#[test]
fn unknown_fields_are_ignored() {
    let json = r#"{
        "open_last_project": true,
        "last_project_path": null,
        "some_future_field": 42,
        "another_unknown": "hello"
    }"#;
    // Should not panic — serde ignores unknown fields by default
    let config: SystemConfig = serde_json::from_str(json).unwrap();
    assert!(config.open_last_project);
}

#[test]
fn config_path_override_is_not_serialized() {
    let config = SystemConfig {
        config_path_override: Some(PathBuf::from("/tmp/override.json")),
        ..Default::default()
    };

    let json = serde_json::to_string(&config).unwrap();
    assert!(
        !json.contains("config_path_override"),
        "config_path_override should be skipped in serialization"
    );

    // And deserialization should set it to None
    let deserialized: SystemConfig = serde_json::from_str(&json).unwrap();
    assert!(deserialized.config_path_override.is_none());
}

#[test]
fn save_and_load_with_override_path() {
    let dir = std::env::temp_dir().join("r2000_config_test");
    let _ = std::fs::create_dir_all(&dir);
    let config_path = dir.join("test_config.json");

    let config = SystemConfig {
        config_path_override: Some(config_path.clone()),
        theme: "Monokai".to_string(),
        open_last_project: false,
        ..Default::default()
    };

    let save_result = config.save();
    assert!(save_result.is_ok(), "Save failed: {:?}", save_result.err());

    // Read back and verify
    let data = std::fs::read_to_string(&config_path).unwrap();
    let loaded: SystemConfig = serde_json::from_str(&data).unwrap();
    assert_eq!(loaded.theme, "Monokai");
    assert!(!loaded.open_last_project);

    // Cleanup
    let _ = std::fs::remove_file(&config_path);
    let _ = std::fs::remove_dir(&dir);
}

#[test]
fn add_recent_project_deduplicates() {
    let mut config = SystemConfig::default();
    let path = std::env::temp_dir().join("dup_test.regen2000proj");

    config.add_recent_project(path.clone());
    config.add_recent_project(path.clone());
    config.add_recent_project(path.clone());

    // Should only appear once
    assert_eq!(config.recent_projects.len(), 1);
}

#[test]
fn add_recent_project_keeps_most_recent_first() {
    let mut config = SystemConfig::default();
    let path_a = std::env::temp_dir().join("a.regen2000proj");
    let path_b = std::env::temp_dir().join("b.regen2000proj");

    config.add_recent_project(path_a.clone());
    config.add_recent_project(path_b.clone());

    // Most recently added should be first
    assert_eq!(
        config.recent_projects[0],
        std::fs::canonicalize(&path_b).unwrap_or(path_b)
    );
}

#[test]
fn add_recent_project_truncates_at_20() {
    let mut config = SystemConfig::default();
    for i in 0..25 {
        let path = std::env::temp_dir().join(format!("proj_{i}.regen2000proj"));
        config.add_recent_project(path);
    }
    assert!(config.recent_projects.len() <= 20);
}

#[test]
fn remove_recent_project() {
    let mut config = SystemConfig::default();
    let path = std::env::temp_dir().join("remove_test.regen2000proj");
    config.add_recent_project(path.clone());
    assert!(!config.recent_projects.is_empty());

    config.remove_recent_project(&path);
    assert!(config.recent_projects.is_empty());
}

#[test]
fn corrupted_json_does_not_crash_load() {
    // SystemConfig::load() reads from the config directory,
    // so we test the JSON parsing directly
    let bad_json = "{ this is not valid json }}}}}";
    let result = serde_json::from_str::<SystemConfig>(bad_json);
    assert!(result.is_err());
}

#[test]
fn entropy_threshold_serialization() {
    let config = SystemConfig {
        entropy_threshold: 5.25,
        ..Default::default()
    };

    let json = serde_json::to_string(&config).unwrap();
    let loaded: SystemConfig = serde_json::from_str(&json).unwrap();
    assert!((loaded.entropy_threshold - 5.25).abs() < f32::EPSILON);
}
