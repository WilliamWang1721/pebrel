use crate::RawSettings;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NotificationChannel {
    #[default]
    Automatic,
    InApp,
    System,
    Mixed,
    Off,
}

impl NotificationChannel {
    pub const VALUES: &'static [&'static str] = &["automatic", "in_app", "system", "mixed", "off"];

    pub fn from_settings(value: &str) -> Option<Self> {
        match value.trim() {
            "automatic" => Some(Self::Automatic),
            "in_app" => Some(Self::InApp),
            "system" => Some(Self::System),
            "mixed" => Some(Self::Mixed),
            "off" => Some(Self::Off),
            _ => None,
        }
    }

    pub fn settings_value(self) -> &'static str {
        Self::VALUES[self as usize]
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NotificationMode {
    #[default]
    Automatic,
    InApp,
    System,
    Mixed,
    Custom,
}

impl NotificationMode {
    pub const VALUES: &'static [&'static str] =
        &["automatic", "in_app", "system", "mixed", "custom"];

    pub fn from_settings(value: &str) -> Option<Self> {
        match value.trim() {
            "automatic" => Some(Self::Automatic),
            "in_app" => Some(Self::InApp),
            "system" => Some(Self::System),
            "mixed" => Some(Self::Mixed),
            "custom" => Some(Self::Custom),
            _ => None,
        }
    }

    pub fn settings_value(self) -> &'static str {
        Self::VALUES[self as usize]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotificationCategory {
    Completion,
    Failure,
    Attention,
    Terminal,
    Application,
}

impl NotificationCategory {
    pub const ALL: [Self; 5] =
        [Self::Completion, Self::Failure, Self::Attention, Self::Terminal, Self::Application];

    pub fn rule_keys(self) -> [&'static str; 2] {
        match self {
            Self::Completion => {
                ["notification_completion_foreground", "notification_completion_background"]
            },
            Self::Failure => ["notification_failure_foreground", "notification_failure_background"],
            Self::Attention => {
                ["notification_attention_foreground", "notification_attention_background"]
            },
            Self::Terminal => {
                ["notification_terminal_foreground", "notification_terminal_background"]
            },
            Self::Application => {
                ["notification_application_foreground", "notification_application_background"]
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NotificationDelivery {
    pub in_app: bool,
    pub system: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NotificationRouting {
    pub mode: NotificationMode,
    rules: [[NotificationChannel; 2]; NotificationCategory::ALL.len()],
}

impl NotificationRouting {
    pub fn from_raw(raw: &RawSettings) -> Self {
        let mut routing = Self {
            mode: raw
                .value("notification_mode")
                .and_then(NotificationMode::from_settings)
                .unwrap_or_default(),
            ..Self::default()
        };
        for category in NotificationCategory::ALL {
            for (index, key) in category.rule_keys().into_iter().enumerate() {
                routing.rules[category as usize][index] =
                    raw.value(key).and_then(NotificationChannel::from_settings).unwrap_or_default();
            }
        }
        routing
    }

    pub fn rule(self, category: NotificationCategory, visible: bool) -> NotificationChannel {
        self.rules[category as usize][usize::from(!visible)]
    }

    pub fn setting_value(self, key: &str) -> Option<&'static str> {
        if key == "notification_mode" {
            return Some(self.mode.settings_value());
        }
        for category in NotificationCategory::ALL {
            for (index, rule_key) in category.rule_keys().into_iter().enumerate() {
                if key == rule_key {
                    return Some(self.rules[category as usize][index].settings_value());
                }
            }
        }
        None
    }

    pub fn delivery(self, category: NotificationCategory, visible: bool) -> NotificationDelivery {
        let channel = match self.mode {
            NotificationMode::Automatic => NotificationChannel::Automatic,
            NotificationMode::InApp => NotificationChannel::InApp,
            NotificationMode::System => NotificationChannel::System,
            NotificationMode::Mixed => NotificationChannel::Mixed,
            NotificationMode::Custom => self.rule(category, visible),
        };
        match channel {
            NotificationChannel::Automatic => NotificationDelivery {
                in_app: !visible
                    || matches!(
                        category,
                        NotificationCategory::Attention | NotificationCategory::Application
                    ),
                system: !visible && category != NotificationCategory::Application,
            },
            NotificationChannel::InApp => NotificationDelivery { in_app: true, system: false },
            NotificationChannel::System => NotificationDelivery { in_app: false, system: true },
            NotificationChannel::Mixed => NotificationDelivery { in_app: true, system: true },
            NotificationChannel::Off => NotificationDelivery::default(),
        }
    }
}
