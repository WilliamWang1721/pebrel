//! Evidence-only harness (throwaway branch, never merged): renders settings
//! sections with the real macOS Metal renderer and writes PNG screenshots.

use super::*;
use std::{borrow::Cow, path::Path, sync::Arc};

#[derive(Clone, Copy)]
enum Shot {
    Section(usize),
    Ssh,
    Backup,
    Agents,
    Mobile { paired: bool },
}

pub(crate) fn run(dir: &Path) {
    std::fs::create_dir_all(dir).unwrap();
    let mut cx = gpui::VisualTestAppContext::with_asset_source(
        gpui_platform::current_platform(false),
        Arc::new(crate::gpui_shell::assets::NebulaAssets),
    );
    cx.update(|cx| {
        cx.set_reduce_motion(true);
        cx.text_system()
            .add_fonts(vec![Cow::Borrowed(crate::font_install::REQUIRED_FONT_BYTES)])
            .unwrap();
        gpui_component::init(cx);
        let mut settings = crate::gpui_shell::config::Settings::load(ThemeName::Nord);
        settings.ui_language = crate::display::UiLanguage::ZhCn;
        gpui_component::set_locale(settings.ui_language.gpui_component_locale());
        cx.set_global(settings);
        crate::gpui_shell::theme::apply_chrome_theme(cx);
    });
    let shots = [
        ("interaction", Shot::Section(6)),
        ("network", Shot::Section(5)),
        ("ssh", Shot::Ssh),
        ("backup-wizard", Shot::Backup),
        ("agents", Shot::Agents),
        ("mobile-pairing", Shot::Mobile { paired: false }),
        ("mobile-paired", Shot::Mobile { paired: true }),
    ];
    for (label, width, height) in [("wide", 1600.0, 1000.0), ("narrow", 900.0, 1000.0)] {
        for (name, shot) in shots {
            capture(&mut cx, &dir.join(format!("{name}-{label}.png")), width, height, shot);
        }
    }
}

fn capture(cx: &mut gpui::VisualTestAppContext, path: &Path, width: f32, height: f32, shot: Shot) {
    let handle = cx
        .open_offscreen_window(gpui::size(px(width), px(height)), move |window, cx| {
            let view = cx.new(|cx| SettingsPane::new(window, cx));
            view.update(cx, |pane, cx| setup(pane, shot, window, cx));
            cx.new(|cx| gpui_component::Root::new(view, window, cx))
        })
        .unwrap();
    for _ in 0..4 {
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, window, cx| {
            window.refresh();
            window.draw(cx).clear(cx);
        })
        .unwrap();
    }
    let image = cx.capture_screenshot(handle.into()).unwrap();
    image.save(path).unwrap();
    eprintln!("[width-snapshot] wrote {}", path.display());
    cx.update_window(handle.into(), |_, window, _| window.remove_window()).unwrap();
    cx.run_until_parked();
}

fn setup(pane: &mut SettingsPane, shot: Shot, window: &mut Window, cx: &mut Context<SettingsPane>) {
    match shot {
        Shot::Section(section) => pane.active_section = section,
        Shot::Ssh => {
            pane.manage_launcher_ssh(String::new(), false, window, cx);
            pane.ssh_delete_confirm = None;
            let hosts = [
                ("kud@nas.example", "家里 NAS"),
                ("deploy@192.0.2.21", "生产 API"),
                ("bastion", "bastion"),
                ("kud@relay.example", "中转服务器"),
            ];
            pane.ssh_hosts = crate::gpui_shell::ssh_hosts::SshHostLists {
                saved: hosts.iter().map(|(host, _)| (*host).into()).collect(),
                pinned: vec![hosts[0].0.into()],
                ..Default::default()
            };
            for (host, label) in hosts {
                let mut profile = pane.ssh_hosts.profiles.for_destination(host);
                profile.label = Some(label.into());
                pane.ssh_hosts.profiles.upsert(profile);
            }
            pane.active_section = 4;
        },
        Shot::Backup => {
            pane.backup_remote = crate::backup_remote::BackupRemoteConfig::default();
            pane.active_section = 9;
        },
        Shot::Agents => {
            pane.evidence_agents();
            pane.active_section = 10;
        },
        Shot::Mobile { paired } => {
            pane.evidence_mobile(paired, window, cx);
            pane.active_section = MOBILE_SECTION;
        },
    }
}
