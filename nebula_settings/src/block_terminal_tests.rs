use super::*;

#[test]
fn command_blocks_are_opt_in_and_round_trip_without_losing_other_preferences() {
    for value in ["", "block_terminal=invalid", "block_terminal=0"] {
        assert!(!RuntimeSettings::from_raw(&RawSettings::from_text(value)).block_terminal);
    }
    let saved = apply_updates("cursor_blink=0\nprivate=keep\n", &[("block_terminal", "1".into())]);
    let settings = RuntimeSettings::from_raw(&RawSettings::from_text(&saved));
    assert!(settings.block_terminal);
    assert_eq!(settings.cursor_blink, Some(false));
    assert!(saved.contains("private=keep"));
}
