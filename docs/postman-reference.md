# Postman request workspace reference

Reference: installed Postman 11.86.1, analyzed on 2026-10-24 with REA.

Artifact: `C:\Users\adm\AppData\Local\Postman\app-11.86.1\resources\app.asar`

SHA-256: `e140a9a5fe44205575d7cb073e373174634b216a14579e6bdef6240727f6df94`

REA inventory evidence: `ev_f83ffaf684d22e7e8eac78b05227d5f80527f3f565a73d392b5a5cba66b3ed1f`.
Focused static evidence: `ev_871539efd3956d0425d59020699e9b82c74f071899bb330b4951b3bc7c75413f`.
Selected modules: `js/scratchpad/RequesterModalContainer.js`, `js/scratchpad/CollectionSidebar.js`.
REA parsed both files, recovered 300 modules, and reported no parse failures. Source analysis was deliberately scoped; graph coverage is partial and does not establish complete application behavior.

The installed stylesheet `js/scratchpad/scratchpad.css` was also read as inert text from the ASAR. Relevant rules establish a 48px application header, a 40px collection menu, a method selector capped at 108px with a 38px inner control, a separate 100px send button group, 32px table header controls, and neutral borders with compact corner radii. The reconstruction uses these dimensions with Chinese labels and LinkShop branding.

Implemented areas:

- Search is an actual text field that filters saved requests and history by name, method, or URL.
- New request controls open independent tabs, with dirty indicators, close controls, keyboard shortcuts, and local session restoration.
- The method menu displays colored methods and accepts valid custom HTTP tokens. The method and URL share a single outlined control.
- Query parameters and headers use a common grid for headers and cells, descriptions, resizable columns, automatic blank rows, bulk edit, enabled toggles, and row hover actions.
- Query updates synchronize with the URL without appending parameters twice. Disabled rows and descriptions are retained.

During the UI reconstruction, Postman was briefly opened for an initial visual comparison, then closed after the user requested static inspection. Subsequent UI reference work used installation artifacts. The later, separately requested data migration is described below. No user API request was sent or Postman collection edited.

LinkShop visual acceptance resumed after the user requested continuation. Verified the colored method popup, query rows, disabled query synchronization, independent request tabs, session restoration, response rendering, save dialog, sidebar updates, and live search. A local echo server returned 200 and confirmed that disabled parameters were omitted and repeated keys were not duplicated. The workspace was checked at large size and in an approximately 1120px-wide native window. Temporary validation requests were removed by restoring the pre-validation workspace backup.

Visual checks uncovered and fixed an oversized logo expanding the header, duplicate IDs shared by request and response tabs, checkbox alignment, bulk-header overlap, and absolute field placement affecting later layout. The new-tab action now sits next to the tab strip and reveals newly created tabs when the strip overflows. Seven automated tests cover request semantics, compatibility, draft preservation, both dividers' drag behavior, and 48 editor/response/viewport combinations without ID clashes or panel overflow. `cargo fmt --check`, Clippy with warnings denied, and the debug build complete successfully.

This is a reconstruction of the request workspace, not a claim of complete Postman feature or pixel equivalence. Script execution, multipart/binary body sending, dynamic variables, and collaboration remain outside the implemented scope. Environments, nested collection navigation, URL-encoded bodies, and Postman imports are supported in 0.2.0.


## Postman data migration (0.2.0)

The user requested a background migration so they could keep using the computer. REA Frida read-only renderer scans recovered collection and request indexes, but those indexes did not contain every request body or auth definition. They were not treated as complete requests. A provider-owned hidden Postman process was then spawned with visibility/focus suppression hooks installed before resuming; no foreground UI automation was used for the completed extraction.

In the installed 11.86.1 runtime, loaded webpack module 619700 exports `fetchCollection(uid)`, which invokes the normal read-only Collection 2.1 export path (`findOneFromSyncWithFormat`). Environment store module 728530 exports `getOneFromSyncUncached(uid)`. These functions were read and invoked through loopback CDP in the existing authenticated app context. They returned complete collection/environment JSON without extracting session cookies or account credentials. The temporary background process and REA session were closed after export. Module numbers are specific to this installed version.

Personal exports and memory evidence stay under the user's local LinkShop migration directory, outside this repository and distributable. Import merges into the local eframe profile, preserves history/open tabs, saves original JSON, backs up the existing profile, and skips duplicate source IDs. Complete records can upgrade incomplete recovered indexes. Imported requests are never sent automatically. Runtime request execution resolves variables and supports raw/URL-encoded bodies and Bearer/Basic/API Key authentication; unsupported imported modes retain original data and show compatibility messages.

Fourteen tests cover parser fidelity, request and environment storage, backup and idempotent merge, variable precedence and recursion, local HTTP request semantics, folder/request identity, imported editors, and existing workspace regressions. The Windows release executable was additionally tested headlessly on the extracted data in an isolated validation profile. No exported data is compiled or bundled into the public ZIP.


## Collection sidebar refinement (0.2.1)

Replaced the default egui collapsing-header tree with full-width rows: 32px collection headers, 28px folders/requests, 16px indentation, 13px names, and 10.5px semibold method labels. Collection/folder icons and subtle selection/hover colors distinguish row types without vertical guide lines. Long names use an ellipsis with a complete tooltip. Expansion persists by collection/path identity; search temporarily reveals matching paths without changing that state. The active request is matched by its imported identity so editing does not remove its highlight.

Visual QA rendered a copied profile in the developer screenshot harness (`examples/ui_preview.rs`). REA spawned the owned preview process with no-activation/tool-window flags and an off-desktop position, allowing GPU framebuffer capture without moving the user's pointer or switching foreground apps. Screenshots and the copied profile remain in the local private preview directory, outside the distributable. Fifteen tests, formatting, and Clippy with warnings denied passed, including folder expansion/search state and edited-request identity.

## LinkShop brand colors (0.2.2)

The interface now follows the blue/cyan fox logo rather than the reference application's orange accents. `src/branding.rs::palette` defines cobalt actions (`#0057F0`), cyan navigation accents (`#5DE2F5`), navy text (`#091F47`), pale blue surfaces, and consistent hover/selection/border colors. Request tabs, editor tabs, sidebar selection, checkboxes, focus outlines, menus, and dialog actions use these shared colors; HTTP methods and response errors retain semantic colors.

The light theme is selected before installing the custom style so the first OS-theme event cannot replace styled native controls with egui defaults. A hidden native preview rendered a synthetic collection with selected and disabled parameter rows, request tabs, and the environment selector. The screenshot is under `target/brand-preview`, and no real profile was changed. All 15 existing tests, formatting, and Clippy with warnings denied passed.

## Context menus (0.2.3)

Collection and folder rows now have right-click menus for adding requests, opening their requests, recursive expansion/collapse, renaming, and confirmed deletion. Request rows support opening in a new tab, duplication, URL copying, renaming, and confirmed deletion. Operations use collection/request IDs and folder paths, including when search filters the visible tree; open editors remain available as drafts after deletion. The synthetic local-saved root keeps its fixed display name.

Request tabs have right-click menus for creating, saving, duplicating, and closing the clicked tab, other tabs, tabs to its right, or all tabs. Empty tab-strip space also supports creating and closing all tabs. Bulk closing queues stable tab IDs and pauses at unsaved drafts; saving closes that tab and resumes the queue, while cancel keeps the remaining drafts. Closing the final tab creates an empty request.

Headless egui pointer-event tests exercise right-click menus on collection, folder, request, and inactive tab rows; request duplication; closing tabs to the right; saving/canceling/discarding during bulk close; and folder operations with identical IDs in separate collections and unsaved edits. These checks use synthetic state and do not send requests or change the real profile.

## Collection authentication (0.2.4)

Collection roots now expose a “集合身份验证” context-menu action. The dialog edits a separate local collection-auth record keyed by collection ID, offers explicit bulk conversion of existing requests, and saves or cancels without sending requests. Request editors offer collection inheritance, independent auth, and No Auth, with a shortcut to edit the collection. Requests created from a collection/folder default to inheritance. Existing profiles retain their previous per-request settings until converted.

The send path reads the current collection configuration before resolving variables; it never copies credentials into inheriting editors or history. Bulk conversion updates saved requests and open-tab baselines while preserving unrelated dirty edits, disables old credential rows, and refreshes any bulk-edit buffers. Configured auth replaces matching manual headers/query parameters on the wire. Postman imports keep collection auth separate, preserve nested-folder/request overrides, and retain local collection-token updates on reimport. GUI and headless imports persist the same collection-auth storage key.

Synthetic local TCP fixtures verify token changes on successive sends from an already-open request, environment precedence, collection isolation, Basic Auth and No Auth overrides, and duplicate-credential removal. Headless UI tests cover opening, cancelling, and saving the dialog at small/large viewports, bulk conversion, rename-stable identity, dirty draft preservation, and persistence. No real profile or remote API is used by these tests.
