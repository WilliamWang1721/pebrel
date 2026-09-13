use super::*;

impl NebulaWorkspace {
    pub(super) fn folder_tab_indices(&self) -> Vec<usize> {
        folder_indices(&self.tab_meta, self.selected_folder.as_deref())
    }

    fn choose_folder_group(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(workspace_ui_language().text(crate::i18n::Message::FoldersAdd).into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = picked.await;
            let _ = this.update_in(cx, |this, window, cx| match result {
                Ok(Ok(Some(paths))) => {
                    let Some(path) = paths.into_iter().next() else { return };
                    this.selected_folder = Some(path.to_string_lossy().into_owned());
                    this.tabs_scroll = 0;
                    this.add_terminal(window, cx);
                },
                Ok(Ok(None)) => {},
                _ => crate::gpui_shell::toast::toast(
                    window,
                    cx,
                    crate::gpui_shell::toast::ToastKind::Warning,
                    workspace_ui_language().text(crate::i18n::Message::FoldersPickFailed),
                ),
            });
        })
        .detach();
    }

    pub(super) fn render_folder_groups(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let language = workspace_ui_language();
        let folders = self
            .tab_meta
            .iter()
            .filter_map(|meta| meta.folder.clone())
            .collect::<std::collections::BTreeSet<_>>();
        let selected = self.selected_folder.clone();
        v_flex()
            .gap_1()
            .child(
                Button::new("add-folder-group")
                    .icon(IconName::Folder)
                    .ghost()
                    .small()
                    .label(language.text(crate::i18n::Message::FoldersAdd))
                    .on_click(
                        cx.listener(|this, _, window, cx| this.choose_folder_group(window, cx)),
                    ),
            )
            .when(!folders.is_empty(), |column| {
                column.child(
                    v_flex()
                        .id("folder-groups")
                        .max_h(px(160.0))
                        .overflow_y_scroll()
                        .gap_1()
                        .child(
                            Button::new("all-folder-tabs")
                                .ghost()
                                .small()
                                .selected(selected.is_none())
                                .label(language.text(crate::i18n::Message::FoldersAll))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.selected_folder = None;
                                    this.reveal_active_tab();
                                    cx.notify();
                                })),
                        )
                        .children(folders.into_iter().enumerate().map(|(index, folder)| {
                            let label = std::path::Path::new(&folder)
                                .file_name()
                                .map(|name| name.to_string_lossy().into_owned())
                                .unwrap_or_else(|| folder.clone());
                            Button::new(("folder-group", index))
                                .ghost()
                                .small()
                                .selected(selected.as_ref() == Some(&folder))
                                .label(label)
                                .tooltip(folder.clone())
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.selected_folder = Some(folder.clone());
                                    this.tabs_scroll = 0;
                                    if let Some(ix) = this.folder_tab_indices().first().copied() {
                                        this.activate_tab(ix, window, cx);
                                    }
                                    cx.notify();
                                }))
                        })),
                )
            })
            .into_any_element()
    }
}

fn folder_indices(meta: &[TabMeta], folder: Option<&str>) -> Vec<usize> {
    meta.iter()
        .enumerate()
        .filter(|(_, meta)| folder.is_none() || meta.folder.as_deref() == folder)
        .map(|(ix, _)| ix)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_keep_original_tab_indices_and_all_tabs_remain_accessible() {
        let tabs = vec![
            TabMeta::default(),
            TabMeta { folder: Some("/work/a".into()), ..TabMeta::default() },
            TabMeta { folder: Some("/work/b".into()), ..TabMeta::default() },
            TabMeta { folder: Some("/work/a".into()), ..TabMeta::default() },
        ];
        assert_eq!(folder_indices(&tabs, None), vec![0, 1, 2, 3]);
        assert_eq!(folder_indices(&tabs, Some("/work/a")), vec![1, 3]);
        assert!(folder_indices(&tabs, Some("/missing")).is_empty());
    }
}
