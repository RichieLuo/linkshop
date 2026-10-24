use crate::{model::*, widgets::*};
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
    postman_originals: Vec<String>,
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
            app.postman_originals =
                eframe::get_value(storage, "postman_originals_v1").unwrap_or_default();
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
            postman_originals: vec![],
            selected_environment: String::new(),
            show_variables: false,
            variables_scope: 0,
            variables_table: TableState::default(),
            show_import: false,
            import_path: String::new(),
            import_message: String::new(),
        }
    }
    fn new_tab(&mut self, request: Request) {
        self.tabs.push(Tab::new(self.next_id, request));
        self.next_id += 1;
        self.active = self.tabs.len() - 1;
        self.focus_url = true;
        self.reveal_active_tab = true;
    }
    fn open_request(&mut self, request: Request) {
        if let Some(index) = self.tabs.iter().position(|t| {
            same_saved_request(&t.request, &request) || same_saved_request(&t.baseline, &request)
        }) {
            self.active = index;
            self.reveal_active_tab = true;
        } else {
            self.new_tab(request);
            self.focus_url = false;
        }
    }
    fn begin_save(&mut self) {
        self.begin_save_tab(self.tabs[self.active].id);
    }
    fn begin_save_tab(&mut self, id: u64) {
        if let Some(tab) = self.tabs.iter().find(|tab| tab.id == id) {
            self.save_tab_id = id;
            self.save_name = tab.request.name.clone();
            self.show_save = true;
        }
    }
    fn save_request(&mut self) {
        if self.save_name.trim().is_empty() {
            return;
        }
        let Some(tab) = self.tabs.iter_mut().find(|t| t.id == self.save_tab_id) else {
            self.show_save = false;
            return;
        };
        tab.request.name = self.save_name.trim().into();
        if let Some(saved) =
            self.saved
                .iter_mut()
                .find(|s| match (&s.request.source, &tab.request.source) {
                    (Some(a), Some(b)) => a.id == b.id && a.collection_id == b.collection_id,
                    (None, None) => s.label == tab.request.name,
                    _ => false,
                })
        {
            saved.label = tab.request.name.clone();
            saved.request = tab.request.clone();
        } else {
            self.saved.push(SavedRequest {
                label: tab.request.name.clone(),
                request: tab.request.clone(),
            });
        }
        tab.baseline = tab.request.clone();
        self.show_save = false;
        self.notice = Some(("请求已保存到本机".into(), Instant::now()));
        if self.close_after_save {
            self.close_after_save = false;
            self.close_tab(self.save_tab_id);
            self.advance_close_queue();
        }
    }
    fn close_tab(&mut self, id: u64) {
        if let Some(i) = self.tabs.iter().position(|t| t.id == id) {
            self.tabs.remove(i);
            if i < self.active {
                self.active -= 1;
            }
            self.active = self.active.min(self.tabs.len().saturating_sub(1));
            self.reveal_active_tab = true;
        }
        if self.tabs.is_empty() {
            self.new_tab(Request::default());
        }
        self.close_tab_id = None;
    }
    fn request_close(&mut self, id: u64) {
        self.request_close_tabs(vec![id]);
    }
    fn request_close_tabs(&mut self, ids: Vec<u64>) {
        self.pending_close = ids.into();
        self.advance_close_queue();
    }
    fn advance_close_queue(&mut self) {
        while let Some(id) = self.pending_close.pop_front() {
            if self.tabs.iter().any(|t| t.id == id && t.dirty()) {
                self.close_tab_id = Some(id);
                return;
            }
            self.close_tab(id);
        }
    }

    fn fresh_request_id(&self) -> String {
        let mut suffix = self.next_id;
        loop {
            let candidate = format!("linkshop-local-{suffix}");
            if !self
                .saved
                .iter()
                .map(|s| &s.request)
                .chain(self.tabs.iter().map(|t| &t.request))
                .any(|request| request.source.as_ref().is_some_and(|s| s.id == candidate))
            {
                return candidate;
            }
            suffix += 1;
        }
    }

    fn duplicate_request(&mut self, mut request: Request, save: bool) {
        let base = format!("{} 副本", request.name);
        request.name = base.clone();
        let mut suffix = 2;
        while self.saved.iter().any(|s| s.label == request.name)
            || self.tabs.iter().any(|t| t.request.name == request.name)
        {
            request.name = format!("{base} {suffix}");
            suffix += 1;
        }
        let id = self.fresh_request_id();
        if let Some(source) = &mut request.source {
            source.id = id;
        }
        if save {
            self.saved.push(SavedRequest {
                label: request.name.clone(),
                request: request.clone(),
            });
        }
        self.new_tab(request);
        if !save {
            // A copied tab must still prompt before its unsaved contents are discarded.
            self.tabs[self.active].baseline = Request::default();
        }
    }

    fn sidebar_action(&mut self, action: SidebarAction) {
        match action {
            SidebarAction::Open(request) => self.open_request(*request),
            SidebarAction::OpenNew(request) => self.new_tab(*request),
            SidebarAction::Duplicate(request) => self.duplicate_request(*request, true),
            SidebarAction::NewRequest(folder) => {
                let source = (folder.key[0] != "local-saved").then(|| ImportSource {
                    id: self.fresh_request_id(),
                    collection_id: folder.key[0].clone(),
                    path: folder.path,
                    complete: true,
                    ..Default::default()
                });
                self.new_tab(Request {
                    source,
                    auth: Auth::inherit(),
                    ..Default::default()
                });
                self.begin_save();
            }
            SidebarAction::OpenAll(folder) => {
                let requests: Vec<_> = self
                    .saved
                    .iter()
                    .filter(|s| folder.contains(&s.request))
                    .map(|s| s.request.clone())
                    .collect();
                for request in requests {
                    self.open_request(request);
                }
            }
            SidebarAction::Auth(folder) => self.begin_collection_auth(folder),
            SidebarAction::Rename(target) => {
                self.rename_name = target.name().into();
                self.rename_error.clear();
                self.rename_target = Some(target);
            }
            SidebarAction::Delete(target) => self.delete_target = Some(target),
        }
    }

    fn begin_collection_auth(&mut self, target: FolderTarget) {
        let auth = self
            .collection_auth
            .iter()
            .find(|entry| entry.collection_id == target.key[0])
            .map(|entry| entry.auth.clone())
            .or_else(|| {
                // Old profiles stored inherited credentials on each request.
                self.saved
                    .iter()
                    .find(|saved| {
                        target.contains(&saved.request) && saved.request.auth.kind != "Inherit"
                    })
                    .map(|saved| saved.request.auth.clone())
            })
            .unwrap_or_default();
        self.collection_auth_draft = Some(CollectionAuthDraft {
            target,
            auth,
            apply_to_existing: false,
        });
    }

    fn save_collection_auth(&mut self) {
        let Some(draft) = self.collection_auth_draft.take() else {
            return;
        };
        if draft.apply_to_existing {
            for saved in &mut self.saved {
                if draft.target.contains(&saved.request) {
                    saved.request.use_collection_auth(&draft.auth);
                }
            }
            for tab in &mut self.tabs {
                let belongs = draft.target.contains(&tab.request)
                    && (draft.target.key[0] != "local-saved"
                        || tab.request.auth.kind == "Inherit"
                        || self.saved.iter().any(|saved| {
                            same_saved_request(&saved.request, &tab.request)
                                || same_saved_request(&saved.request, &tab.baseline)
                        }));
                if belongs {
                    // Apply the same operation to the baseline so unrelated
                    // unsaved edits keep their existing dirty state.
                    tab.request.use_collection_auth(&draft.auth);
                    tab.baseline.use_collection_auth(&draft.auth);
                    tab.sync_auth_tables();
                }
            }
        }
        if let Some(entry) = self
            .collection_auth
            .iter_mut()
            .find(|entry| entry.collection_id == draft.target.key[0])
        {
            entry.auth = draft.auth;
        } else {
            self.collection_auth.push(CollectionAuth {
                collection_id: draft.target.key[0].clone(),
                auth: draft.auth,
            });
        }
        self.notice = Some(("集合身份验证已保存，下次发送时生效".into(), Instant::now()));
    }

    fn prepared_request(&self, request: &Request) -> Request {
        let mut request = request.clone();
        request.auth = effective_auth(&request, &self.collection_auth);
        let environment = self
            .environments
            .iter()
            .find(|env| env.id == self.selected_environment)
            .map(|env| env.values.as_slice())
            .unwrap_or(&[]);
        request.variables = merge_variables(&[&self.globals, &request.variables, environment]);
        request
    }

    fn rename_collection_target(&mut self) {
        let Some(target) = self.rename_target.clone() else {
            return;
        };
        let name = self.rename_name.trim().to_owned();
        if name.is_empty() {
            return;
        }
        let conflict = match &target {
            CollectionTarget::Request(request) if request.source.is_none() => {
                self.saved.iter().any(|s| {
                    s.request.source.is_none() && s.label == name && !target.contains(&s.request)
                })
            }
            CollectionTarget::Folder(folder) if folder.key.len() > 1 => {
                self.saved.iter().any(|s| {
                    s.request.source.as_ref().is_some_and(|source| {
                        let depth = folder.path.len() - 1;
                        source.collection_id == folder.key[0]
                            && source.path.get(..depth) == Some(&folder.path[..depth])
                            && source.path.get(depth) == Some(&name)
                            && !folder.contains(&s.request)
                    })
                })
            }
            _ => false,
        };
        if conflict {
            self.rename_error = "同一位置已有此名称，请换一个名称。".into();
            return;
        }
        let rename = |request: &mut Request| match &target {
            CollectionTarget::Request(_) => request.name = name.clone(),
            CollectionTarget::Folder(folder) => {
                if let Some(source) = &mut request.source
                    && let Some(segment) = source.path.get_mut(folder.path.len() - 1)
                {
                    *segment = name.clone();
                }
            }
        };
        for saved in &mut self.saved {
            if target.contains(&saved.request) {
                rename(&mut saved.request);
                saved.label = saved.request.name.clone();
            }
        }
        for tab in &mut self.tabs {
            if target.contains(&tab.request) || target.contains(&tab.baseline) {
                rename(&mut tab.request);
                rename(&mut tab.baseline);
            }
        }
        self.rename_target = None;
        self.notice = Some(("已重命名".into(), Instant::now()));
    }

    fn delete_collection_target(&mut self) {
        let Some(target) = self.delete_target.take() else {
            return;
        };
        self.saved.retain(|s| !target.contains(&s.request));
        // Keep open editors as drafts. Saving one can restore the removed request.
        for tab in &mut self.tabs {
            if target.contains(&tab.request) || target.contains(&tab.baseline) {
                tab.baseline = Request::default();
            }
        }
        self.notice = Some((
            "已从集合删除，已打开的请求保留为草稿".into(),
            Instant::now(),
        ));
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
    fn top_bar(&mut self, ctx: &egui::Context) {
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
    fn status_bar(&mut self, ctx: &egui::Context) {
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
    fn navigation(&mut self, ctx: &egui::Context) {
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
                    "import_postman",
                    "导入",
                )
                .on_hover_text("导入 Postman 集合、环境或数据文件夹")
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
impl App {
    fn render(&mut self, ctx: &egui::Context) -> Rect {
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

    fn tab_strip(&mut self, ui: &mut egui::Ui) {
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

    fn workspace(&mut self, ui: &mut egui::Ui) {
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

    fn response_ui(&mut self, ui: &mut egui::Ui) {
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

impl App {
    fn import_files(&mut self, paths: &[PathBuf]) {
        let result = crate::postman::load(paths).and_then(|bundle| {
            crate::postman::backup_profile()?;
            for original in &bundle.originals {
                if !self.postman_originals.contains(original) {
                    self.postman_originals.push(original.clone());
                }
            }
            Ok(crate::postman::merge(
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

    fn dialogs(&mut self, ctx: &egui::Context) {
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

impl eframe::App for App {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, "saved", &self.saved);
        eframe::set_value(storage, "history", &self.history);
        eframe::set_value(storage, "environments_v1", &self.environments);
        eframe::set_value(storage, "globals_v1", &self.globals);
        eframe::set_value(storage, "collection_auth_v1", &self.collection_auth);
        eframe::set_value(storage, "postman_originals_v1", &self.postman_originals);
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

fn set_style(ctx: &egui::Context) {
    // The custom-painted workspace is light; keep native widgets on the same theme
    // when the OS theme arrives after the first frame.
    ctx.set_theme(egui::Theme::Light);
    let mut style = (*ctx.style()).clone();
    style.visuals = egui::Visuals::light();
    style.visuals.panel_fill = WHITE;
    style.visuals.window_fill = WHITE;
    style.visuals.window_stroke = Stroke::new(1., LINE);
    style.visuals.override_text_color = Some(TEXT);
    style.visuals.weak_text_color = Some(MUTED);
    style.visuals.hyperlink_color = ACCENT;
    style.visuals.widgets.noninteractive.fg_stroke = Stroke::new(1., MUTED);
    style.visuals.widgets.noninteractive.bg_stroke = Stroke::new(1., LINE);
    style.visuals.widgets.noninteractive.bg_fill = BG;
    style.visuals.widgets.noninteractive.weak_bg_fill = BG;
    for widget in [
        &mut style.visuals.widgets.inactive,
        &mut style.visuals.widgets.hovered,
        &mut style.visuals.widgets.active,
        &mut style.visuals.widgets.open,
    ] {
        widget.corner_radius = egui::CornerRadius::same(4);
        widget.bg_stroke = Stroke::new(1., LINE);
        widget.fg_stroke = Stroke::new(1., TEXT);
    }
    style.visuals.widgets.inactive.bg_fill = BG;
    style.visuals.widgets.inactive.weak_bg_fill = BG;
    style.visuals.widgets.hovered.bg_fill = HOVER;
    style.visuals.widgets.hovered.weak_bg_fill = HOVER;
    style.visuals.widgets.hovered.bg_stroke = Stroke::new(1., ACCENT);
    style.visuals.widgets.active.bg_fill = SELECTED;
    style.visuals.widgets.active.weak_bg_fill = SELECTED;
    style.visuals.widgets.active.bg_stroke = Stroke::new(1., ACCENT);
    style.visuals.widgets.active.fg_stroke = Stroke::new(1., ACCENT);
    style.visuals.widgets.open.bg_fill = SELECTED;
    style.visuals.widgets.open.weak_bg_fill = SELECTED;
    style.visuals.widgets.open.bg_stroke = Stroke::new(1., ACCENT);
    style.visuals.selection.bg_fill = SELECTED;
    style.visuals.selection.stroke = Stroke::new(1., ACCENT);
    style.spacing.item_spacing = vec2(8., 6.);
    style.spacing.button_padding = vec2(12., 6.);
    style.spacing.interact_size = vec2(32., 28.);
    style
        .text_styles
        .insert(egui::TextStyle::Body, FontId::proportional(12.));
    style
        .text_styles
        .insert(egui::TextStyle::Button, FontId::proportional(12.));
    style
        .text_styles
        .insert(egui::TextStyle::Monospace, FontId::monospace(12.));
    ctx.set_style(style);
}

fn install_fonts(ctx: &egui::Context) {
    #[cfg(target_os = "windows")]
    {
        let fonts = std::env::var_os("WINDIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from(r"C:\Windows"))
            .join("Fonts");
        let mut definitions = egui::FontDefinitions::default();
        for (name, paths, primary) in [
            ("LinkShop-UI", vec!["segoeui.ttf"], true),
            (
                "LinkShop-CJK",
                vec!["msyh.ttc", "msjh.ttc", "simsun.ttc"],
                false,
            ),
        ] {
            if let Some(bytes) = paths.iter().find_map(|p| std::fs::read(fonts.join(p)).ok()) {
                definitions.font_data.insert(
                    name.into(),
                    std::sync::Arc::new(egui::FontData::from_owned(bytes)),
                );
                if primary {
                    definitions
                        .families
                        .get_mut(&egui::FontFamily::Proportional)
                        .unwrap()
                        .insert(0, name.into());
                } else {
                    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
                        definitions
                            .families
                            .get_mut(&family)
                            .unwrap()
                            .push(name.into());
                    }
                }
            }
        }
        let mut semibold = vec![];
        for (name, filename) in [
            ("LinkShop-UI-Semibold", "seguisb.ttf"),
            ("LinkShop-CJK-Bold", "msyhbd.ttc"),
        ] {
            if let Ok(bytes) = std::fs::read(fonts.join(filename)) {
                definitions.font_data.insert(
                    name.into(),
                    std::sync::Arc::new(egui::FontData::from_owned(bytes)),
                );
                semibold.push(name.into());
            }
        }
        semibold.extend(
            definitions.families[&egui::FontFamily::Proportional]
                .iter()
                .cloned(),
        );
        definitions
            .families
            .insert(egui::FontFamily::Name("LinkShop-Semibold".into()), semibold);
        ctx.set_fonts(definitions);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_requests() -> Vec<SavedRequest> {
        crate::postman::parse(&serde_json::json!({
            "collections": [
                {"info":{"name":"Collection A","_postman_id":"c1"},"item":[
                    {"name":"Folder","item":[{"id":"r1","name":"First request","request":{"method":"GET","url":"https://example.com/one"}}]},
                    {"name":"Other folder","item":[{"id":"r2","name":"Second request","request":{"method":"GET","url":"https://example.com/two"}}]}
                ]},
                {"info":{"name":"Collection B","_postman_id":"c2"},"item":[
                    {"name":"Folder","item":[{"id":"r1","name":"Third request","request":{"method":"GET","url":"https://example.com/three"}}]}
                ]}
            ]
        })).unwrap().requests
    }

    #[test]
    fn open_requests_send_current_collection_auth_and_keep_request_overrides() {
        use std::io::{Read, Write};

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        listener.set_nonblocking(true).unwrap();
        let worker = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(15);
            let mut requests = vec![];
            while requests.len() < 6 {
                let mut socket = match listener.accept() {
                    Ok((socket, _)) => socket,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(Instant::now() < deadline, "requests never arrived");
                        thread::sleep(Duration::from_millis(10));
                        continue;
                    }
                    Err(error) => panic!("{error}"),
                };
                socket
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut bytes = Vec::new();
                while !bytes.windows(4).any(|part| part == b"\r\n\r\n") {
                    let mut chunk = [0; 4096];
                    let n = socket.read(&mut chunk).unwrap();
                    assert!(n > 0);
                    bytes.extend_from_slice(&chunk[..n]);
                }
                requests.push(String::from_utf8(bytes).unwrap().to_lowercase());
                socket
                    .write_all(
                        b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}",
                    )
                    .unwrap();
            }
            requests
        });
        let ctx = egui::Context::default();
        let mut saved = fixture_requests();
        for entry in &mut saved {
            entry.request.url = format!("http://{addr}/");
        }
        let mut app = App::with_state(&ctx, saved, vec![], vec![], 0);
        app.collection_auth = vec![
            CollectionAuth {
                collection_id: "c1".into(),
                auth: Auth {
                    kind: "Bearer Token".into(),
                    token: "{{token}}".into(),
                    ..Default::default()
                },
            },
            CollectionAuth {
                collection_id: "c2".into(),
                auth: Auth {
                    kind: "Bearer Token".into(),
                    token: "other-collection".into(),
                    ..Default::default()
                },
            },
        ];
        app.globals = parse_bulk("token:global", &[]);
        app.saved[0].request.variables = parse_bulk("token:collection", &[]);
        app.environments.push(Environment {
            id: "env".into(),
            values: parse_bulk("token:environment-one", &[]),
            ..Default::default()
        });
        app.selected_environment = "env".into();
        app.open_request(app.saved[0].request.clone());
        let first_tab = app.active;
        let send = |app: &mut App| {
            app.send();
            let event = app.rx.recv_timeout(Duration::from_secs(10)).unwrap();
            assert_eq!(event.result.unwrap().status, 200);
            assert_eq!(event.tab_id, app.tabs[app.active].id);
            app.tabs[app.active].job = None;
        };
        send(&mut app);
        app.environments[0].values[0].value = "environment-two".into();
        send(&mut app);
        app.begin_collection_auth(FolderTarget::collection(&app.tabs[first_tab].request));
        app.collection_auth_draft.as_mut().unwrap().auth.token = "fresh-token".into();
        app.save_collection_auth();
        send(&mut app);
        assert_eq!(app.tabs[first_tab].request.auth.kind, "Inherit");
        assert!(!app.tabs[first_tab].dirty());
        assert_eq!(app.history[0].request.auth.kind, "Inherit");
        app.open_request(app.saved[2].request.clone());
        send(&mut app);
        app.active = first_tab;
        app.tabs[first_tab].request.auth = Auth {
            kind: "Basic Auth".into(),
            username: "user".into(),
            password: "pass".into(),
            ..Default::default()
        };
        send(&mut app);
        app.tabs[first_tab].request.auth.kind = "No Auth".into();
        send(&mut app);
        let wires = worker.join().unwrap();
        for (wire, token) in wires.iter().zip([
            "environment-one",
            "environment-two",
            "fresh-token",
            "other-collection",
        ]) {
            assert!(
                wire.contains(&format!("authorization: bearer {token}\r\n")),
                "{wire}"
            );
        }
        assert!(wires[4].contains("authorization: basic dxnlcjpwyxnz\r\n"));
        assert!(!wires[5].contains("authorization:"));
    }

    #[test]
    fn bulk_auth_conversion_preserves_dirty_edits_isolates_collections_and_persists() {
        let ctx = egui::Context::default();
        let mut saved = fixture_requests();
        saved[0].request.auth = Auth {
            kind: "Bearer Token".into(),
            token: "expired".into(),
            ..Default::default()
        };
        saved[0].request.headers = parse_bulk("Authorization:Bearer expired\nX-Keep:yes", &[]);
        saved[1].request.auth = Auth {
            kind: "API Key".into(),
            key: "key".into(),
            value: "expired".into(),
            location: "query".into(),
            ..Default::default()
        };
        saved[1].request.url = "https://example.com/two?key=expired&keep=yes".into();
        saved[1].request.read_url_params();
        saved[2].request.auth.kind = "No Auth".into();
        let other_request = saved[2].request.clone();
        let mut app = App::with_state(&ctx, saved, vec![], vec![], 0);
        app.open_request(app.saved[0].request.clone());
        let clean_tab = app.active;
        app.tabs[clean_tab].headers_table.bulk = true;
        app.tabs[clean_tab].headers_table.text = bulk_text(&app.tabs[clean_tab].request.headers);
        app.open_request(app.saved[1].request.clone());
        let dirty_tab = app.active;
        app.tabs[dirty_tab].request.body = "unsaved body".into();
        let target = FolderTarget::collection(&app.tabs[clean_tab].request);
        app.begin_collection_auth(target.clone());
        assert_eq!(
            app.collection_auth_draft.as_ref().unwrap().auth.token,
            "expired"
        );
        assert!(
            !app.collection_auth_draft
                .as_ref()
                .unwrap()
                .apply_to_existing
        );
        let draft = app.collection_auth_draft.as_mut().unwrap();
        draft.auth.token = "fresh".into();
        draft.apply_to_existing = true;
        app.save_collection_auth();
        assert!(!app.tabs[clean_tab].dirty());
        assert!(app.tabs[dirty_tab].dirty());
        assert_eq!(app.tabs[dirty_tab].request.body, "unsaved body");
        assert_eq!(app.tabs[dirty_tab].baseline.body, "");
        assert_eq!(app.saved[0].request.auth, Auth::inherit());
        assert!(!app.saved[0].request.headers[0].enabled);
        assert!(
            app.tabs[clean_tab]
                .headers_table
                .text
                .starts_with("// Authorization:")
        );
        assert!(app.saved[0].request.headers[1].enabled);
        assert!(!app.saved[1].request.params[0].enabled);
        assert_eq!(app.saved[1].request.url, "https://example.com/two?keep=yes");
        assert_eq!(app.saved[2].request, other_request);
        app.sidebar_action(SidebarAction::Rename(CollectionTarget::Folder(
            target.clone(),
        )));
        app.rename_name = "Renamed collection".into();
        app.rename_collection_target();
        assert_eq!(
            app.prepared_request(&app.tabs[clean_tab].request)
                .auth
                .token,
            "fresh"
        );
        app.sidebar_action(SidebarAction::NewRequest(target));
        assert_eq!(app.tabs[app.active].request.auth, Auth::inherit());
        assert_eq!(
            app.prepared_request(&app.tabs[app.active].request)
                .auth
                .token,
            "fresh"
        );

        #[derive(Default)]
        struct Storage(std::collections::BTreeMap<String, String>);
        impl eframe::Storage for Storage {
            fn get_string(&self, key: &str) -> Option<String> {
                self.0.get(key).cloned()
            }
            fn set_string(&mut self, key: &str, value: String) {
                self.0.insert(key.into(), value);
            }
            fn flush(&mut self) {}
        }
        let mut storage = Storage::default();
        eframe::App::save(&mut app, &mut storage);
        let restored_auth: Vec<CollectionAuth> =
            eframe::get_value(&storage, "collection_auth_v1").unwrap();
        let restored_saved: Vec<SavedRequest> = eframe::get_value(&storage, "saved").unwrap();
        let restored_tabs: Vec<SessionTab> = eframe::get_value(&storage, "open_tabs_v2").unwrap();
        assert_eq!(
            effective_auth(&restored_saved[0].request, &restored_auth).token,
            "fresh"
        );
        assert_eq!(restored_tabs[dirty_tab].request.auth, Auth::inherit());
        assert_ne!(
            restored_tabs[dirty_tab].request,
            restored_tabs[dirty_tab].baseline
        );
    }

    #[test]
    fn collection_auth_dialog_can_cancel_or_save_without_replacing_overrides() {
        let ctx = egui::Context::default();
        let mut saved = fixture_requests();
        saved[0].request.auth = Auth {
            kind: "Bearer Token".into(),
            token: "request-token".into(),
            ..Default::default()
        };
        let mut app = App::with_state(&ctx, saved, vec![], vec![], 0);
        frame(&ctx, &mut app, vec![]);
        let output = frame(&ctx, &mut app, vec![]);
        let position = text_center(&output, "Collection A");
        let output = click(&ctx, &mut app, position, egui::PointerButton::Secondary);
        let position = text_center(&output, "集合身份验证");
        click(&ctx, &mut app, position, egui::PointerButton::Primary);
        assert!(app.collection_auth_draft.is_some());
        app.collection_auth_draft.as_mut().unwrap().auth.token = "cancelled".into();
        let output = frame(&ctx, &mut app, vec![]);
        let position = text_center(&output, "取消");
        click(&ctx, &mut app, position, egui::PointerButton::Primary);
        assert!(app.collection_auth.is_empty());
        assert_eq!(app.saved[0].request.auth.token, "request-token");
        app.begin_collection_auth(FolderTarget::collection(&app.saved[0].request));
        app.collection_auth_draft.as_mut().unwrap().auth.token = "collection-token".into();
        for size in [vec2(1080., 680.), vec2(1920., 1080.)] {
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(pos2(0., 0.), size)),
                    ..Default::default()
                },
                |ctx| {
                    app.render(ctx);
                },
            );
            let mut text = vec![];
            for shape in &output.shapes {
                collect_text(&shape.shape, &mut text);
            }
            assert!(
                !text.iter().any(|s| s.contains("use of widget ID")),
                "{text:?}"
            );
        }
        let output = frame(&ctx, &mut app, vec![]);
        let position = text_center(&output, "保存");
        click(&ctx, &mut app, position, egui::PointerButton::Primary);
        assert!(app.collection_auth_draft.is_none());
        assert_eq!(app.collection_auth[0].auth.token, "collection-token");
        assert_eq!(app.saved[0].request.auth.token, "request-token");
    }

    fn frame(ctx: &egui::Context, app: &mut App, events: Vec<egui::Event>) -> egui::FullOutput {
        ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(pos2(0., 0.), vec2(1440., 900.))),
                events,
                ..Default::default()
            },
            |ctx| {
                app.render(ctx);
            },
        )
    }

    fn text_center(output: &egui::FullOutput, title: &str) -> egui::Pos2 {
        fn find(shape: &egui::Shape, title: &str) -> Option<egui::Pos2> {
            match shape {
                egui::Shape::Text(text) if text.galley.job.text == title => {
                    Some(text.pos + text.galley.size() / 2.)
                }
                egui::Shape::Vec(shapes) => shapes.iter().find_map(|s| find(s, title)),
                _ => None,
            }
        }
        output
            .shapes
            .iter()
            .rev()
            .find_map(|s| find(&s.shape, title))
            .unwrap_or_else(|| panic!("missing UI text: {title}"))
    }

    fn click(
        ctx: &egui::Context,
        app: &mut App,
        pos: egui::Pos2,
        button: egui::PointerButton,
    ) -> egui::FullOutput {
        frame(ctx, app, vec![egui::Event::PointerMoved(pos)]);
        for pressed in [true, false] {
            frame(
                ctx,
                app,
                vec![egui::Event::PointerButton {
                    pos,
                    button,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                }],
            );
        }
        frame(ctx, app, vec![])
    }

    #[test]
    fn collection_folder_and_request_rows_open_context_menus() {
        for (title, menu_titles) in [
            (
                "Collection A",
                vec!["添加请求", "打开全部请求", "重命名", "删除集合"],
            ),
            (
                "Folder",
                vec!["添加请求", "展开全部", "重命名", "删除文件夹"],
            ),
            (
                "First request",
                vec!["在新标签中打开", "复制请求", "复制 URL", "删除请求"],
            ),
        ] {
            let ctx = egui::Context::default();
            let saved = fixture_requests();
            let folders = request_folders(
                &saved
                    .iter()
                    .map(|s| (s.label.clone(), s.request.clone()))
                    .collect::<Vec<_>>(),
            );
            let mut app = App::with_state(&ctx, saved, vec![], vec![], 0);
            for folder in folders {
                folder.set_expanded(&ctx, true);
            }
            frame(&ctx, &mut app, vec![]);
            let output = frame(&ctx, &mut app, vec![]);
            let pos = text_center(&output, title);
            let output = click(&ctx, &mut app, pos, egui::PointerButton::Secondary);
            for title in menu_titles {
                text_center(&output, title);
            }
            assert_eq!(app.tabs.len(), 1, "right-click should only open a menu");
            if title == "First request" {
                let pos = text_center(&output, "复制请求");
                click(&ctx, &mut app, pos, egui::PointerButton::Primary);
                assert_eq!(app.saved.len(), 4);
                assert_eq!(app.tabs[app.active].request.name, "First request 副本");
                assert_ne!(
                    app.saved[0].request.source.as_ref().unwrap().id,
                    app.tabs[app.active].request.source.as_ref().unwrap().id
                );
            }
        }
    }

    #[test]
    fn tab_context_menu_closes_tabs_relative_to_the_clicked_tab() {
        let ctx = egui::Context::default();
        let mut app = App::with_state(&ctx, vec![], vec![], vec![], 0);
        app.tabs[0].request.name = "Tab 1".into();
        app.tabs[0].baseline = app.tabs[0].request.clone();
        for name in ["Tab 2", "Tab 3", "Tab 4"] {
            app.new_tab(Request {
                name: name.into(),
                ..Default::default()
            });
        }
        frame(&ctx, &mut app, vec![]);
        let output = frame(&ctx, &mut app, vec![]);
        let pos = text_center(&output, "Tab 2");
        let output = click(&ctx, &mut app, pos, egui::PointerButton::Secondary);
        assert_eq!(app.active, 3);
        for title in [
            "保存请求",
            "复制标签",
            "关闭标签",
            "关闭其他标签",
            "关闭右侧标签",
            "关闭所有标签",
        ] {
            text_center(&output, title);
        }
        let pos = text_center(&output, "关闭右侧标签");
        click(&ctx, &mut app, pos, egui::PointerButton::Primary);
        assert_eq!(
            app.tabs
                .iter()
                .map(|t| t.request.name.as_str())
                .collect::<Vec<_>>(),
            ["Tab 1", "Tab 2"]
        );
        assert_eq!(app.tabs[app.active].request.name, "Tab 2");
    }

    #[test]
    fn bulk_close_saves_dirty_tabs_and_can_cancel_the_remaining_queue() {
        let ctx = egui::Context::default();
        let mut app = App::with_state(&ctx, vec![], vec![], vec![], 0);
        app.tabs[0].request.url = "https://example.com/unsaved-one".into();
        let first = app.tabs[0].id;
        app.new_tab(Request {
            name: "Clean".into(),
            ..Default::default()
        });
        app.new_tab(Request {
            name: "Unsaved".into(),
            ..Default::default()
        });
        app.tabs[2].request.url = "https://example.com/unsaved-two".into();
        let last = app.tabs[2].id;
        app.request_close_tabs(app.tabs.iter().map(|t| t.id).collect());
        assert_eq!(app.close_tab_id, Some(first));
        frame(&ctx, &mut app, vec![]);
        let output = frame(&ctx, &mut app, vec![]);
        click(
            &ctx,
            &mut app,
            text_center(&output, "保存"),
            egui::PointerButton::Primary,
        );
        assert!(app.show_save);
        assert_eq!(app.save_tab_id, first);
        app.save_name = "Saved first".into();
        app.save_request();
        assert_eq!(app.saved[0].request.url, "https://example.com/unsaved-one");
        assert_eq!(app.tabs.len(), 1);
        assert_eq!(app.close_tab_id, Some(last));
        let output = frame(&ctx, &mut app, vec![]);
        click(
            &ctx,
            &mut app,
            text_center(&output, "取消"),
            egui::PointerButton::Primary,
        );
        assert!(app.close_tab_id.is_none());
        assert!(app.pending_close.is_empty());
        assert_eq!(app.tabs[0].id, last);
        assert!(app.tabs[0].dirty());
        app.request_close(last);
        let output = frame(&ctx, &mut app, vec![]);
        click(
            &ctx,
            &mut app,
            text_center(&output, "不保存"),
            egui::PointerButton::Primary,
        );
        assert_eq!(app.tabs.len(), 1);
        assert_ne!(app.tabs[0].id, last);
        assert!(!app.tabs[0].dirty());
    }

    #[test]
    fn folder_actions_preserve_collection_identity_and_dirty_editors() {
        let ctx = egui::Context::default();
        let mut app = App::with_state(&ctx, fixture_requests(), vec![], vec![], 0);
        let original = app.saved[0].request.clone();
        app.open_request(original.clone());
        app.tabs[app.active].request.body = "unsaved content".into();
        let target = FolderTarget {
            key: vec!["c1".into(), "Folder".into()],
            path: vec!["Collection A".into(), "Folder".into()],
        };
        app.sidebar_action(SidebarAction::OpenAll(target.clone()));
        assert_eq!(
            app.tabs.len(),
            2,
            "opening a collection must keep an edited tab"
        );
        assert_eq!(app.tabs[app.active].request.body, "unsaved content");
        app.sidebar_action(SidebarAction::Rename(CollectionTarget::Folder(
            target.clone(),
        )));
        app.rename_name = "Other folder".into();
        app.rename_collection_target();
        assert!(
            app.rename_target.is_some(),
            "renaming must not merge sibling folders"
        );
        app.rename_name = "Renamed folder".into();
        app.rename_collection_target();
        assert!(app.rename_target.is_none());
        assert_eq!(
            app.saved[0].request.source.as_ref().unwrap().path,
            ["Collection A", "Renamed folder"]
        );
        assert_eq!(
            app.saved[2].request.source.as_ref().unwrap().path,
            ["Collection B", "Folder"]
        );
        assert_eq!(
            app.tabs[app.active].baseline.source.as_ref().unwrap().path,
            ["Collection A", "Renamed folder"]
        );
        assert!(app.tabs[app.active].dirty());
        app.sidebar_action(SidebarAction::NewRequest(FolderTarget {
            key: vec!["c1".into(), "Renamed folder".into()],
            path: vec!["Collection A".into(), "Renamed folder".into()],
        }));
        app.save_name = "Added request".into();
        app.save_request();
        assert_eq!(
            app.saved[3].request.source.as_ref().unwrap().path,
            ["Collection A", "Renamed folder"]
        );
        let target = CollectionTarget::Folder(FolderTarget {
            key: vec!["c1".into()],
            path: vec!["Collection A".into()],
        });
        app.sidebar_action(SidebarAction::Delete(target));
        app.delete_collection_target();
        assert_eq!(app.saved.len(), 1);
        assert_eq!(
            app.saved[0].request.source.as_ref().unwrap().collection_id,
            "c2"
        );
        assert_eq!(app.tabs[1].request.body, "unsaved content");
        assert!(app.tabs[1].dirty());
        assert!(app.tabs[2].dirty());
    }

    #[test]
    fn collection_rows_toggle_and_search_preserves_collapsed_state() {
        let ctx = egui::Context::default();
        install_fonts(&ctx);
        set_style(&ctx);
        let request = Request {
            name: "Nested request".into(),
            source: Some(ImportSource {
                id: "request".into(),
                collection_id: "collection".into(),
                path: vec!["Collection".into()],
                complete: true,
                ..Default::default()
            }),
            ..Default::default()
        };
        let folders = request_folders(&[(request.name.clone(), request)]);
        let folder = &folders[0];
        let active = Request::default();
        let render = |events, searching| {
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(pos2(0., 0.), vec2(350., 300.))),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        ui.spacing_mut().item_spacing = Vec2::ZERO;
                        folder.show(ui, &active, searching, &mut None);
                    });
                },
            );
            let mut text = vec![];
            for shape in &output.shapes {
                collect_text(&shape.shape, &mut text);
            }
            text
        };
        assert!(!render(vec![], false).iter().any(|s| s == "Nested request"));
        for pressed in [true, false] {
            render(
                vec![
                    egui::Event::PointerMoved(pos2(30., 24.)),
                    egui::Event::PointerButton {
                        pos: pos2(30., 24.),
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                false,
            );
        }
        assert!(render(vec![], false).iter().any(|s| s == "Nested request"));
        folder.set_expanded(&ctx, false);
        assert!(render(vec![], true).iter().any(|s| s == "Nested request"));
        assert_eq!(
            ctx.data_mut(|data| data.get_persisted::<bool>(folder.expansion_id())),
            Some(false)
        );
        assert!(!render(vec![], false).iter().any(|s| s == "Nested request"));
    }

    #[test]
    fn imported_folders_and_duplicate_names_keep_identity_and_render_forms_and_variables() {
        let ctx = egui::Context::default();
        let bundle = crate::postman::parse(&serde_json::json!({
            "collections": [
                {"info":{"name":"Shared","_postman_id":"c1"},"item":[{"name":"Folder","item":[{"id":"r1","name":"Same","request":{"method":"POST","url":"https://example.com/one","body":{"mode":"urlencoded","urlencoded":[{"key":"name","value":"{{name}}"}]}}}]}]},
                {"info":{"name":"Shared","_postman_id":"c2"},"item":[{"id":"r2","name":"Same","request":{"method":"GET","url":"https://example.com/two"}}]}
            ],
            "environments":[{"id":"env","name":"Test environment","values":[{"key":"name","value":"test"}]}]
        })).unwrap();
        let entries: Vec<_> = bundle
            .requests
            .iter()
            .map(|r| (r.label.clone(), r.request.clone()))
            .collect();
        let folders = request_folders(&entries);
        assert_eq!(folders.len(), 2);
        assert_ne!(folders[0].key, folders[1].key);
        assert_eq!(folders[0].children[0].name, "Folder");
        let mut app = App::with_state(&ctx, bundle.requests, vec![], vec![], 0);
        app.environments = bundle.environments;
        app.selected_environment = "env".into();
        app.open_request(app.saved[0].request.clone());
        app.begin_save();
        app.tabs[app.active].request.form[0].value = "changed".into();
        assert!(same_saved_request(
            &app.saved[0].request,
            &app.tabs[app.active].request
        ));
        assert!(!same_saved_request(
            &app.saved[1].request,
            &app.tabs[app.active].request
        ));
        app.save_request();
        assert_eq!(app.saved.len(), 2);
        assert_eq!(app.saved[1].request.url, "https://example.com/two");
        for size in [vec2(1080., 680.), vec2(1920., 1080.)] {
            for editor in ["Params", "Body", "Auth", "Scripts"] {
                app.tabs[app.active].editor = editor.into();
                app.show_variables = editor == "Body";
                for scope in 0..3 {
                    app.variables_scope = scope;
                    let output = ctx.run(
                        egui::RawInput {
                            screen_rect: Some(Rect::from_min_size(pos2(0., 0.), size)),
                            ..Default::default()
                        },
                        |ctx| {
                            app.render(ctx);
                        },
                    );
                    let mut text = vec![];
                    for shape in &output.shapes {
                        collect_text(&shape.shape, &mut text);
                    }
                    assert!(
                        !text.iter().any(|s| s.contains("use of widget ID")),
                        "{text:?}"
                    );
                }
            }
        }
    }

    fn collect_text(shape: &egui::Shape, result: &mut Vec<String>) {
        match shape {
            egui::Shape::Text(text) => result.push(text.galley.job.text.clone()),
            egui::Shape::Vec(shapes) => {
                for shape in shapes {
                    collect_text(shape, result);
                }
            }
            _ => {}
        }
    }

    #[test]
    fn workspace_renders_at_small_and_large_sizes_without_id_clashes_or_panel_overflow() {
        let ctx = egui::Context::default();
        let mut app = App::with_state(&ctx, vec![], vec![], vec![], 0);
        app.tabs[0].request.url = "http://localhost/?q=one&q=two".into();
        app.tabs[0].request.read_url_params();
        app.tabs[0].response = Some(Reply {
            status: 200,
            reason: "OK".into(),
            elapsed: 1,
            size: 2,
            headers: vec![("content-type".into(), "application/json".into())],
            body: "{}".into(),
        });
        for size in [vec2(1080., 680.), vec2(1920., 1009.)] {
            for editor in ["Params", "Headers", "Body", "Auth", "Scripts", "Settings"] {
                for response in ["Body", "Headers", "Cookies", "Tests"] {
                    app.tabs[0].editor = editor.into();
                    app.tabs[0].response_tab = response.into();
                    let mut panel = Rect::NOTHING;
                    let output = ctx.run(
                        egui::RawInput {
                            screen_rect: Some(Rect::from_min_size(pos2(0., 0.), size)),
                            ..Default::default()
                        },
                        |ctx| {
                            panel = app.render(ctx);
                        },
                    );
                    assert!((panel.top() - 48.).abs() < 1., "header overflow: {panel:?}");
                    assert!(panel.height() >= size.y - 74., "panel overflow: {panel:?}");
                    assert!(
                        panel.right() <= size.x + 1.,
                        "horizontal overflow: {panel:?}"
                    );
                    let mut text = vec![];
                    for shape in &output.shapes {
                        collect_text(&shape.shape, &mut text);
                    }
                    assert!(
                        !text.iter().any(|s| s.contains("use of widget ID")),
                        "{editor}/{response}: {text:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn closing_an_earlier_tab_keeps_the_selected_request_and_dirty_drafts() {
        let ctx = egui::Context::default();
        let mut app = App::with_state(&ctx, vec![], vec![], vec![], 0);
        let first = app.tabs[0].id;
        app.new_tab(Request {
            name: "Second".into(),
            ..Default::default()
        });
        app.tabs[1].request.url = "https://example.com/unsaved".into();
        let selected = app.tabs[1].id;
        app.close_tab(first);
        assert_eq!(app.tabs[app.active].id, selected);
        assert_eq!(
            app.tabs[app.active].request.url,
            "https://example.com/unsaved"
        );
        assert!(app.tabs[app.active].dirty());
        app.request_close(selected);
        assert_eq!(app.close_tab_id, Some(selected));
    }

    #[test]
    fn column_and_panel_dividers_respond_to_pointer_drags() {
        let ctx = egui::Context::default();
        let mut app = App::with_state(&ctx, vec![], vec![], vec![], 0);
        let screen = Rect::from_min_size(pos2(0., 0.), vec2(1080., 680.));
        let frame = |app: &mut App, events: Vec<egui::Event>| {
            let mut panel = Rect::NOTHING;
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(screen),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    panel = app.render(ctx);
                },
            );
            panel
        };
        let panel = frame(&mut app, vec![]);
        let column = pos2(
            panel.left() + 16. + 44. + (panel.width() - 32. - 44.) * 0.34,
            48. + 40. + 48. + 54. + 38. + 34. + 16.,
        );
        let press = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        frame(&mut app, vec![egui::Event::PointerMoved(column)]);
        frame(&mut app, vec![press(column, true)]);
        let moved = column - vec2(60., 0.);
        frame(&mut app, vec![egui::Event::PointerMoved(moved)]);
        frame(&mut app, vec![press(moved, false)]);
        assert!(
            app.tabs[0].params_table.columns[0] < 0.3,
            "column divider did not move"
        );
        let initial = app.editor_height;
        let split = pos2(panel.center().x, 48. + 40. + 48. + 54. + 38. + initial + 3.);
        frame(&mut app, vec![egui::Event::PointerMoved(split)]);
        frame(&mut app, vec![press(split, true)]);
        let moved = split - vec2(0., 40.);
        frame(&mut app, vec![egui::Event::PointerMoved(moved)]);
        frame(&mut app, vec![press(moved, false)]);
        assert!(
            app.editor_height < initial - 30.,
            "panel divider did not move"
        );
    }
}
