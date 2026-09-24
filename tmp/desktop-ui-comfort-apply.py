from pathlib import Path
import re
import sys

ROOT = Path('nebula_app/src/gpui_shell')

def replace(text, old, new, count=1):
    actual = text.count(old)
    if actual != count:
        raise RuntimeError(f'Expected {count} matches, found {actual}: {old!r}')
    return text.replace(old, new)

def edit(name, transform):
    path = ROOT / name
    old = path.read_text()
    path.write_text(transform(old))

def widgets(text):
    text = replace(text, 'App, ClickEvent, ElementId, IntoElement,', 'App, ClickEvent, ElementId, InteractiveElement as _, IntoElement,')
    marker = '/// 设置行开关。组件库 `Switch` 的转发壳'
    helpers = '''/// Desktop form controls keep their padding even with a small UI font.
pub(crate) fn settings_control_height(cx: &App) -> gpui::Pixels {
    px(f32::from(cx.theme().font_size).max(16.0) * 2.0)
}

/// Toolbar glyphs and their hover/hit surfaces have independent logical sizes.
pub(crate) fn toolbar_button(id: impl Into<ElementId>, icon: impl Into<Icon>) -> Button {
    Button::new(id).icon(Icon::new(icon).size(px(18.0))).ghost().size(px(32.0))
}

#[cfg(all(test, feature = "gpui-test-support"))]
#[path = "widgets_tests.rs"]
mod tests;

'''
    text = replace(text, marker, helpers + marker)
    old = '''impl RenderOnce for NebulaButton {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let button = Button::new(ElementId::Name(format!("nebula-btn-{}", self.key).into()))
            .label(self.label)
            .disabled(self.disabled);'''
    new = '''impl RenderOnce for NebulaButton {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let key = self.key.clone();
        let button = Button::new(ElementId::Name(format!("nebula-btn-{}", self.key).into()))
            .debug_selector(move || format!("nebula-btn-{key}"))
            .h(settings_control_height(cx))
            .px(px(12.0))
            .label(self.label)
            .disabled(self.disabled);'''
    return replace(text, old, new)

edit('widgets.rs', widgets)
(ROOT / 'widgets_tests.rs').write_text(Path(sys.argv[1]).read_text())

def settings(text):
    text = replace(text, 'use crate::gpui_shell::widgets::NebulaButton;', 'use crate::gpui_shell::widgets::{NebulaButton, settings_control_height};')
    text = replace(text, '.children(select.map(|state| Select::new(&state)))', '.children(select.map(|state| Select::new(&state).h(settings_control_height(cx))))')
    return replace(text, '.child(Select::new(&self.shell_select)),', '.child(Select::new(&self.shell_select).h(settings_control_height(cx))),')

edit('settings_pane.rs', settings)

def segmented(text):
    text = replace(text, '            .small()\n', '', 2)
    text = replace(text, '.h(px(28.0))', '.h(settings_control_height(cx))')
    return replace(text, '.rounded(px(14.0))', '.rounded(px(6.0))')

edit('settings_pane/segmented.rs', segmented)

def design(text):
    text = replace(text, '.size(px(20.0))', '.size(px(32.0))')
    text = replace(text, '.child(Icon::new(IconName::Undo2).xsmall())', '.child(Icon::new(IconName::Undo2).size(px(16.0)))')
    return replace(text, '.size(px(22.0))', '.size(px(32.0))')

edit('settings_pane/design.rs', design)

def toolbar(text, ids, sidebar=False):
    text, count = re.subn(r'(?m)^use super::\*;$', 'use super::*;\nuse crate::gpui_shell::widgets::toolbar_button;', text)
    if count != 1:
        raise RuntimeError(f'Expected one module import, found {count}')
    for button_id in ids:
        pattern = rf'Button::new\("{re.escape(button_id)}"\)\s*\.icon\((.*?)\)\s*\.ghost\(\)'
        text, count = re.subn(pattern, lambda match: f'toolbar_button("{button_id}", {match.group(1).strip()})', text, flags=re.S)
        if count != 1:
            raise RuntimeError(f'{button_id}: expected one toolbar button, found {count}')
    if sidebar:
        start = text.index('    pub(super) fn render_sidebar_title_bar(')
        end = text.index('    fn render_collapsed_tab_title(', start)
        section = text[start:end]
        section = replace(section, '// 旧壳两枚 32px 命中块之间固定留 8px；默认 Button 正好是\n                    // 32px，`.small()` 会把热区缩成 24px。', '// Keep toolbar gaps independent of the UI font/rem size.')
        section = section.replace('.gap_2()', '.gap(px(8.0))')
        text = text[:start] + section + text[end:]
    return text

edit('workspace/sidebar.rs', lambda text: toolbar(text, ['toggle-sidebar', 'open-settings', 'toggle-command-manager'], True))
edit('workspace/top_tabs.rs', lambda text: toolbar(text, ['top-toggle-command-manager']))
edit('workspace/details_panel.rs', lambda text: toolbar(text, ['toggle-right-sidebar']))
