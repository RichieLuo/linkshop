use super::*;

impl App {
    #[cfg(target_os = "windows")]
    pub(super) fn top_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("app_header")
            .exact_height(window_chrome::TITLE_HEIGHT)
            .frame(
                egui::Frame::new()
                    .fill(window_chrome::title_fill(ctx))
                    .inner_margin(0.),
            )
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing.x = 0.;
                ui.horizontal(|ui| {
                    ui.set_height(window_chrome::TITLE_HEIGHT);
                    ui.add_space(window_chrome::leading_inset());
                    let brand = ui.allocate_ui_with_layout(
                        vec2(160., window_chrome::TITLE_HEIGHT),
                        egui::Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            ui.add_space(16.);
                            ui.add(
                                egui::Image::new(&self.mascot)
                                    .fit_to_exact_size(vec2(28., 28.))
                                    .alt_text("LinkShop 狐狸 Logo"),
                            );
                            ui.add_space(8.);
                            ui.label(
                                RichText::new("LinkShop")
                                    .strong()
                                    .size(18.)
                                    .color(egui::Color32::from_rgb(30, 40, 48)),
                            );
                        },
                    );
                    let drag = ui.interact(
                        brand.response.rect,
                        ui.id().with("title-brand-drag"),
                        Sense::click_and_drag(),
                    );
                    window_chrome::drag_response(&drag);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        window_chrome::controls(ui);
                        ui.add_space(16.);
                        ui.label(RichText::new("● 本地工作区").size(12.).color(MUTED));
                        window_chrome::drag_area(ui);
                    });
                });
            });
    }

    #[cfg(not(target_os = "windows"))]
    pub(super) fn top_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("app_header")
            .exact_height(48.)
            .frame(egui::Frame::new().fill(BG).inner_margin(0.))
            .show(ctx, |ui| {
                let r = row(ui, 48.);
                ui.place(
                    Rect::from_min_size(r.min + vec2(16., 11.), vec2(26., 26.)),
                    egui::Image::new(&self.mascot).fit_to_exact_size(vec2(26., 26.)),
                );
                label(
                    ui,
                    Rect::from_min_size(r.min + vec2(51., 0.), vec2(100., 48.)),
                    "LinkShop",
                    17.,
                    TEXT,
                );
                vline(ui, r.left() + 145., r.top() + 15., r.bottom() - 15.);
                label(
                    ui,
                    Rect::from_min_size(r.min + vec2(163., 0.), vec2(120., 48.)),
                    "API 工作台",
                    13.,
                    MUTED,
                );
                let dot = pos2(r.right() - 102., r.center().y);
                ui.painter().circle_filled(dot, 3., GREEN);
                label(
                    ui,
                    Rect::from_min_size(dot + vec2(10., -24.), vec2(83., 48.)),
                    "本地工作区",
                    12.,
                    MUTED,
                );
                hline(ui, r.left(), r.right(), r.bottom() - 1.);
            });
    }
    pub(super) fn status_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("status")
            .exact_height(24.)
            .frame(egui::Frame::new().fill(BG).inner_margin(0.))
            .show(ctx, |ui| {
                let r = row(ui, 24.);
                hline(ui, r.left(), r.right(), r.top());
                icon(ui, r.min + vec2(17., 12.), Icon::Collection, MUTED);
                label(
                    ui,
                    Rect::from_min_size(r.min + vec2(33., 0.), vec2(200., 24.)),
                    "所有更改均保存在此设备",
                    10.,
                    MUTED,
                );
                let message = self
                    .notice
                    .as_ref()
                    .filter(|(_, at)| at.elapsed() < Duration::from_secs(4))
                    .map(|(s, _)| s.as_str())
                    .unwrap_or("Ctrl Enter 发送    ·    Ctrl S 保存    ·    Ctrl T 新建请求");
                let galley =
                    ui.painter()
                        .layout_no_wrap(message.into(), FontId::proportional(10.), MUTED);
                ui.painter().galley(
                    pos2(
                        r.right() - galley.size().x - 16.,
                        r.center().y - galley.size().y / 2.,
                    ),
                    galley,
                    MUTED,
                );
            });
    }
    pub(super) fn navigation(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("workspace_sidebar_v3")
            .default_width(350.)
            .width_range(300.0..=480.)
            .resizable(true)
            .frame(egui::Frame::new().fill(BG).inner_margin(0.))
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing = Vec2::ZERO;
                let header = row(ui, 40.);
                icon(ui, header.min + vec2(22., 20.), Icon::Collection, TEXT);
                label(
                    ui,
                    Rect::from_min_size(
                        header.min + vec2(38., 0.),
                        vec2(header.width() - 164., 40.),
                    ),
                    "我的工作区",
                    12.,
                    TEXT,
                );
                if text_button(
                    ui,
                    Rect::from_min_size(
                        pos2(header.right() - 118., header.top() + 7.),
                        vec2(50., 26.),
                    ),
                    "import_data",
                    "导入",
                )
                .on_hover_text("导入集合、环境或数据文件夹")
                .clicked()
                {
                    self.show_import = true;
                }
                if text_button(
                    ui,
                    Rect::from_min_size(
                        pos2(header.right() - 62., header.top() + 7.),
                        vec2(50., 26.),
                    ),
                    "new_request",
                    "新建",
                )
                .on_hover_text("新建 HTTP 请求 (Ctrl T)")
                .clicked()
                {
                    self.new_tab(Request::default());
                }
                hline(ui, header.left(), header.right(), header.bottom());
                let content = ui.available_rect_before_wrap();
                let rail =
                    Rect::from_min_max(content.min, pos2(content.left() + 64., content.bottom()));
                vline(ui, rail.right(), rail.top(), rail.bottom());
                for (i, (key, title, kind)) in [
                    ("Collections", "集合", Icon::Collection),
                    ("History", "历史记录", Icon::History),
                ]
                .into_iter()
                .enumerate()
                {
                    let r = Rect::from_min_size(
                        rail.min + vec2(4., 8. + i as f32 * 62.),
                        vec2(56., 56.),
                    );
                    let hit = ui
                        .interact(r, ui.id().with(key), Sense::click())
                        .on_hover_cursor(egui::CursorIcon::PointingHand);
                    let selected = self.sidebar == key;
                    if selected || hit.hovered() {
                        ui.painter()
                            .rect_filled(r, 4., if selected { SELECTED } else { HOVER });
                    }
                    if selected {
                        ui.painter().rect_filled(
                            Rect::from_min_size(pos2(r.left(), r.top() + 10.), vec2(3., 36.)),
                            1.,
                            CYAN,
                        );
                    }
                    icon(
                        ui,
                        pos2(r.center().x, r.top() + 17.),
                        kind,
                        if selected { ACCENT } else { MUTED },
                    );
                    ui.painter().text(
                        pos2(r.center().x, r.top() + 39.),
                        Align2::CENTER_CENTER,
                        title,
                        FontId::proportional(10.),
                        if selected { ACCENT } else { MUTED },
                    );
                    if hit.clicked() {
                        self.sidebar = key.into();
                    }
                }
                let body = Rect::from_min_max(
                    pos2(rail.right() + 8., content.top()),
                    pos2(content.right() - 8., content.bottom()),
                );
                let mut side = ui.new_child(
                    egui::UiBuilder::new()
                        .id_salt("sidebar_content")
                        .max_rect(body),
                );
                side.spacing_mut().item_spacing = Vec2::ZERO;
                let search_row = row(&mut side, 48.);
                let plus_rect = Rect::from_min_size(
                    pos2(search_row.right() - 30., search_row.top() + 9.),
                    vec2(28., 30.),
                );
                if icon_button(&mut side, plus_rect, "plus", Icon::Plus, "新建请求").clicked() {
                    self.new_tab(Request::default());
                }
                let input = Rect::from_min_max(
                    search_row.min + vec2(4., 8.),
                    search_row.max - vec2(40., 8.),
                );
                side.painter().rect_filled(input, 5., WHITE);
                side.painter()
                    .rect_stroke(input, 5., Stroke::new(1., LINE), StrokeKind::Inside);
                icon(
                    &side,
                    pos2(input.left() + 13., input.center().y),
                    Icon::Search,
                    MUTED,
                );
                let search_empty = self.search.is_empty();
                let edit_rect = Rect::from_min_max(
                    input.min + vec2(21., 0.),
                    input.max - vec2(if search_empty { 48. } else { 23. }, 0.),
                );
                let search = field(
                    &mut side,
                    edit_rect,
                    "search_requests",
                    &mut self.search,
                    "搜索请求",
                )
                .on_hover_text("搜索当前集合或历史记录 (Ctrl+K)");
                if self.focus_search {
                    search.request_focus();
                    self.focus_search = false;
                }
                if search_empty && self.search.is_empty() {
                    label(
                        &side,
                        Rect::from_min_max(pos2(input.right() - 42., input.top()), input.max),
                        "Ctrl K",
                        10.,
                        MUTED,
                    );
                }
                if !self.search.is_empty()
                    && icon_button(
                        &mut side,
                        Rect::from_min_size(
                            pos2(input.right() - 28., input.top() + 2.),
                            vec2(26., 28.),
                        ),
                        "clear_search",
                        Icon::Close,
                        "清除搜索",
                    )
                    .clicked()
                {
                    self.search.clear();
                }
                let query = self.search.to_lowercase();
                let is_history = self.sidebar == "History";
                let entries: Vec<_> = if is_history {
                    self.history
                        .iter()
                        .filter(|h| matches_search(&h.request, &query))
                        .map(|h| (h.request.name.clone(), h.request.clone()))
                        .collect()
                } else {
                    self.saved
                        .iter()
                        .filter(|s| {
                            matches_search(&s.request, &query)
                                || s.label.to_lowercase().contains(&query)
                        })
                        .map(|s| (s.label.clone(), s.request.clone()))
                        .collect()
                };
                let folders = request_folders(&entries);
                if !is_history {
                    let heading = row(&mut side, 34.);
                    ellipsis_label(
                        &side,
                        Rect::from_min_size(heading.min + vec2(12., 0.), vec2(44., 34.)),
                        "集合",
                        strong_font(12.),
                        MUTED,
                    );
                    label(
                        &side,
                        Rect::from_min_max(
                            heading.min + vec2(54., 0.),
                            heading.max - vec2(32., 0.),
                        ),
                        &format!("{} 个请求", entries.len()),
                        11.,
                        MUTED,
                    );
                    if icon_button(
                        &mut side,
                        Rect::from_min_size(
                            pos2(heading.right() - 30., heading.top() + 3.),
                            vec2(28., 28.),
                        ),
                        "collapse_collections",
                        Icon::Collapse,
                        "折叠所有集合",
                    )
                    .clicked()
                    {
                        for folder in &folders {
                            folder.set_expanded(ctx, false);
                        }
                    }
                }
                let mut action = None;
                egui::ScrollArea::vertical()
                    .id_salt("sidebar_list")
                    .auto_shrink([false, false])
                    .show(&mut side, |ui| {
                        if is_history {
                            if !entries.is_empty() {
                                let r = row(ui, 28.);
                                label(ui, r.shrink2(vec2(8., 0.)), "最近的请求", 11., MUTED);
                            }
                            for (i, (name, request)) in entries.iter().enumerate() {
                                let hit = request_row(
                                    ui,
                                    i,
                                    name,
                                    request,
                                    &self.tabs[self.active].request,
                                );
                                request_context_menu(&hit, request, false, &mut action);
                                if hit.clicked() {
                                    action = Some(SidebarAction::Open(Box::new(request.clone())));
                                }
                            }
                        } else {
                            for folder in &folders {
                                folder.show(
                                    ui,
                                    &self.tabs[self.active].request,
                                    !query.is_empty(),
                                    &mut action,
                                );
                            }
                        }
                        if entries.is_empty() {
                            ui.add_space(64.);
                            ui.vertical_centered(|ui| {
                                let r = row(ui, 38.);
                                icon(
                                    ui,
                                    r.center(),
                                    if query.is_empty() {
                                        Icon::Folder
                                    } else {
                                        Icon::Search
                                    },
                                    PLACEHOLDER,
                                );
                                ui.label(
                                    RichText::new(if !query.is_empty() {
                                        "没有匹配的请求"
                                    } else if is_history {
                                        "还没有历史记录"
                                    } else {
                                        "在这里组织你的 API"
                                    })
                                    .size(12.)
                                    .color(TEXT),
                                );
                                ui.add_space(7.);
                                ui.label(
                                    RichText::new(if !query.is_empty() {
                                        "试试其他名称、方法或 URL"
                                    } else if is_history {
                                        "发送的请求会显示在这里"
                                    } else {
                                        "保存请求，随时继续调试"
                                    })
                                    .size(11.)
                                    .color(MUTED),
                                );
                            });
                        }
                    });
                if let Some(action) = action {
                    self.sidebar_action(action);
                }
                vline(ui, content.right(), header.top(), content.bottom());
            });
    }
}
