use super::*;

impl NebulaWorkspace {
    pub(super) fn render_side_panel_slot(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        if !self.side_panel_anim_armed && !self.side_panel.open {
            return div().into_any_element();
        }
        let open = self.side_panel.open;
        let width = self.side_panel_width;
        // 路由每帧都做，而且必须在挑渲染分支之前：聚焦 pane 可能刚从本地切到
        // 远端（或反过来），这一帧就该画对。
        let remote = self.side_panel.open && self.route_remote_browser(window, cx);
        let panel = match self.side_panel.view {
            // "文件"视图画谁由聚焦 pane 的身份决定，不是一个用户要自己选的
            // 页签——用户想看的永远是"当前这台机器上的文件"。
            crate::display::side_panel::PanelView::Files if remote => self.render_remote_files(cx),
            crate::display::side_panel::PanelView::Files => self.render_file_tree(cx),
            crate::display::side_panel::PanelView::Git => self.render_git_tree(window, cx),
        };
        div()
            .h_full()
            .flex()
            .justify_end()
            .flex_shrink_0()
            .overflow_hidden()
            // 本地文件、SFTP 和 Git 共用这一层壳色，与左侧栏、卡缝融为一体。
            // 子视图只画内容；再次铺半透明底色会叠加 alpha，变成更亮的独立浮卡。
            .bg(cx.theme().background)
            .child(
                // 抽屉整体从右缘推进来，而不是原地被擦出来。旧壳
                // （side_panel.rs:1718）把 x 插值成
                // `rest_x + (1-eased) * (w + margin)`——整列内容在动；只动槽位宽度
                // 的话内容一动不动，只有裁剪窗口在变宽，那就是"擦除"的观感来源。
                // 槽位宽度仍然同步收放，正文（终端卡）才会跟着让位。
                //
                // 底部 8px 由槽位给（用户 08-26 裁定「文件树底部要留一段间距」，
                // 此前抽屉直插窗口底边）：写成抽屉自己的 margin 会和它的 `h_full`
                // 相加而溢出槽位、底部两角被 `overflow_hidden` 裁掉；写成父级
                // padding 则 `h_full` 按内容框解析，正好矮 8px。上边贴 chrome 下沿、
                // 右边贴窗口右缘不变，左边那条缝由终端卡的 `pr` 给。
                div()
                    .relative()
                    .h_full()
                    .flex_shrink_0()
                    .pb(px(crate::gpui_shell::theme::PaneCardStyle::current(cx).margin.bottom))
                    .child(panel)
                    .with_animation(
                        ("side-panel-push", open as usize),
                        Animation::new(Duration::from_millis(240)).with_easing(ease_out_quint()),
                        move |band, t| {
                            let progress = if open { t } else { 1.0 - t };
                            band.left(px(width * (1.0 - progress)))
                        },
                    ),
            )
            .with_animation(
                ("side-panel-slide", open as usize),
                Animation::new(Duration::from_millis(240)).with_easing(ease_out_quint()),
                move |slot, t| {
                    let progress = if open { t } else { 1.0 - t };
                    slot.w(px(width * progress))
                },
            )
            .into_any_element()
    }
}

impl NebulaWorkspace {
    pub(super) fn side_panel_resize_handle(
        &self,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        (self.side_panel.open && nebula_settings::RuntimeSettings::load().panel_resize).then(|| {
            div()
                .relative()
                .w_0()
                .h_full()
                .flex_shrink_0()
                .child(
                    div()
                        .id("side-panel-resize")
                        .absolute()
                        .top_0()
                        .bottom_0()
                        .left(px(-SIDEBAR_RESIZE_HANDLE_WIDTH * 0.5))
                        .w(px(SIDEBAR_RESIZE_HANDLE_WIDTH))
                        .cursor_col_resize()
                        .hover(|style| style.bg(cx.theme().border))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _, _, cx| {
                                cx.stop_propagation();
                                this.side_panel_resizing = true;
                                cx.notify();
                            }),
                        ),
                )
                .into_any_element()
        })
    }

    pub(super) fn side_panel_resize_overlay(
        &self,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        self.side_panel_resizing.then(|| {
            div()
                .absolute()
                .inset_0()
                .occlude()
                .cursor_col_resize()
                .on_mouse_move(cx.listener(|this, event: &gpui::MouseMoveEvent, window, cx| {
                    if event.pressed_button != Some(MouseButton::Left) {
                        this.side_panel_resizing = false;
                    } else {
                        this.side_panel_width = panel_width(
                            f32::from(window.viewport_size().width),
                            f32::from(event.position.x),
                        );
                    }
                    cx.notify();
                }))
                .on_mouse_up(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        this.side_panel_resizing = false;
                        cx.notify();
                    }),
                )
                .into_any_element()
        })
    }
}

fn panel_width(window_width: f32, pointer_x: f32) -> f32 {
    (window_width - pointer_x).clamp(170.0, (window_width * 0.5).clamp(170.0, 640.0))
}

#[cfg(test)]
mod tests {
    use super::panel_width;

    #[test]
    fn right_panel_tracks_pointer_and_reserves_room_for_terminal() {
        assert_eq!(panel_width(1000.0, 680.0), 320.0);
        assert_eq!(panel_width(1000.0, -20.0), 500.0);
        assert_eq!(panel_width(1000.0, 1100.0), 170.0);
        assert_eq!(panel_width(300.0, 0.0), 170.0);
    }
}
