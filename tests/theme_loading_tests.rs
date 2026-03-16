#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use regenerator2000_tui::theme::Theme;

#[test]
fn all_named_themes_load() {
    let names = Theme::all_names();
    assert!(!names.is_empty());
    for name in &names {
        let theme = Theme::from_name(name);
        assert_eq!(theme.name, *name);
    }
}

#[test]
fn from_name_unknown_falls_back_to_dark() {
    let theme = Theme::from_name("Totally Invalid Theme");
    assert_eq!(theme.name, "Solarized Dark");
}

#[test]
fn default_theme_is_dracula() {
    let theme = Theme::default();
    assert_eq!(theme.name, "Dracula");
}

#[test]
fn themes_have_distinct_backgrounds() {
    let names = Theme::all_names();
    let mut backgrounds = Vec::new();
    for name in &names {
        let theme = Theme::from_name(name);
        backgrounds.push(format!("{:?}", theme.background));
    }
    backgrounds.sort();
    backgrounds.dedup();
    assert!(backgrounds.len() >= 6);
}

#[test]
fn all_themes_bg_differs_from_fg() {
    for name in Theme::all_names() {
        let theme = Theme::from_name(name);
        let bg = format!("{:?}", theme.background);
        let fg = format!("{:?}", theme.foreground);
        assert_ne!(bg, fg, "Theme {name:?} has identical bg and fg");
    }
}

#[test]
fn solarized_dark() {
    assert_eq!(Theme::dark().name, "Solarized Dark");
}

#[test]
fn solarized_light() {
    assert_eq!(Theme::light().name, "Solarized Light");
}

#[test]
fn dracula() {
    assert_eq!(Theme::dracula().name, "Dracula");
}

#[test]
fn gruvbox_dark() {
    assert_eq!(Theme::gruvbox_dark().name, "Gruvbox Dark");
}

#[test]
fn gruvbox_light() {
    assert_eq!(Theme::gruvbox_light().name, "Gruvbox Light");
}

#[test]
fn monokai() {
    assert_eq!(Theme::monokai().name, "Monokai");
}

#[test]
fn nord() {
    assert_eq!(Theme::nord().name, "Nord");
}

#[test]
fn catppuccin_mocha() {
    assert_eq!(Theme::catppuccin_mocha().name, "Catppuccin Mocha");
}

#[test]
fn catppuccin_latte() {
    assert_eq!(Theme::catppuccin_latte().name, "Catppuccin Latte");
}
