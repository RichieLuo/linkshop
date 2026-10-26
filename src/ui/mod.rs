mod actions;

use linkshop::model::*;
use widgets::*;

mod dialogs;
mod editor;
mod shell;
mod theme;
mod widgets;
#[cfg(target_os = "windows")]
mod window_chrome;
use theme::{install_fonts, set_style};

#[cfg(test)]
mod tests;
use eframe::egui::{
    self, Align, Align2, FontId, Key, Rect, RichText, Sense, Stroke, StrokeKind, Vec2, pos2, vec2,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::VecDeque,
    path::PathBuf,
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::{Duration, Instant},
};

#[derive(Clone, Serialize, Deserialize)]
struct SessionTab {
    request: Request,
    baseline: Request,
}

struct Tab {
    id: u64,
    request: Request,
    baseline: Request,
    editor: String,
    response_tab: String,
    response: Option<Reply>,
    error: Option<String>,
    job: Option<u64>,
    params_table: TableState,
    headers_table: TableState,
    custom_method: String,
    raw_response: bool,
    form_table: TableState,
}

#[derive(Clone)]
struct FolderTarget {
    key: Vec<String>,
    path: Vec<String>,
}

impl FolderTarget {
    fn collection(request: &Request) -> Self {
        Self {
            key: vec![request.collection_id().into()],
            path: vec![
                request
                    .source
                    .as_ref()
                    .and_then(|source| source.path.first())
                    .cloned()
                    .unwrap_or_else(|| "已保存的请求".into()),
            ],
        }
    }

    fn contains(&self, request: &Request) -> bool {
        if self.key[0] == "local-saved" {
            return request.source.as_ref().is_none_or(|s| s.path.is_empty());
        }
        request.source.as_ref().is_some_and(|source| {
            source.collection_id == self.key[0]
                && source
                    .path
                    .get(1..)
                    .unwrap_or_default()
                    .starts_with(&self.key[1..])
        })
    }
}

#[derive(Clone)]
enum CollectionTarget {
    Request(Box<Request>),
    Folder(FolderTarget),
}

impl CollectionTarget {
    fn name(&self) -> &str {
        match self {
            Self::Request(request) => &request.name,
            Self::Folder(folder) => folder.path.last().map_or("", String::as_str),
        }
    }

    fn contains(&self, request: &Request) -> bool {
        match self {
            Self::Request(target) => same_saved_request(target, request),
            Self::Folder(folder) => folder.contains(request),
        }
    }
}

enum SidebarAction {
    Open(Box<Request>),
    OpenNew(Box<Request>),
    Duplicate(Box<Request>),
    NewRequest(FolderTarget),
    OpenAll(FolderTarget),
    Auth(FolderTarget),
    Rename(CollectionTarget),
    Delete(CollectionTarget),
}

struct CollectionAuthDraft {
    target: FolderTarget,
    auth: Auth,
    apply_to_existing: bool,
}

enum TabAction {
    New,
    Save(u64),
    Duplicate(u64),
    Close(Vec<u64>),
}

impl Tab {
    fn new(id: u64, mut request: Request) -> Self {
        request.normalize();
        Self {
            id,
            baseline: request.clone(),
            request,
            editor: "Params".into(),
            response_tab: "Body".into(),
            response: None,
            error: None,
            job: None,
            params_table: TableState::default(),
            headers_table: TableState::default(),
            custom_method: String::new(),
            raw_response: false,
            form_table: TableState::default(),
        }
    }
    fn dirty(&self) -> bool {
        self.request != self.baseline
    }

    fn sync_auth_tables(&mut self) {
        if self.headers_table.bulk {
            self.headers_table.text = bulk_text(&self.request.headers);
        }
        if self.params_table.bulk {
            self.params_table.text = bulk_text(&self.request.params);
        }
    }
}

pub struct App {
    mascot: egui::TextureHandle,
    tabs: Vec<Tab>,
    active: usize,
    next_id: u64,
    next_job: u64,
    tx: Sender<WorkerEvent>,
    rx: Receiver<WorkerEvent>,
    saved: Vec<SavedRequest>,
    history: Vec<History>,
    search: String,
    sidebar: String,
    focus_search: bool,
    focus_url: bool,
    reveal_active_tab: bool,
    show_save: bool,
    save_tab_id: u64,
    save_name: String,
    close_tab_id: Option<u64>,
    pending_close: VecDeque<u64>,
    close_after_save: bool,
    rename_target: Option<CollectionTarget>,
    rename_name: String,
    rename_error: String,
    delete_target: Option<CollectionTarget>,
    collection_auth: Vec<CollectionAuth>,
    collection_auth_draft: Option<CollectionAuthDraft>,
    editor_height: f32,
    notice: Option<(String, Instant)>,
    environments: Vec<Environment>,
    globals: Vec<Pair>,
    import_originals: Vec<String>,
    selected_environment: String,
    show_variables: bool,
    variables_scope: usize,
    variables_table: TableState,
    show_import: bool,
    import_path: String,
    import_message: String,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        #[cfg(target_os = "windows")]
        window_chrome::configure_window(cc);
        let saved = cc
            .storage
            .and_then(|s| eframe::get_value(s, "saved"))
            .unwrap_or_default();
        let history = cc
            .storage
            .and_then(|s| eframe::get_value(s, "history"))
            .unwrap_or_default();
        let session: Vec<SessionTab> = cc
            .storage
            .and_then(|s| eframe::get_value(s, "open_tabs_v2"))
            .unwrap_or_default();
        let active: usize = cc
            .storage
            .and_then(|s| eframe::get_value(s, "active_tab_v2"))
            .unwrap_or(0);
        let mut app = Self::with_state(&cc.egui_ctx, saved, history, session, active);
        if let Some(storage) = cc.storage {
            app.environments = eframe::get_value(storage, "environments_v1").unwrap_or_default();
            app.globals = eframe::get_value(storage, "globals_v1").unwrap_or_default();
            app.import_originals = eframe::get_value(storage, linkshop::import::ORIGINALS_KEY)
                .or_else(|| eframe::get_value(storage, linkshop::import::LEGACY_ORIGINALS_KEY))
                .unwrap_or_default();
            app.selected_environment =
                eframe::get_value(storage, "selected_environment_v1").unwrap_or_default();
            app.collection_auth =
                eframe::get_value(storage, "collection_auth_v1").unwrap_or_default();
        }
        app
    }

    fn with_state(
        ctx: &egui::Context,
        saved: Vec<SavedRequest>,
        history: Vec<History>,
        session: Vec<SessionTab>,
        active: usize,
    ) -> Self {
        install_fonts(ctx);
        set_style(ctx);
        #[cfg(target_os = "windows")]
        let mascot = crate::branding::title_texture(ctx);
        #[cfg(not(target_os = "windows"))]
        let mascot = crate::branding::texture(ctx);
        let mut tabs: Vec<_> = session
            .into_iter()
            .enumerate()
            .map(|(i, s)| {
                let mut tab = Tab::new(i as u64 + 1, s.request);
                tab.baseline = s.baseline;
                tab
            })
            .collect();
        if tabs.is_empty() {
            tabs.push(Tab::new(1, Request::default()));
        }
        let (tx, rx) = mpsc::channel();
        Self {
            mascot,
            active: active.min(tabs.len() - 1),
            next_id: tabs.len() as u64 + 1,
            next_job: 1,
            tabs,
            tx,
            rx,
            saved,
            history,
            search: String::new(),
            sidebar: "Collections".into(),
            focus_search: false,
            focus_url: false,
            reveal_active_tab: true,
            show_save: false,
            save_tab_id: 1,
            save_name: String::new(),
            close_tab_id: None,
            pending_close: VecDeque::new(),
            close_after_save: false,
            rename_target: None,
            rename_name: String::new(),
            rename_error: String::new(),
            delete_target: None,
            collection_auth: vec![],
            collection_auth_draft: None,
            editor_height: 240.,
            notice: None,
            environments: vec![],
            globals: vec![],
            import_originals: vec![],
            selected_environment: String::new(),
            show_variables: false,
            variables_scope: 0,
            variables_table: TableState::default(),
            show_import: false,
            import_path: String::new(),
            import_message: String::new(),
        }
    }

    fn send(&mut self) {
        let tab = &mut self.tabs[self.active];
        if tab.job.is_some() {
            return;
        }
        if tab.request.url.trim().is_empty() {
            tab.error = Some("请输入请求 URL".into());
            self.focus_url = true;
            return;
        }
        let job_id = self.next_job;
        self.next_job += 1;
        tab.job = Some(job_id);
        tab.error = None;
        tab.response = None;
        let request = tab.request.clone();
        let tab_id = tab.id;
        let req = self.prepared_request(&request);
        let tx = self.tx.clone();
        self.history.insert(
            0,
            History {
                label: format!("{}  {}", req.method, req.url),
                request,
            },
        );
        self.history.truncate(100);
        thread::spawn(move || {
            let _ = tx.send(WorkerEvent {
                tab_id,
                job_id,
                result: perform(req),
            });
        });
    }
}

impl eframe::App for App {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, "saved", &self.saved);
        eframe::set_value(storage, "history", &self.history);
        eframe::set_value(storage, "environments_v1", &self.environments);
        eframe::set_value(storage, "globals_v1", &self.globals);
        eframe::set_value(storage, "collection_auth_v1", &self.collection_auth);
        eframe::set_value(
            storage,
            linkshop::import::ORIGINALS_KEY,
            &self.import_originals,
        );
        eframe::set_value(
            storage,
            "selected_environment_v1",
            &self.selected_environment,
        );
        let session: Vec<_> = self
            .tabs
            .iter()
            .map(|t| SessionTab {
                request: t.request.clone(),
                baseline: t.baseline.clone(),
            })
            .collect();
        eframe::set_value(storage, "open_tabs_v2", &session);
        eframe::set_value(storage, "active_tab_v2", &self.active);
    }
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_shell_open(ctx);
        let dropped: Vec<_> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .filter_map(|f| f.path.clone())
                .collect()
        });
        if !dropped.is_empty() {
            self.import_files(&dropped);
        }
        while let Ok(event) = self.rx.try_recv() {
            if let Some(tab) = self
                .tabs
                .iter_mut()
                .find(|t| t.id == event.tab_id && t.job == Some(event.job_id))
            {
                tab.job = None;
                match event.result {
                    Ok(reply) => tab.response = Some(reply),
                    Err(error) => tab.error = Some(error),
                }
            }
        }
        if !self.show_save
            && self.close_tab_id.is_none()
            && self.rename_target.is_none()
            && self.delete_target.is_none()
            && self.collection_auth_draft.is_none()
        {
            let shortcuts = ctx.input_mut(|i| {
                (
                    i.consume_key(egui::Modifiers::CTRL, Key::T),
                    i.consume_key(egui::Modifiers::CTRL, Key::W),
                    i.consume_key(egui::Modifiers::CTRL, Key::S),
                    i.consume_key(egui::Modifiers::CTRL, Key::Enter),
                    i.consume_key(egui::Modifiers::CTRL, Key::K),
                    i.consume_key(egui::Modifiers::CTRL, Key::L),
                )
            });
            if shortcuts.0 {
                self.new_tab(Request::default());
            }
            if shortcuts.1 {
                self.request_close(self.tabs[self.active].id);
            }
            if shortcuts.2 {
                self.begin_save();
            }
            if shortcuts.3 {
                self.send();
            }
            if shortcuts.4 {
                self.focus_search = true;
            }
            if shortcuts.5 {
                self.focus_url = true;
            }
        }
        self.render(ctx);
        if self.tabs.iter().any(|t| t.job.is_some()) {
            ctx.request_repaint_after(Duration::from_millis(50));
        }
    }
}

fn matches_search(request: &Request, query: &str) -> bool {
    request.name.to_lowercase().contains(query)
        || request.url.to_lowercase().contains(query)
        || request.method.to_lowercase().contains(query)
        || request
            .source
            .as_ref()
            .is_some_and(|s| s.path.iter().any(|p| p.to_lowercase().contains(query)))
}

#[derive(Default)]
struct RequestFolder {
    name: String,
    key: Vec<String>,
    path: Vec<String>,
    children: Vec<RequestFolder>,
    requests: Vec<(String, Request)>,
}

impl RequestFolder {
    fn expansion_id(&self) -> egui::Id {
        egui::Id::new(("collection_tree_expansion_v3", &self.key))
    }

    fn set_expanded(&self, ctx: &egui::Context, expanded: bool) {
        ctx.data_mut(|data| data.insert_persisted(self.expansion_id(), expanded));
        for child in &self.children {
            child.set_expanded(ctx, expanded);
        }
    }

    fn show(
        &self,
        ui: &mut egui::Ui,
        active: &Request,
        searching: bool,
        action: &mut Option<SidebarAction>,
    ) {
        let depth = self.key.len().saturating_sub(1);
        let on_active_path = active.source.as_ref().is_some_and(|source| {
            source.collection_id == self.key[0]
                && source
                    .path
                    .get(1..)
                    .unwrap_or_default()
                    .starts_with(&self.key[1..])
        });
        let expanded = searching
            || ui
                .ctx()
                .data_mut(|data| data.get_persisted::<bool>(self.expansion_id()))
                .unwrap_or(on_active_path);
        let r = row(ui, if depth == 0 { 32. } else { 28. });
        let hit = ui
            .interact(r, self.expansion_id().with("row"), Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text(self.name.as_str());
        if hit.hovered() {
            ui.painter().rect_filled(r.shrink2(vec2(4., 1.)), 4., HOVER);
        }
        let inset = (depth as f32 * 16.).min((r.width() - 104.).max(0.));
        let x = r.left() + 14. + inset;
        icon(
            ui,
            pos2(x, r.center().y),
            if expanded { Icon::Down } else { Icon::Right },
            MUTED,
        );
        icon(
            ui,
            pos2(x + 21., r.center().y),
            if depth == 0 {
                Icon::Collection
            } else {
                Icon::Folder
            },
            if depth == 0 { CYAN_DARK } else { MUTED },
        );
        ellipsis_label(
            ui,
            Rect::from_min_max(pos2(x + 36., r.top()), r.max - vec2(12., 0.)),
            &self.name,
            if depth == 0 {
                strong_font(13.)
            } else {
                FontId::proportional(13.)
            },
            TEXT,
        );
        if hit.clicked() && !searching {
            ui.ctx()
                .data_mut(|data| data.insert_persisted(self.expansion_id(), !expanded));
        }
        hit.context_menu(|ui| {
            ui.set_min_width(180.);
            let target = FolderTarget {
                key: self.key.clone(),
                path: self.path.clone(),
            };
            if menu_item(ui, "添加请求") {
                *action = Some(SidebarAction::NewRequest(target.clone()));
            }
            if menu_item(ui, "打开全部请求") {
                *action = Some(SidebarAction::OpenAll(target.clone()));
            }
            if depth == 0 && menu_item(ui, "集合身份验证") {
                *action = Some(SidebarAction::Auth(target.clone()));
            }
            if menu_item(ui, "展开全部") {
                self.set_expanded(ui.ctx(), true);
            }
            if menu_item(ui, "折叠全部") {
                self.set_expanded(ui.ctx(), false);
            }
            ui.separator();
            if self.key[0] != "local-saved" && menu_item(ui, "重命名") {
                *action = Some(SidebarAction::Rename(CollectionTarget::Folder(
                    target.clone(),
                )));
            }
            if menu_item(
                ui,
                if depth == 0 {
                    "删除集合"
                } else {
                    "删除文件夹"
                },
            ) {
                *action = Some(SidebarAction::Delete(CollectionTarget::Folder(target)));
            }
        });
        if expanded {
            // All rows share one layout so selection spans the full sidebar width.
            for child in &self.children {
                child.show(ui, active, searching, action);
            }
            ui.push_id(&self.key, |ui| {
                ui.spacing_mut().item_spacing = Vec2::ZERO;
                for (i, (name, request)) in self.requests.iter().enumerate() {
                    let hit = request_row_at_depth(ui, i, name, request, active, depth + 1);
                    request_context_menu(&hit, request, true, action);
                    if hit.clicked() {
                        *action = Some(SidebarAction::Open(Box::new(request.clone())));
                    }
                }
            });
        }
    }
}

fn request_folders(entries: &[(String, Request)]) -> Vec<RequestFolder> {
    let mut roots: Vec<RequestFolder> = vec![];
    for (name, request) in entries {
        let (path, root_key) = request
            .source
            .as_ref()
            .filter(|source| !source.path.is_empty())
            .map(|source| (source.path.clone(), source.collection_id.clone()))
            .unwrap_or_else(|| (vec!["已保存的请求".into()], "local-saved".into()));
        let mut siblings = &mut roots;
        let mut key = vec![];
        for (depth, segment) in path.iter().enumerate() {
            key.push(if depth == 0 {
                root_key.clone()
            } else {
                segment.clone()
            });
            let index = siblings
                .iter()
                .position(|folder| folder.key == key)
                .unwrap_or_else(|| {
                    siblings.push(RequestFolder {
                        name: segment.clone(),
                        key: key.clone(),
                        path: path[..=depth].to_vec(),
                        ..Default::default()
                    });
                    siblings.len() - 1
                });
            let folder = &mut siblings[index];
            if depth + 1 == path.len() {
                folder.requests.push((name.clone(), request.clone()));
            }
            siblings = &mut folder.children;
        }
    }
    roots
}

fn request_row(
    ui: &mut egui::Ui,
    index: usize,
    name: &str,
    request: &Request,
    active: &Request,
) -> egui::Response {
    request_row_at_depth(ui, index, name, request, active, 0)
}

fn menu_item(ui: &mut egui::Ui, title: &str) -> bool {
    let clicked = ui.button(title).clicked();
    if clicked {
        ui.close();
    }
    clicked
}

fn request_context_menu(
    hit: &egui::Response,
    request: &Request,
    saved: bool,
    action: &mut Option<SidebarAction>,
) {
    hit.context_menu(|ui| {
        ui.set_min_width(180.);
        if menu_item(ui, "打开请求") {
            *action = Some(SidebarAction::Open(Box::new(request.clone())));
        }
        if menu_item(ui, "在新标签中打开") {
            *action = Some(SidebarAction::OpenNew(Box::new(request.clone())));
        }
        if menu_item(ui, "复制请求") {
            *action = Some(SidebarAction::Duplicate(Box::new(request.clone())));
        }
        if menu_item(ui, "复制 URL") {
            ui.ctx().copy_text(request.url.clone());
        }
        if saved {
            ui.separator();
            if menu_item(ui, "重命名") {
                *action = Some(SidebarAction::Rename(CollectionTarget::Request(Box::new(
                    request.clone(),
                ))));
            }
            if menu_item(ui, "删除请求") {
                *action = Some(SidebarAction::Delete(CollectionTarget::Request(Box::new(
                    request.clone(),
                ))));
            }
        }
    });
}

fn same_saved_request(request: &Request, active: &Request) -> bool {
    match (&request.source, &active.source) {
        (Some(a), Some(b)) => a.id == b.id && a.collection_id == b.collection_id,
        _ => request == active,
    }
}

fn request_row_at_depth(
    ui: &mut egui::Ui,
    index: usize,
    name: &str,
    request: &Request,
    active: &Request,
    depth: usize,
) -> egui::Response {
    let r = row(ui, 28.);
    let hit = ui
        .interact(r, ui.id().with(("request", index)), Sense::click())
        .on_hover_text(format!("{} {}", request.method, request.url))
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    let selected = same_saved_request(request, active);
    let highlight = r.shrink2(vec2(4., 1.));
    if selected {
        ui.painter().rect_filled(highlight, 4., SELECTED);
        ui.painter().rect_filled(
            Rect::from_min_size(pos2(highlight.left(), r.top() + 7.), vec2(2., 14.)),
            1.,
            ACCENT,
        );
    } else if hit.hovered() {
        ui.painter().rect_filled(highlight, 4., HOVER);
    }
    let inset = (depth as f32 * 16.).min((r.width() - 104.).max(0.));
    let x = r.left() + 14. + inset;
    let method_rect = Rect::from_min_size(pos2(x, r.top()), vec2(42., r.height()));
    let font = strong_font(if request.method.len() > 6 { 9. } else { 10.5 });
    let galley =
        ui.painter()
            .layout_no_wrap(request.method.clone(), font, method_color(&request.method));
    ui.painter().with_clip_rect(method_rect).galley(
        pos2(
            method_rect.center().x - galley.size().x / 2.,
            method_rect.center().y - galley.size().y / 2.,
        ),
        galley,
        method_color(&request.method),
    );
    let display = if name == "未命名请求" || name == "新建请求" {
        &request.url
    } else {
        name
    };
    ellipsis_label(
        ui,
        Rect::from_min_max(pos2(x + 50., r.top()), r.max - vec2(12., 0.)),
        display,
        FontId::proportional(13.),
        TEXT,
    );
    hit
}

fn body_editor(ui: &mut egui::Ui, request: &mut Request, table: &mut TableState) {
    ui.add_space(8.);
    egui::ComboBox::from_id_salt("body_mode")
        .selected_text(match request.body_mode.as_str() {
            "urlencoded" => "x-www-form-urlencoded",
            "formdata" => "form-data（保留）",
            "file" => "binary（保留）",
            "graphql" => "GraphQL / JSON",
            _ => "raw",
        })
        .show_ui(ui, |ui| {
            ui.selectable_value(&mut request.body_mode, "raw".into(), "raw");
            ui.selectable_value(
                &mut request.body_mode,
                "urlencoded".into(),
                "x-www-form-urlencoded",
            );
        });
    if request.body_mode == "urlencoded" {
        pairs_ui(ui, "form_body", "表单参数", &mut request.form, table);
        return;
    }
    if matches!(request.body_mode.as_str(), "formdata" | "file") {
        ui.label("此请求体已保留在原始数据中，当前不支持发送。请选择 raw 或 urlencoded 重新配置。");
        return;
    }
    let r = row(ui, 38.);
    label(ui, r.shrink2(vec2(4., 0.)), "●  原始请求体", 12., TEXT);
    let is_json = serde_json::from_str::<serde_json::Value>(&request.body).is_ok();
    label(
        ui,
        Rect::from_min_size(r.min + vec2(124., 0.), vec2(80., 38.)),
        if is_json { "JSON" } else { "Text" },
        12.,
        ACCENT,
    );
    if text_button(
        ui,
        Rect::from_min_size(pos2(r.right() - 76., r.top() + 5.), vec2(76., 28.)),
        "format_body",
        "格式化",
    )
    .clicked()
        && is_json
    {
        request.body = pretty_json(&request.body);
    }
    egui::ScrollArea::both()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.add_sized(
                [ui.available_width(), ui.available_height().max(64.)],
                egui::TextEdit::multiline(&mut request.body)
                    .font(egui::TextStyle::Monospace)
                    .frame(false)
                    .margin(8.)
                    .hint_text("输入请求体"),
            );
        });
}

fn auth_editor(ui: &mut egui::Ui, auth: &mut Auth, allow_inherit: bool) {
    ui.add_space(16.);
    ui.label(RichText::new("身份验证类型").size(12.).color(MUTED));
    ui.add_space(8.);
    egui::ComboBox::from_id_salt("auth_type")
        .selected_text(if auth.kind == "Inherit" {
            "继承集合身份验证"
        } else if auth.kind.is_empty() {
            "No Auth"
        } else {
            &auth.kind
        })
        .width(220.)
        .show_ui(ui, |ui| {
            if allow_inherit {
                ui.selectable_value(&mut auth.kind, "Inherit".into(), "继承集合身份验证");
            }
            for kind in ["No Auth", "Bearer Token", "Basic Auth", "API Key"] {
                ui.selectable_value(&mut auth.kind, kind.into(), kind);
            }
        });
    ui.add_space(16.);
    match auth.kind.as_str() {
        "Inherit" => {
            ui.label(
                RichText::new("发送时使用集合的最新身份验证，更新 token 只需修改集合配置。")
                    .color(MUTED),
            );
        }
        "Bearer Token" => {
            ui.label("Token");
            ui.add_sized(
                [500., 32.],
                egui::TextEdit::singleline(&mut auth.token).password(true),
            );
        }
        "Basic Auth" => {
            ui.label("用户名");
            ui.add_sized([500., 32.], egui::TextEdit::singleline(&mut auth.username));
            ui.add_space(8.);
            ui.label("密码");
            ui.add_sized(
                [500., 32.],
                egui::TextEdit::singleline(&mut auth.password).password(true),
            );
        }
        "API Key" => {
            ui.horizontal(|ui| {
                ui.label("添加位置");
                egui::ComboBox::from_id_salt("api_key_location")
                    .selected_text(if auth.location == "query" {
                        "查询参数"
                    } else {
                        "请求头"
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut auth.location, "header".into(), "请求头");
                        ui.selectable_value(&mut auth.location, "query".into(), "查询参数");
                    });
            });
            ui.label("名称");
            ui.add_sized([500., 32.], egui::TextEdit::singleline(&mut auth.key));
            ui.add_space(8.);
            ui.label("值");
            ui.add_sized(
                [500., 32.],
                egui::TextEdit::singleline(&mut auth.value).password(true),
            );
        }
        "" | "No Auth" => {
            ui.label(
                RichText::new("此请求不使用身份验证。")
                    .size(12.)
                    .color(MUTED),
            );
        }
        _ => {
            ui.label(RichText::new("此身份验证类型尚未支持，请选择其他类型。").color(MUTED));
        }
    }
}

fn format_size(size: usize) -> String {
    if size < 1024 {
        format!("{size} B")
    } else {
        format!("{:.2} KB", size as f64 / 1024.)
    }
}
fn pretty_json(text: &str) -> String {
    serde_json::from_str::<serde_json::Value>(text)
        .ok()
        .and_then(|v| serde_json::to_string_pretty(&v).ok())
        .unwrap_or_else(|| text.into())
}
