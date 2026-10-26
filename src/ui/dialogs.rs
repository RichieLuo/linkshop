use super::*;

impl App {
    pub(super) fn import_files(&mut self, paths: &[PathBuf]) {
        self.import_files_using(paths, || linkshop::postman::backup_profile().map(|_| ()));
    }

    fn import_files_using(
        &mut self,
        paths: &[PathBuf],
        backup: impl FnOnce() -> Result<(), String>,
    ) {
        let result = linkshop::postman::load(paths).and_then(|bundle| {
            backup()?;
            for original in &bundle.originals {
                if !self.postman_originals.contains(original) {
                    self.postman_originals.push(original.clone());
                }
            }
            Ok(linkshop::postman::merge(
                &mut self.saved,
                &mut self.environments,
                &mut self.globals,
                &mut self.collection_auth,
                bundle,
            ))
        });
        self.import_message = match result {
            Ok(report) => format!(
                "已导入 {} 个集合、{} 条请求、{} 个环境；跳过 {} 条重复请求。{}",
                report.collections,
                report.added + report.upgraded,
                report.environments,
                report.skipped,
                if report.warnings > 0 {
                    "部分请求含兼容性提示，请查看请求上方说明。"
                } else {
                    ""
                }
            ),
            Err(error) => format!("导入失败：{error}"),
        };
        self.show_import = true;
    }

    pub(super) fn poll_shell_open(&mut self, ctx: &egui::Context) {
        let ready = !self.show_save
            && !self.show_import
            && self.close_tab_id.is_none()
            && self.rename_target.is_none()
            && self.delete_target.is_none()
            && self.collection_auth_draft.is_none();
        let request = if ready {
            crate::shell_open::poll(ctx)
        } else {
            crate::shell_open::poll_when(ctx, false)
        };
        match request {
            Ok(Some(paths)) if !paths.is_empty() => self
                .open_shell_paths_using(&paths, || linkshop::postman::backup_profile().map(|_| ())),
            Err(error) => {
                self.import_message = format!("读取文件打开请求失败：{error}");
                self.show_import = true;
            }
            _ => {}
        }
    }

    fn open_shell_paths_using(
        &mut self,
        paths: &[PathBuf],
        backup: impl FnOnce() -> Result<(), String>,
    ) {
        if let Some(path) = paths.iter().find(|path| {
            !path.is_file()
                || !path
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
        }) {
            self.import_message = format!(
                "无法导入 {}：请选择已有的 Postman JSON 文件",
                path.display()
            );
            self.show_import = true;
            return;
        }
        self.import_files_using(paths, backup);
    }

    pub(super) fn dialogs(&mut self, ctx: &egui::Context) {
        if let Some(draft) = &mut self.collection_auth_draft {
            let mut save = false;
            let mut cancel = false;
            let count = self
                .saved
                .iter()
                .filter(|saved| draft.target.contains(&saved.request))
                .count();
            let modal = egui::Modal::new(egui::Id::new("collection_auth_dialog")).show(ctx, |ui| {
                ui.set_width(520.);
                ui.heading("集合身份验证");
                ui.add_space(8.);
                ui.label(RichText::new(&draft.target.path[0]).color(MUTED));
                egui::ScrollArea::vertical().max_height(360.).show(ui, |ui| {
                    auth_editor(ui, &mut draft.auth, false);
                    ui.add_space(16.);
                    ui.label("保存后，继承此集合的请求将在发送时使用最新配置。支持 {{变量名}}。");
                    ui.add_space(10.);
                    ui.checkbox(&mut draft.apply_to_existing, format!(
                        "将集合内已有的 {count} 条请求改为继承（含子文件夹）"
                    ));
                    if draft.apply_to_existing {
                        ui.label(RichText::new("同时停用已有的 Authorization 和同名 API Key 请求头/参数。独立身份验证将被替换。").color(MUTED));
                    } else {
                        ui.label(RichText::new("已有请求的独立身份验证保持原有配置；从集合添加的新请求默认继承。").color(MUTED));
                    }
                });
                ui.add_space(18.);
                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                    save = ui.add(egui::Button::new(RichText::new("保存").color(WHITE))
                        .fill(ACCENT).min_size(vec2(76., 32.))).clicked();
                    cancel = ui.button("取消").clicked();
                });
            });
            if save {
                self.save_collection_auth();
            } else if cancel || modal.should_close() {
                self.collection_auth_draft = None;
            }
        }
        if self.show_import {
            let mut visible = true;
            egui::Window::new("导入 Postman 数据")
                .id(egui::Id::new("import_window"))
                .open(&mut visible)
                .resizable(false)
                .default_width(560.)
                .show(ctx, |ui| {
                    ui.label(
                        "将 Collection / Environment JSON 拖入窗口，或输入文件、目录的完整路径。",
                    );
                    ui.add_space(10.);
                    ui.add_sized(
                        [540., 32.],
                        egui::TextEdit::singleline(&mut self.import_path)
                            .hint_text("C:\\路径\\collection.postman_collection.json"),
                    );
                    ui.add_space(10.);
                    if ui
                        .add_enabled(
                            !self.import_path.trim().is_empty(),
                            egui::Button::new("导入"),
                        )
                        .clicked()
                    {
                        self.import_files(&[PathBuf::from(
                            self.import_path.trim().trim_matches('"'),
                        )]);
                    }
                    if !self.import_message.is_empty() {
                        ui.add_space(10.);
                        ui.label(&self.import_message);
                    }
                    ui.add_space(8.);
                    ui.label(
                        RichText::new(
                            "保留集合层级、请求配置和原始数据。导入前自动备份本机工作区。",
                        )
                        .color(MUTED),
                    );
                });
            self.show_import = visible;
        }
        if self.show_variables {
            let mut visible = true;
            egui::Window::new("变量")
                .id(egui::Id::new("variables_window"))
                .open(&mut visible)
                .default_size([740., 390.])
                .show(ctx, |ui| {
                    let previous = self.variables_scope;
                    let scope_name = match self.variables_scope {
                        0 => "全局变量",
                        1 => "此请求变量（含集合变量）",
                        i => self
                            .environments
                            .get(i - 2)
                            .map(|e| e.name.as_str())
                            .unwrap_or("全局变量"),
                    };
                    egui::ComboBox::from_id_salt("variable_scope")
                        .width(260.)
                        .selected_text(scope_name)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.variables_scope, 0, "全局变量");
                            ui.selectable_value(
                                &mut self.variables_scope,
                                1,
                                "此请求变量（含集合变量）",
                            );
                            for (i, env) in self.environments.iter().enumerate() {
                                ui.selectable_value(&mut self.variables_scope, i + 2, &env.name);
                            }
                        });
                    if previous != self.variables_scope {
                        self.variables_table = TableState::default();
                    }
                    ui.add_space(6.);
                    ui.label(
                        RichText::new(
                            "发送时依次覆盖：全局 → 集合/请求 → 当前环境。使用 {{变量名}} 引用。",
                        )
                        .color(MUTED),
                    );
                    ui.add_space(10.);
                    let values = match self.variables_scope {
                        1 => &mut self.tabs[self.active].request.variables,
                        i if i >= 2 && i - 2 < self.environments.len() => {
                            &mut self.environments[i - 2].values
                        }
                        _ => &mut self.globals,
                    };
                    pairs_ui(ui, "variables", "变量", values, &mut self.variables_table);
                });
            self.show_variables = visible;
        }
        if self.show_save {
            egui::Modal::new(egui::Id::new("save_dialog")).show(ctx, |ui| {
                ui.set_width(400.);
                ui.heading("保存请求");
                ui.add_space(16.);
                ui.label("请求名称");
                ui.add_space(5.);
                ui.add_sized(
                    [400., 34.],
                    egui::TextEdit::singleline(&mut self.save_name).hint_text("给请求起一个名称"),
                );
                ui.add_space(8.);
                let destination = self
                    .tabs
                    .iter()
                    .find(|tab| tab.id == self.save_tab_id)
                    .and_then(|tab| tab.request.source.as_ref())
                    .map(|source| source.path.join(" / "))
                    .unwrap_or_else(|| "已保存的请求".into());
                ui.label(
                    RichText::new(format!("保存到：我的工作区 / {destination}"))
                        .size(12.)
                        .color(MUTED),
                );
                ui.add_space(18.);
                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                    if ui
                        .add_enabled(
                            !self.save_name.trim().is_empty(),
                            egui::Button::new(RichText::new("保存").color(WHITE))
                                .fill(ACCENT)
                                .min_size(vec2(76., 32.)),
                        )
                        .clicked()
                    {
                        self.save_request();
                    }
                    if ui.button("取消").clicked() {
                        self.show_save = false;
                        self.close_after_save = false;
                        self.pending_close.clear();
                    }
                });
            });
        }
        if let Some(id) = self.close_tab_id {
            let name = self
                .tabs
                .iter()
                .find(|t| t.id == id)
                .map(|t| t.request.name.clone())
                .unwrap_or_default();
            egui::Modal::new(egui::Id::new("close_dialog")).show(ctx, |ui| {
                ui.set_width(400.);
                ui.heading("保存更改？");
                ui.add_space(12.);
                ui.label(format!("“{name}”包含尚未保存的更改。"));
                ui.add_space(20.);
                ui.horizontal(|ui| {
                    if ui.button("取消").clicked() {
                        self.close_tab_id = None;
                        self.pending_close.clear();
                    }
                    if ui.button("不保存").clicked() {
                        self.close_tab(id);
                        self.advance_close_queue();
                    }
                    if ui
                        .add(egui::Button::new(RichText::new("保存").color(WHITE)).fill(ACCENT))
                        .clicked()
                    {
                        self.close_tab_id = None;
                        self.close_after_save = true;
                        self.begin_save_tab(id);
                    }
                });
            });
        }
        if let Some(target) = self.rename_target.clone() {
            egui::Modal::new(egui::Id::new("rename_collection_item")).show(ctx, |ui| {
                ui.set_width(400.);
                ui.heading("重命名");
                ui.add_space(12.);
                ui.label(format!("当前名称：{}", target.name()));
                ui.add_space(6.);
                ui.add_sized(
                    [400., 34.],
                    egui::TextEdit::singleline(&mut self.rename_name),
                );
                if !self.rename_error.is_empty() {
                    ui.colored_label(DANGER, &self.rename_error);
                }
                ui.add_space(16.);
                ui.horizontal(|ui| {
                    if ui.button("取消").clicked() {
                        self.rename_target = None;
                    }
                    if ui
                        .add_enabled(
                            !self.rename_name.trim().is_empty(),
                            egui::Button::new("保存"),
                        )
                        .clicked()
                    {
                        self.rename_collection_target();
                    }
                });
            });
        }
        if let Some(target) = self.delete_target.clone() {
            let count = self
                .saved
                .iter()
                .filter(|s| target.contains(&s.request))
                .count();
            egui::Modal::new(egui::Id::new("delete_collection_item")).show(ctx, |ui| {
                ui.set_width(400.);
                ui.heading("确认删除？");
                ui.add_space(12.);
                ui.label(format!(
                    "将删除“{}”及其中的 {count} 条请求。",
                    target.name()
                ));
                ui.label("已打开的请求将保留为草稿，可重新保存。");
                ui.add_space(16.);
                ui.horizontal(|ui| {
                    if ui.button("取消").clicked() {
                        self.delete_target = None;
                    }
                    if ui.button("删除").clicked() {
                        self.delete_collection_target();
                    }
                });
            });
        }
    }
}

#[cfg(test)]
mod file_open_tests;
