<p align="center">
  <img src="assets/branding/linkshop-fox-logo.png" alt="LinkShop · 狐狸 Logo" width="560">
</p>

# LinkShop

LinkShop is a local-first Rust desktop API client inspired by Postman. It opens directly into the request workspace and has no account, login, or cloud requirement.

## Run on Windows

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\dev.ps1 run
```

The helper loads an installed MSVC toolchain and Windows SDK. It uses Cargo's configured registry; the legacy `-CratesIo` option remains accepted. You can also use `cargo run --locked` from an already initialized MSVC developer shell.

## Current scope

- Postman-style workspace with a searchable sidebar, scrollable request tabs, colored method menu, and a resizable request/response split.
- Fox-logo palette: cobalt blue actions and selection indicators, cyan navigation accents, navy text, and pale blue surfaces. Shared brand tokens also cover menus, focus, checkboxes, and dialogs.
- Collection navigation uses consistent rows, collection/folder icons, readable method labels, ellipsis for long names, remembered expansion, and a collapse-all action. Selected requests stay highlighted while being edited.
- Right-click collections and folders to add/open requests, expand/collapse, rename, or delete; right-click requests to open, duplicate, copy URLs, rename, or delete. Deletion asks for confirmation and keeps open editors as drafts.
- Right-click request tabs to save, duplicate, or close the current/other/right/all tabs. Bulk closing prompts for each unsaved draft; cancel stops the remaining closures. Right-click empty tab-strip space to create a request or close all tabs.
- Compose and send GET, POST, PUT, PATCH, DELETE, HEAD, OPTIONS, and custom HTTP methods.
- Edit query parameters and headers in aligned, resizable Name / Value / Description columns; automatically append a blank row, toggle enabled rows, duplicate/delete rows on hover, or use bulk editing.
- Keep query parameters and the URL synchronized, including repeated keys, percent encoding, and disabled parameters.
- Send raw and URL-encoded request bodies with optional JSON formatting, Bearer Token, Basic Auth, and API Key headers or query parameters.
- Configure authentication once per collection. Requests can inherit the latest collection credentials when sent, use their own auth, or explicitly choose No Auth. Existing requests can be switched to inheritance together, including nested folders and open tabs.
- Import Postman Collection v2/v2.1 and environment JSON files or directories from the Import button or by dragging files into the app. Keep nested folders, disabled rows, inherited auth, descriptions, and original JSON. Reimporting skips duplicate request IDs.
- Select and edit environments, request/collection variables, and globals; resolve `{{variable}}` references when sending (environment overrides request/collection, which overrides globals).
- Inspect response status, elapsed time, size, JSON/raw body, headers, and response cookies; copy response content.
- Save named requests, restore open tabs, and revisit request history; workspace data is stored by eframe locally. Existing saved requests remain compatible.
- Use Ctrl+T / Ctrl+W for tabs, Ctrl+K for search, Ctrl+L for URL focus, Ctrl+S to save, and Ctrl+Enter to send.
- Requests execute on a background thread with a 120 second timeout.

The reconstruction targets the HTTP request workspace. Script execution, multipart/binary bodies, dynamic variables, and cloud collaboration are not implemented. Unsupported imported configurations remain in the original data and show compatibility messages. No imported requests are sent automatically.

## Collection authentication

Right-click a collection (including “已保存的请求”) and choose “集合身份验证”. Set Bearer Token, Basic Auth, API Key, or No Auth, then save. To switch existing requests together, select “将集合内已有的 … 条请求改为继承”; this also disables their old Authorization and matching API Key header/query rows. Independent auth settings are preserved when this option is unchecked.

Requests added from a collection or folder inherit by default. In a request's “身份验证” tab, choose “继承集合身份验证” or use “编辑集合身份验证” to update shared credentials. Updating a collection token applies on the next send, including already-open requests; no per-request resave is needed. Token fields accept `{{variable}}` references with the existing variable precedence. Credentials remain in the local workspace, and existing profiles keep their auth behavior until explicitly converted. Newly imported Postman collections retain collection inheritance and folder/request overrides; reimporting preserves locally updated collection credentials.

For headless local migration, close LinkShop first and run:

```powershell
.\linkshop.exe import "C:\exports\collections.json" "C:\exports\environment.json"
```

The importer backs up `%APPDATA%\LinkShop\data\app.ron` before merging and writes `last-import-report.json` beside it. Do not run headless import while another LinkShop instance is saving the same profile.

## Package

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\package.ps1
```

Requires Python 3.9+. Creates `dist/LinkShop-<version>-windows-x64/linkshop.exe`, a same-name ZIP and SHA-256 file, usage instructions, `package.json`, and third-party notices/licenses. Windows builds use a static MSVC runtime. Existing packages are preserved; pass `-OutputDirectory` for another destination. macOS/Linux use `python3 scripts/package.py` and produce tar.gz archives.

The same script powers release CI. Only a pushed `v<version>` tag matching Cargo.toml publishes a GitHub Release. Artifacts are unsigned portable binaries; macOS is not notarized and Linux requires system libraries. See the shared [architecture and packaging conventions](docs/architecture.md).

## Project structure

`src/main.rs` calls `app::run()`. `src/app.rs` owns desktop startup and `src/cli.rs` preserves headless import. `src/lib.rs` exports `model` and `postman`; the desktop UI consumes that library. `src/ui/mod.rs` owns application state, with navigation in `shell.rs`, request/response editing in `editor.rs`, imports/dialogs in `dialogs.rs`, and reusable controls/styles in `widgets.rs` and `theme.rs`.

## Verify

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\dev.ps1 verify
python -m unittest discover -s scripts/tests -v
```

Tests cover local TCP HTTP fixtures, live collection auth updates and request overrides, variable precedence and recursion, URL-encoded bodies and query auth, Postman hierarchy and storage migration, query synchronization, bulk editing, legacy storage compatibility, draft preservation, divider dragging, and editor/response/viewport layout combinations. Reference and visual-validation details are recorded in [docs/postman-reference.md](docs/postman-reference.md).
