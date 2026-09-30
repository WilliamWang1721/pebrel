use nebula_settings::{RawSettings, RuntimeSettings, apply_updates};

#[test]
fn network_preferences_keep_old_defaults_and_round_trip() {
    let defaults = RuntimeSettings::from_raw(&RawSettings::from_text(""));
    assert_eq!(defaults.network_test_url, "http://example.com/");
    assert!(defaults.update_proxy);
    let text = apply_updates(
        "other=keep\n",
        &[
            ("network_test_url", "https://example.org:8443/health?probe=1".into()),
            ("update_proxy", "0".into()),
        ],
    );
    let settings = RuntimeSettings::from_raw(&RawSettings::from_text(&text));
    assert_eq!(settings.network_test_url, "https://example.org:8443/health?probe=1");
    assert!(!settings.update_proxy);
    assert!(text.contains("other=keep"));
    let defaults = RuntimeSettings::from_raw(&RawSettings::from_text(
        "network_test_url=\nupdate_proxy=invalid\n",
    ));
    assert_eq!(defaults.network_test_url, "http://example.com/");
    assert!(defaults.update_proxy);
}
