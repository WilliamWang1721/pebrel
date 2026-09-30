use super::routing::*;
use crate::{RawSettings, RuntimeSettings, apply_updates};

#[test]
fn old_preferences_preserve_focus_routing_and_ai_visibility() {
    let settings = RuntimeSettings::from_raw(&RawSettings::from_text("ai_toasts=0"));
    assert!(!settings.ai_toasts);
    let routing = settings.notification_routing;
    for category in NotificationCategory::ALL {
        assert!(!routing.delivery(category, true).system);
        assert!(routing.delivery(category, false).in_app);
        assert_eq!(
            routing.delivery(category, false).system,
            category != NotificationCategory::Application
        );
        assert_eq!(
            routing.delivery(category, true).in_app,
            matches!(category, NotificationCategory::Attention | NotificationCategory::Application)
        );
    }
}

#[test]
fn explicit_channels_apply_to_foreground_and_background_for_every_source() {
    for (value, expected) in [
        ("in_app", NotificationDelivery { in_app: true, system: false }),
        ("system", NotificationDelivery { in_app: false, system: true }),
        ("mixed", NotificationDelivery { in_app: true, system: true }),
    ] {
        let saved =
            apply_updates("unknown=keep\nai_toasts=0\n", &[("notification_mode", value.into())]);
        let settings = RuntimeSettings::from_raw(&RawSettings::from_text(&saved));
        assert!(!settings.ai_toasts);
        assert!(saved.contains("unknown=keep"));
        assert_eq!(settings.notification_routing.mode.settings_value(), value);
        for category in NotificationCategory::ALL {
            for visible in [true, false] {
                assert_eq!(settings.notification_routing.delivery(category, visible), expected);
            }
        }
    }
}

#[test]
fn custom_rules_round_trip_and_only_change_the_selected_source_and_visibility() {
    for category in NotificationCategory::ALL {
        for (index, key) in category.rule_keys().into_iter().enumerate() {
            for value in NotificationChannel::VALUES {
                let saved = apply_updates(
                    "notification_mode=custom\nunknown=keep\n",
                    &[(key, (*value).into())],
                );
                let routing = NotificationRouting::from_raw(&RawSettings::from_text(&saved));
                assert!(saved.contains("unknown=keep"));
                assert_eq!(routing.setting_value(key), Some(*value));
                assert_eq!(routing.rule(category, index == 0).settings_value(), *value);
                assert_eq!(routing.rule(category, index != 0), NotificationChannel::Automatic);
                for other in NotificationCategory::ALL {
                    if other != category {
                        assert_eq!(
                            routing.delivery(other, true),
                            NotificationRouting::default().delivery(other, true)
                        );
                        assert_eq!(
                            routing.delivery(other, false),
                            NotificationRouting::default().delivery(other, false)
                        );
                    }
                }
                if *value == "off" {
                    assert_eq!(
                        routing.delivery(category, index == 0),
                        NotificationDelivery::default()
                    );
                }
            }
        }
    }
}

#[test]
fn invalid_modes_and_rules_keep_existing_behavior() {
    assert_eq!(NotificationMode::from_settings("invalid"), None);
    assert_eq!(NotificationChannel::from_settings("custom"), None);
    for mode in NotificationMode::VALUES {
        assert_eq!(NotificationMode::from_settings(mode).unwrap().settings_value(), *mode);
    }
    let routing = NotificationRouting::from_raw(&RawSettings::from_text(
        "notification_mode=invalid\nnotification_attention_background=invalid\n",
    ));
    assert_eq!(routing, NotificationRouting::default());
}
