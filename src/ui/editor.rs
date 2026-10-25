use super::*;

impl App {
    pub(super) fn render(&mut self, ctx: &egui::Context) -> Rect {
        self.top_bar(ctx);
        self.status_bar(ctx);
        self.navigation(ctx);
        let rect = egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(WHITE).inner_margin(0.))
            .show(ctx, |ui| self.workspace(ui))
            .response
            .rect;
        self.dialogs(ctx);
        rect
    }

    pub(super) fn tab_strip(&mut self, ui: &mut egui::Ui) {
        let r = row(ui, 40.);
        ui.painter().rect_filled(r, 0., BG);
        hline(ui, r.left(), r.right(), r.bottom() - 1.);
        let tabs_width = (self.tabs.len() as f32 * 192.).min((r.width() - 44.).max(1.));
        let plus = Rect::from_min_size(
            pos2(r.left() + tabs_width + 4., r.top() + 4.),
            vec2(32., 32.),
        );
        let scroll_rect = Rect::from_min_max(r.min, pos2(plus.left() - 4., r.bottom()));
        let mut strip = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("request_tabs")
                .max_rect(scroll_rect),
        );
        let mut activate = None;
        let mut action = None;
        egui::ScrollArea::horizontal()
            .id_salt("tab_scroll")
            .auto_shrink([false, false])
            .show(&mut strip, |ui| {
                ui.spacing_mut().item_spacing = Vec2::ZERO;
                ui.horizontal(|ui| {
                    for (i, tab) in self.tabs.iter().enumerate() {
                        let (r, _) = ui.allocate_exact_size(vec2(192., 39.), Sense::hover());
                        let selected = self.active == i;
                        let mut hit = ui
                            .interact(r, ui.id().with(tab.id), Sense::click())
                            .on_hover_cursor(egui::CursorIcon::PointingHand);
                        let activate_tab = hit.clicked();
                        if selected {
                            ui.painter().rect_filled(r, 0., WHITE);
                        } else if hit.hovered() {
                            ui.painter().rect_filled(r, 0., HOVER);
                        }
                        if selected {
                            ui.painter().line_segment(
                                [r.left_bottom(), r.right_bottom()],
                                Stroke::new(2., ACCENT),
                            );
                        }
                        vline(ui, r.right(), r.top() + 10., r.bottom() - 10.);
                        label(
                            ui,
                            Rect::from_min_size(r.min + vec2(12., 0.), vec2(44., 39.)),
                            &tab.request.method,
                            9.,
                            method_color(&tab.request.method),
                        );
                        label(
                            ui,
                            Rect::from_min_max(r.min + vec2(57., 0.), r.max - vec2(32., 0.)),
                            &tab.request.name,
                            11.,
                            if selected { TEXT } else { MUTED },
                        );
                        if tab.job.is_some() {
                            ui.place(
                                Rect::from_center_size(
                                    pos2(r.right() - 17., r.center().y),
                                    vec2(14., 14.),
                                ),
                                egui::Spinner::new().size(12.),
                            );
                        } else if ui.rect_contains_pointer(r) {
                            let close = icon_button(
                                ui,
                                Rect::from_min_size(
                                    pos2(r.right() - 28., r.top() + 7.),
                                    vec2(24., 24.),
                                ),
                                ("close", tab.id),
                                Icon::Close,
                                "关闭请求 (Ctrl W)",
                            );
                            if close.clicked() {
                                action = Some(TabAction::Close(vec![tab.id]));
                            }
                            hit = hit.union(close);
                        } else if tab.dirty() {
                            ui.painter().circle_filled(
                                pos2(r.right() - 17., r.center().y),
                                3.,
                                CYAN_DARK,
                            );
                        }
                        if activate_tab {
                            activate = Some(i);
                        }
                        hit.context_menu(|ui| {
                            ui.set_min_width(180.);
                            if menu_item(ui, "新建请求标签") {
                                action = Some(TabAction::New);
                            }
                            if menu_item(ui, "保存请求") {
                                action = Some(TabAction::Save(tab.id));
                            }
                            if menu_item(ui, "复制标签") {
                                action = Some(TabAction::Duplicate(tab.id));
                            }
                            ui.separator();
                            if menu_item(ui, "关闭标签") {
                                action = Some(TabAction::Close(vec![tab.id]));
                            }
                            if ui
                                .add_enabled(self.tabs.len() > 1, egui::Button::new("关闭其他标签"))
                                .clicked()
                            {
                                action = Some(TabAction::Close(
                                    self.tabs
                                        .iter()
                                        .filter(|t| t.id != tab.id)
                                        .map(|t| t.id)
                                        .collect(),
                                ));
                                ui.close();
                            }
                            if ui
                                .add_enabled(
                                    i + 1 < self.tabs.len(),
                                    egui::Button::new("关闭右侧标签"),
                                )
                                .clicked()
                            {
                                action = Some(TabAction::Close(
                                    self.tabs[i + 1..].iter().map(|t| t.id).collect(),
                                ));
                                ui.close();
                            }
                            if menu_item(ui, "关闭所有标签") {
                                action = Some(TabAction::Close(
                                    self.tabs.iter().map(|t| t.id).collect(),
                                ));
                            }
                        });
                        if selected && self.reveal_active_tab {
                            hit.scroll_to_me(Some(Align::Center));
                            self.reveal_active_tab = false;
                        }
                    }
                });
            });
        if icon_button(ui, plus, "add_tab", Icon::Plus, "新建请求 (Ctrl T)").clicked() {
            self.new_tab(Request::default());
        }
        let blank = Rect::from_min_max(pos2(plus.right() + 4., r.top()), r.max);
        if blank.is_positive() {
            ui.interact(blank, ui.id().with("tab_strip_menu"), Sense::click())
                .context_menu(|ui| {
                    ui.set_min_width(180.);
                    if menu_item(ui, "新建请求标签") {
                        action = Some(TabAction::New);
                    }
                    if menu_item(ui, "关闭所有标签") {
                        action = Some(TabAction::Close(self.tabs.iter().map(|t| t.id).collect()));
                    }
                });
        }
        if let Some(i) = activate {
            self.active = i;
        }
        if let Some(action) = action {
            match action {
                TabAction::New => self.new_tab(Request::default()),
                TabAction::Save(id) => self.begin_save_tab(id),
                TabAction::Duplicate(id) => {
                    if let Some(tab) = self.tabs.iter().find(|t| t.id == id) {
                        self.duplicate_request(tab.request.clone(), false);
                    }
                }
                TabAction::Close(ids) => self.request_close_tabs(ids),
            }
        }
    }

    pub(super) fn workspace(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing = Vec2::ZERO;
        self.tab_strip(ui);
        let header = row(ui, 48.);
        icon(ui, header.min + vec2(25., 24.), Icon::Code, MUTED);
        label(
            ui,
            Rect::from_min_max(
                header.min + vec2(46., 0.),
                pos2(header.right() - 368., header.bottom()),
            ),
            &self.tabs[self.active].request.name,
            13.,
            TEXT,
        );
        let environment_name = self
            .environments
            .iter()
            .find(|env| env.id == self.selected_environment)
            .map(|env| env.name.as_str())
            .unwrap_or("无环境");
        let mut environment_ui = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("environment_selector")
                .max_rect(Rect::from_min_size(
                    pos2(header.right() - 352., header.top() + 9.),
                    vec2(172., 30.),
                )),
        );
        egui::ComboBox::from_id_salt("selected_environment")
            .width(162.)
            .selected_text(environment_name)
            .show_ui(&mut environment_ui, |ui| {
                ui.selectable_value(&mut self.selected_environment, String::new(), "无环境");
                for env in &self.environments {
                    ui.selectable_value(&mut self.selected_environment, env.id.clone(), &env.name);
                }
            });
        if text_button(
            ui,
            Rect::from_min_size(
                pos2(header.right() - 174., header.top() + 9.),
                vec2(52., 30.),
            ),
            "edit_variables",
            "变量",
        )
        .clicked()
        {
            self.show_variables = true;
        }
        let save_rect = Rect::from_min_size(
            pos2(header.right() - 112., header.top() + 9.),
            vec2(96., 30.),
        );
        let save = ui
            .interact(save_rect, ui.id().with("save_request"), Sense::click())
            .on_hover_text("保存请求 (Ctrl S)")
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        ui.painter()
            .rect_filled(save_rect, 4., if save.hovered() { HOVER } else { BG });
        icon(ui, save_rect.min + vec2(18., 15.), Icon::Save, MUTED);
        label(
            ui,
            Rect::from_min_max(save_rect.min + vec2(36., 0.), save_rect.max),
            "保存",
            12.,
            TEXT,
        );
        if save.clicked() {
            self.begin_save();
        }
        if let Some(source) = &self.tabs[self.active].request.source {
            let breadcrumb = row(ui, 26.);
            let mut text = source.path.join(" / ");
            if !source.warnings.is_empty() {
                text.push_str("  ·  ");
                text.push_str(&source.warnings.join("；"));
            }
            label(ui, breadcrumb.shrink2(vec2(16., 0.)), &text, 11., MUTED);
            ui.interact(breadcrumb, ui.id().with("import_details"), Sense::hover())
                .on_hover_text(text);
        }
        let url_row = row(ui, 54.);
        let input = Rect::from_min_max(url_row.min + vec2(16., 3.), url_row.max - vec2(130., 9.));
        ui.painter().rect_filled(input, 4., WHITE);
        ui.painter()
            .rect_stroke(input, 4., Stroke::new(1., BORDER), StrokeKind::Inside);
        let tab = &mut self.tabs[self.active];
        let method_rect = Rect::from_min_max(input.min, pos2(input.left() + 110., input.bottom()));
        method_picker(
            ui,
            method_rect,
            &mut tab.request.method,
            &mut tab.custom_method,
        );
        vline(
            ui,
            method_rect.right(),
            input.top() + 9.,
            input.bottom() - 9.,
        );
        let url_rect = Rect::from_min_max(pos2(method_rect.right() + 1., input.top()), input.max);
        let edit = field(
            ui,
            url_rect,
            ("url", tab.id),
            &mut tab.request.url,
            "输入 URL 或粘贴文本",
        );
        if self.focus_url {
            edit.request_focus();
            self.focus_url = false;
        }
        if edit.has_focus() {
            ui.painter()
                .rect_stroke(input, 4., Stroke::new(1., ACCENT), StrokeKind::Inside);
        }
        if edit.changed() {
            tab.request.read_url_params();
        }
        let send_rect =
            Rect::from_min_size(pos2(url_row.right() - 122., input.top()), vec2(106., 42.));
        let busy = tab.job.is_some();
        let send = ui
            .interact(send_rect, ui.id().with("send"), Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text(if busy {
                "正在发送请求"
            } else {
                "发送请求 (Ctrl Enter)"
            });
        ui.painter().rect_filled(
            send_rect,
            4.,
            if send.hovered() { ACCENT_HOVER } else { ACCENT },
        );
        ui.painter().text(
            send_rect.center(),
            Align2::CENTER_CENTER,
            if busy { "发送中…" } else { "发送" },
            FontId::proportional(14.),
            WHITE,
        );
        if send.clicked() && !busy {
            self.send();
        }
        let tabs = row(ui, 38.);
        let mut x = tabs.left() + 4.;
        let tab = &mut self.tabs[self.active];
        for (key, name, width) in [
            ("Params", "参数", 56.),
            ("Auth", "身份验证", 82.),
            ("Headers", "请求头", 82.),
            ("Body", "请求体", 68.),
            ("Scripts", "脚本", 56.),
            ("Settings", "设置", 56.),
        ] {
            let rect = Rect::from_min_size(pos2(x, tabs.top()), vec2(width, 38.));
            let count = if key == "Headers" {
                tab.request
                    .headers
                    .iter()
                    .filter(|p| p.enabled && !p.key.is_empty())
                    .count()
            } else {
                0
            };
            let display = if count > 0 {
                format!("{name} ({count})")
            } else {
                name.into()
            };
            if tab_button(ui, rect, ("editor_tab", key), &display, tab.editor == key).clicked() {
                tab.editor = key.into();
            }
            if (key == "Body" && !tab.request.body.is_empty())
                || (key == "Auth"
                    && !effective_auth(&tab.request, &self.collection_auth)
                        .kind
                        .is_empty()
                    && effective_auth(&tab.request, &self.collection_auth).kind != "No Auth")
            {
                ui.painter()
                    .circle_filled(pos2(rect.right() - 7., rect.center().y), 2.5, GREEN);
            }
            x += width;
        }
        hline(ui, tabs.left(), tabs.right(), tabs.bottom());
        let available = ui.available_height();
        let max_editor = (available - 180.).max(110.);
        self.editor_height = self.editor_height.clamp(110., max_editor.max(110.));
        let editor_rect = row(ui, self.editor_height);
        let mut editor = ui.new_child(
            egui::UiBuilder::new()
                .id_salt(("editor", tab.id))
                .max_rect(editor_rect.shrink2(vec2(16., 0.))),
        );
        let mut edit_collection_auth = None;
        match tab.editor.as_str() {
            "Params" => {
                if pairs_ui(
                    &mut editor,
                    "params",
                    "查询参数",
                    &mut tab.request.params,
                    &mut tab.params_table,
                ) {
                    tab.request.write_url_params();
                }
            }
            "Headers" => {
                pairs_ui(
                    &mut editor,
                    "headers",
                    "请求头",
                    &mut tab.request.headers,
                    &mut tab.headers_table,
                );
            }
            "Body" => body_editor(&mut editor, &mut tab.request, &mut tab.form_table),
            "Auth" => {
                egui::ScrollArea::vertical()
                    .id_salt("request_auth_scroll")
                    .show(&mut editor, |ui| {
                        let previous_auth = tab.request.auth.clone();
                        auth_editor(ui, &mut tab.request.auth, true);
                        if previous_auth.kind != "Inherit" && tab.request.auth.kind == "Inherit" {
                            let auth = effective_auth(&tab.request, &self.collection_auth);
                            tab.request.auth = previous_auth;
                            tab.request.use_collection_auth(&auth);
                            tab.sync_auth_tables();
                        }
                        let collection = FolderTarget::collection(&tab.request);
                        ui.add_space(12.);
                        if tab.request.auth.kind == "Inherit" {
                            let auth = effective_auth(&tab.request, &self.collection_auth);
                            ui.label(
                                RichText::new(format!(
                                    "当前集合：{} · {}",
                                    collection.path[0],
                                    if auth.kind.is_empty() {
                                        "No Auth"
                                    } else {
                                        &auth.kind
                                    }
                                ))
                                .color(MUTED),
                            );
                        }
                        if ui.link("编辑集合身份验证").clicked() {
                            edit_collection_auth = Some(collection);
                        }
                    });
            }
            "Scripts" => {
                editor.add_space(20.);
                editor.label(RichText::new("请求脚本").size(13.).color(TEXT));
                editor.add_space(8.);
                editor.label(
                    RichText::new("脚本执行器尚未接入，当前请求不会执行前置脚本或测试脚本。")
                        .size(12.)
                        .color(MUTED),
                );
                if let Some(source) = &tab.request.source {
                    egui::ScrollArea::both().show(&mut editor, |ui| {
                        ui.label(
                            RichText::new(pretty_json(&source.original_json))
                                .monospace()
                                .size(11.),
                        );
                    });
                }
            }
            _ => {
                editor.add_space(20.);
                editor.label(RichText::new("请求设置").size(13.).color(TEXT));
                editor.add_space(12.);
                editor.label(RichText::new("请求超时：120 秒").color(MUTED));
                editor.label(RichText::new("自动跟随重定向 · 自动解压响应").color(MUTED));
            }
        }
        if let Some(target) = edit_collection_auth {
            self.begin_collection_auth(target);
        }
        let divider = row(ui, 6.);
        hline(ui, divider.left(), divider.right(), divider.center().y);
        let split = ui
            .interact(
                divider,
                ui.id().with("split_request_response"),
                Sense::drag(),
            )
            .on_hover_cursor(egui::CursorIcon::ResizeVertical);
        if split.hovered() || split.dragged() {
            ui.painter().rect_filled(divider, 0., CYAN);
        }
        if split.dragged() {
            self.editor_height =
                (self.editor_height + split.drag_delta().y).clamp(110., max_editor.max(110.));
        }
        self.response_ui(ui);
    }

    pub(super) fn response_ui(&mut self, ui: &mut egui::Ui) {
        let tab = &mut self.tabs[self.active];
        let header = row(ui, 36.);
        label(
            ui,
            Rect::from_min_size(header.min + vec2(16., 0.), vec2(80., 36.)),
            "响应",
            12.,
            TEXT,
        );
        if let Some(reply) = &tab.response {
            let info = format!(
                "{} {}    ·    {} ms    ·    {}",
                reply.status,
                reply.reason,
                reply.elapsed,
                format_size(reply.size)
            );
            let color = if reply.status < 400 { GREEN } else { DANGER };
            let galley = ui
                .painter()
                .layout_no_wrap(info, FontId::proportional(12.), color);
            ui.painter().galley(
                pos2(
                    header.right() - galley.size().x - 16.,
                    header.center().y - galley.size().y / 2.,
                ),
                galley,
                color,
            );
        }
        if tab.response.is_none() && tab.error.is_none() {
            let empty = ui.available_rect_before_wrap();
            if tab.job.is_some() {
                ui.place(
                    Rect::from_center_size(empty.center() - vec2(0., 18.), vec2(30., 30.)),
                    egui::Spinner::new().size(28.),
                );
                ui.painter().text(
                    empty.center() + vec2(0., 25.),
                    Align2::CENTER_CENTER,
                    "正在发送请求…",
                    FontId::proportional(12.),
                    MUTED,
                );
            } else {
                let center = empty.center();
                let logo = Rect::from_center_size(center - vec2(0., 27.), vec2(52., 52.));
                ui.painter().image(
                    self.mascot.id(),
                    logo,
                    Rect::from_min_max(pos2(0., 0.), pos2(1., 1.)),
                    WHITE,
                );
                ui.painter().text(
                    center + vec2(0., 24.),
                    Align2::CENTER_CENTER,
                    "输入 URL，点击发送以获取响应",
                    FontId::proportional(12.),
                    MUTED,
                );
            }
            return;
        }
        if let Some(error) = &tab.error {
            let r = ui.available_rect_before_wrap().shrink2(vec2(24., 16.));
            let mut error_ui = ui.new_child(egui::UiBuilder::new().max_rect(r));
            error_ui.label(RichText::new("无法完成请求").size(16.).color(TEXT));
            error_ui.add_space(12.);
            error_ui.label(RichText::new(error).size(12.).color(DANGER));
            return;
        }
        let tabs = row(ui, 34.);
        let mut x = tabs.left() + 4.;
        for (key, title, width) in [
            ("Body", "响应体", 72.),
            ("Cookies", "Cookie", 70.),
            ("Headers", "响应头", 72.),
            ("Tests", "测试结果", 82.),
        ] {
            if tab_button(
                ui,
                Rect::from_min_size(pos2(x, tabs.top()), vec2(width, 34.)),
                ("response_tab", key),
                title,
                tab.response_tab == key,
            )
            .clicked()
            {
                tab.response_tab = key.into();
            }
            x += width;
        }
        hline(ui, tabs.left(), tabs.right(), tabs.bottom());
        let Some(reply) = &tab.response else {
            return;
        };
        let content = ui.available_rect_before_wrap().shrink2(vec2(16., 8.));
        let mut body = ui.new_child(
            egui::UiBuilder::new()
                .id_salt(("response", tab.id))
                .max_rect(content),
        );
        match tab.response_tab.as_str() {
            "Headers" => {
                egui::ScrollArea::both()
                    .auto_shrink([false, false])
                    .show(&mut body, |ui| {
                        for (k, v) in &reply.headers {
                            let r = row(ui, 32.);
                            label(
                                ui,
                                Rect::from_min_size(r.min + vec2(8., 0.), vec2(230., 32.)),
                                k,
                                12.,
                                TEXT,
                            );
                            label(
                                ui,
                                Rect::from_min_max(r.min + vec2(246., 0.), r.max),
                                v,
                                12.,
                                MUTED,
                            );
                            hline(ui, r.left(), r.right(), r.bottom());
                        }
                    });
            }
            "Cookies" => {
                let cookies: Vec<_> = reply
                    .headers
                    .iter()
                    .filter(|(k, _)| k.eq_ignore_ascii_case("set-cookie"))
                    .collect();
                if cookies.is_empty() {
                    body.label(RichText::new("此响应未设置 Cookie").size(12.).color(MUTED));
                } else {
                    for (_, v) in cookies {
                        body.label(RichText::new(v).monospace().size(12.));
                    }
                }
            }
            "Tests" => {
                body.label(
                    RichText::new("当前请求未执行测试脚本")
                        .size(12.)
                        .color(MUTED),
                );
            }
            _ => {
                let bar = row(&mut body, 28.);
                if tab_button(
                    &mut body,
                    Rect::from_min_size(bar.min, vec2(60., 28.)),
                    "pretty",
                    "美化",
                    !tab.raw_response,
                )
                .clicked()
                {
                    tab.raw_response = false;
                }
                if tab_button(
                    &mut body,
                    Rect::from_min_size(bar.min + vec2(60., 0.), vec2(60., 28.)),
                    "raw",
                    "原始",
                    tab.raw_response,
                )
                .clicked()
                {
                    tab.raw_response = true;
                }
                let copy_rect =
                    Rect::from_min_size(pos2(bar.right() - 76., bar.top()), vec2(76., 28.));
                if text_button(&mut body, copy_rect, "copy_response", "复制响应").clicked() {
                    body.ctx().copy_text(reply.body.clone());
                    self.notice = Some(("响应已复制".into(), Instant::now()));
                }
                let value = if tab.raw_response {
                    reply.body.clone()
                } else {
                    pretty_json(&reply.body)
                };
                let mut display = value.as_str();
                egui::ScrollArea::both()
                    .auto_shrink([false, false])
                    .show(&mut body, |ui| {
                        ui.add(
                            egui::TextEdit::multiline(&mut display)
                                .id_salt(("reply_text", tab.id))
                                .font(egui::TextStyle::Monospace)
                                .frame(false)
                                .desired_width(f32::INFINITY),
                        );
                    });
            }
        }
    }
}
