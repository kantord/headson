use headson::ColorMode;

#[test]
fn color_mode_on_forces_color() {
    assert!(ColorMode::On.effective(true));
    assert!(ColorMode::On.effective(false));
}

#[test]
fn color_mode_off_disables_color() {
    assert!(!ColorMode::Off.effective(true));
    assert!(!ColorMode::Off.effective(false));
}

#[test]
fn color_mode_auto_follows_terminal() {
    assert!(ColorMode::Auto.effective(true));
    assert!(!ColorMode::Auto.effective(false));
}
