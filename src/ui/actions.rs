use super::*;

impl App {
    pub(super) fn new_tab(&mut self, request: Request) {
        self.tabs.push(Tab::new(self.next_id, request));
        self.next_id += 1;
        self.active = self.tabs.len() - 1;
        self.focus_url = true;
        self.reveal_active_tab = true;
    }

    pub(super) fn open_request(&mut self, request: Request) {
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

    pub(super) fn begin_save(&mut self) {
        self.begin_save_tab(self.tabs[self.active].id);
    }

    pub(super) fn begin_save_tab(&mut self, id: u64) {
        if let Some(tab) = self.tabs.iter().find(|tab| tab.id == id) {
            self.save_tab_id = id;
            self.save_name = tab.request.name.clone();
            self.show_save = true;
        }
    }

    pub(super) fn save_request(&mut self) {
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
        let folder = FolderTarget::parent(&tab.request);
        self.remember_folder(&folder);
        self.show_save = false;
        self.notice = Some(("请求已保存到本机".into(), Instant::now()));
        if self.close_after_save {
            self.close_after_save = false;
            self.close_tab(self.save_tab_id);
            self.advance_close_queue();
        }
    }

    pub(super) fn close_tab(&mut self, id: u64) {
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

    pub(super) fn request_close(&mut self, id: u64) {
        self.request_close_tabs(vec![id]);
    }

    pub(super) fn request_close_tabs(&mut self, ids: Vec<u64>) {
        self.pending_close = ids.into();
        self.advance_close_queue();
    }

    pub(super) fn advance_close_queue(&mut self) {
        while let Some(id) = self.pending_close.pop_front() {
            if self.tabs.iter().any(|t| t.id == id && t.dirty()) {
                self.close_tab_id = Some(id);
                return;
            }
            self.close_tab(id);
        }
    }

    pub(super) fn fresh_request_id(&self) -> String {
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

    pub(super) fn duplicate_request(&mut self, mut request: Request, save: bool) {
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

    pub(super) fn sidebar_action(&mut self, action: SidebarAction) {
        match action {
            SidebarAction::Open(request) => self.open_request(*request),
            SidebarAction::OpenNew(request) => self.new_tab(*request),
            SidebarAction::Duplicate(request) => self.duplicate_request(*request, true),
            SidebarAction::MoveRequest(request, folder) => self.move_request(&request, &folder),
            SidebarAction::NewRequest(folder) => {
                let source = (folder.key[0] != "local-saved" || folder.key.len() > 1).then(|| {
                    ImportSource {
                        id: self.fresh_request_id(),
                        collection_id: folder.key[0].clone(),
                        path: folder.path,
                        complete: true,
                        ..Default::default()
                    }
                });
                self.new_tab(Request {
                    source,
                    auth: Auth::inherit(),
                    ..Default::default()
                });
                self.begin_save();
            }
            SidebarAction::NewFolder(parent) => {
                self.new_folder_draft = Some(NewFolderDraft {
                    parent,
                    name: String::new(),
                    error: String::new(),
                });
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

    pub(super) fn begin_collection_auth(&mut self, target: FolderTarget) {
        let auth = self
            .collection_auth
            .iter()
            .find(|entry| entry.collection_id == target.key[0] && entry.path == target.key[1..])
            .map(|entry| entry.auth.clone())
            .or_else(|| {
                if target.key.len() > 1 {
                    return Some(Auth::inherit());
                }
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

    pub(super) fn save_collection_auth(&mut self) {
        let Some(draft) = self.collection_auth_draft.take() else {
            return;
        };
        self.remember_folder(&draft.target);
        let previous_auth = self.collection_auth.clone();
        if draft.apply_to_existing {
            for entry in &mut self.collection_auth {
                if entry.collection_id == draft.target.key[0]
                    && entry.path.starts_with(&draft.target.key[1..])
                {
                    entry.auth = Auth::inherit();
                }
            }
        }
        if let Some(entry) = self.collection_auth.iter_mut().find(|entry| {
            entry.collection_id == draft.target.key[0] && entry.path == draft.target.key[1..]
        }) {
            entry.auth = draft.auth.clone();
        } else {
            self.collection_auth.push(CollectionAuth {
                collection_id: draft.target.key[0].clone(),
                path: draft.target.key[1..].to_vec(),
                auth: draft.auth.clone(),
            });
        }
        if draft.apply_to_existing {
            let apply = |request: &mut Request| {
                let old_auth = effective_auth(request, &previous_auth);
                request.use_collection_auth(&old_auth);
                let auth = effective_auth(request, &self.collection_auth);
                request.use_collection_auth(&auth);
            };
            for saved in &mut self.saved {
                if draft.target.contains(&saved.request) {
                    apply(&mut saved.request);
                }
            }
            for tab in &mut self.tabs {
                let belongs = draft.target.contains(&tab.request)
                    && (draft.target.key[0] != "local-saved"
                        || draft.target.key.len() > 1
                        || tab.request.auth.kind == "Inherit"
                        || self.saved.iter().any(|saved| {
                            same_saved_request(&saved.request, &tab.request)
                                || same_saved_request(&saved.request, &tab.baseline)
                        }));
                if belongs {
                    // Apply the same operation to the baseline so unrelated
                    // unsaved edits keep their existing dirty state.
                    apply(&mut tab.request);
                    apply(&mut tab.baseline);
                    tab.sync_auth_tables();
                }
            }
        }
        self.notice = Some(("身份验证已保存，下次发送时生效".into(), Instant::now()));
    }

    pub(super) fn remember_folder(&mut self, target: &FolderTarget) {
        if !self.folders.iter().any(|folder| {
            folder.collection_id == target.key[0]
                && folder.path.get(1..).unwrap_or_default() == &target.key[1..]
        }) {
            self.folders.push(CollectionFolder {
                collection_id: target.key[0].clone(),
                path: target.path.clone(),
            });
        }
    }

    fn folder_name_exists(&self, parent: &FolderTarget, name: &str) -> bool {
        let mut path = parent.key[1..].to_vec();
        path.push(name.into());
        self.folders.iter().any(|folder| {
            folder.collection_id == parent.key[0]
                && folder.path.get(1..).is_some_and(|p| p.starts_with(&path))
        }) || self
            .saved
            .iter()
            .map(|saved| &saved.request)
            .chain(self.tabs.iter().map(|tab| &tab.request))
            .any(|request| {
                request.collection_id() == parent.key[0] && request.folder_path().starts_with(&path)
            })
    }

    pub(super) fn create_folder(&mut self, ctx: &egui::Context) {
        let Some(draft) = &self.new_folder_draft else {
            return;
        };
        let parent = draft.parent.clone();
        let name = draft.name.trim().to_owned();
        let error = if name.is_empty() {
            Some("请输入文件夹名称。")
        } else if parent.path.len() >= 65 {
            Some("文件夹嵌套不能超过 64 层。")
        } else if self.folder_name_exists(&parent, &name) {
            Some("同一位置已有此名称，请换一个名称。")
        } else {
            None
        };
        if let Some(error) = error {
            self.new_folder_draft.as_mut().unwrap().error = error.into();
            return;
        }
        let mut path = parent.path.clone();
        path.push(name);
        self.folders.push(CollectionFolder {
            collection_id: parent.key[0].clone(),
            path,
        });
        let key = parent.key;
        for depth in 1..=key.len() {
            ctx.data_mut(|data| {
                data.insert_persisted(
                    egui::Id::new(("collection_tree_expansion_v3", &key[..depth])),
                    true,
                )
            });
        }
        self.new_folder_draft = None;
        self.search.clear();
        self.notice = Some(("文件夹已添加".into(), Instant::now()));
    }

    pub(super) fn prepared_request(&self, request: &Request) -> Request {
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

    pub(super) fn rename_collection_target(&mut self) {
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
                name != folder.path[folder.path.len() - 1]
                    && self.folder_name_exists(
                        &FolderTarget {
                            key: folder.key[..folder.key.len() - 1].to_vec(),
                            path: folder.path[..folder.path.len() - 1].to_vec(),
                        },
                        &name,
                    )
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
        if let CollectionTarget::Folder(folder) = &target {
            for stored in &mut self.folders {
                if stored.collection_id == folder.key[0]
                    && stored
                        .path
                        .get(1..)
                        .is_some_and(|p| p.starts_with(&folder.key[1..]))
                {
                    stored.path[folder.path.len() - 1] = name.clone();
                }
            }
            if folder.key.len() > 1 {
                for entry in &mut self.collection_auth {
                    if entry.collection_id == folder.key[0]
                        && entry.path.starts_with(&folder.key[1..])
                    {
                        entry.path[folder.key.len() - 2] = name.clone();
                    }
                }
            }
        }
        self.rename_target = None;
        self.notice = Some(("已重命名".into(), Instant::now()));
    }

    pub(super) fn delete_collection_target(&mut self) {
        let Some(target) = self.delete_target.take() else {
            return;
        };
        self.saved.retain(|s| !target.contains(&s.request));
        if let CollectionTarget::Folder(folder) = &target {
            self.folders.retain(|stored| {
                stored.collection_id != folder.key[0]
                    || !stored
                        .path
                        .get(1..)
                        .unwrap_or_default()
                        .starts_with(&folder.key[1..])
            });
            self.collection_auth.retain(|entry| {
                entry.collection_id != folder.key[0] || !entry.path.starts_with(&folder.key[1..])
            });
        }
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
}
