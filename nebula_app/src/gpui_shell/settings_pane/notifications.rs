use super::*;

#[cfg(all(test, feature = "gpui-test-support"))]
mod tests;

#[cfg(all(test, feature = "gpui-test-support"))]
mod native_tests;

impl SettingsPane {
    pub(super) fn set_notification_setting(
        &mut self,
        key: &'static str,
        value: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Err(error) = self.try_persist(&[(key, value.to_owned())], cx) {
            let previous = if key == "notification_duration" {
                self.runtime.notification_duration.settings_value()
            } else if key == "bell" {
                self.runtime.bell.settings_value()
            } else {
                self.runtime.notification_routing.setting_value(key).unwrap_or("automatic")
            };
            self.sync_select(key, previous, window, cx);
            let language = crate::gpui_shell::config::ui_language(cx);
            super::super::toast::feedback(
                window,
                cx,
                super::super::toast::ToastKind::Warning,
                language.format(
                    crate::i18n::Message::SettingsNotificationsSaveFailed,
                    &[("error", &error.to_string())],
                ),
            );
            cx.notify();
        } else {
            self.sync_select(key, value, window, cx);
        }
    }
}

impl SettingsPane {
    pub(super) fn section_notifications(&self, cx: &mut Context<Self>) -> gpui::Div {
        use crate::i18n::Message;
        use nebula_settings::{NotificationCategory, NotificationMode};
        let language = crate::gpui_shell::config::ui_language(cx);
        let general = self
            .group(language.text(Message::SettingsNotificationsGeneral), cx)
            .child(self.select_row(
                "notification_mode",
                language.text(Message::SettingsNotificationsMode),
                language.text(Message::SettingsNotificationsModeDescription),
                cx,
            ))
            .child(self.switch_row(
                "ai_toasts",
                language.text(Message::SettingsNotificationsAiMessages),
                help("ai_toasts", language),
                self.runtime.ai_toasts,
                cx,
            ))
            .child(self.select_row(
                "notification_duration",
                language.text(Message::SettingsNotificationsDuration),
                help("notification_duration", language),
                cx,
            ))
            .child(self.select_row(
                "bell",
                language.text(Message::SettingsNotificationsBell),
                help("bell", language),
                cx,
            ))
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(language.text(Message::SettingsNotificationsNativeHint)),
            )
            .child(
                NebulaButton::new("notification-test")
                    .label(language.text(Message::SettingsNotificationsTest))
                    .on_click(cx.listener(|_, _, window, cx| {
                        let language = crate::gpui_shell::config::ui_language(cx);
                        super::super::toast::toast(
                            window,
                            cx,
                            super::super::toast::ToastKind::Info,
                            language.text(Message::SettingsNotificationsTestMessage),
                        );
                    })),
            );
        let mut content = v_flex().w_full().gap(px(GROUP_GAP)).child(general);
        if self.runtime.notification_routing.mode == NotificationMode::Custom {
            content = content.child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(language.text(Message::SettingsNotificationsRulesDescription)),
            );
            for category in NotificationCategory::ALL {
                let title = match category {
                    NotificationCategory::Completion => Message::SettingsNotificationsCompletion,
                    NotificationCategory::Failure => Message::SettingsNotificationsFailure,
                    NotificationCategory::Attention => Message::SettingsNotificationsAttention,
                    NotificationCategory::Terminal => Message::SettingsNotificationsTerminal,
                    NotificationCategory::Application => Message::SettingsNotificationsApplication,
                };
                let [foreground, background] = category.rule_keys();
                content = content.child(
                    self.group(language.text(title), cx)
                        .child(self.select_row(
                            foreground,
                            language.text(Message::SettingsNotificationsForeground),
                            language.text(Message::SettingsNotificationsForegroundDescription),
                            cx,
                        ))
                        .child(self.select_row(
                            background,
                            language.text(Message::SettingsNotificationsBackground),
                            language.text(Message::SettingsNotificationsBackgroundDescription),
                            cx,
                        )),
                );
            }
        }
        content
    }
}
