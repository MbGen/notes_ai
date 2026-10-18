# Desktop atelier update — 2026-10-18

## Delivered

- Replaced the desktop UI with a shared graphite / ivory / champagne design system,
  serif headings, vector navigation icons, deliberate spacing, hover feedback and
  keyboard activation for action buttons. Capture, Library, collection detail,
  Discover and Preferences use the same components.
- Added persistent per-collection unread counts and a navigation total. Existing
  notes are marked read on migration; subsequent inserts, including classified
  phone notes, are tracked by a database trigger. Opening a collection marks its
  notes seen. Deletes remove unread activity through foreign-key cascading.
- Added a latest-filed strip and a notice containing the category and note preview.
  View opens the category. A note filed before this session is shown silently.
- Added DuckDuckGo internet search inside Discover, alongside semantic note search.
  The client validates and unwraps source links, skips ad cards, distinguishes
  empty results from provider challenges, and offers a browser fallback.
- Saving a source puts its title, snippet and URL in the existing AI queue. The
  result card becomes Saved only after a successful database write.
- Search request generations prevent old responses replacing a newer query or
  switching the user away from another screen. Mode changes cancel old updates.
- Added independent Ollama health/model checks. Web search and saving to the queue
  remain available without Ollama. Settings changes apply to the next AI operation.
- Updated queue processing to commit categorisation, embeddings and receipt mapping
  atomically. Failed attempts retain the queue item; retry rotation prevents one
  failing note starving later arrivals.
- Font rendering correction: restored `renderer-winit-skia`, the renderer feature
  present in commit 6734cd0. The temporary switch to FemtoVG during the redesign
  was reverted after the user reported uneven text on Windows. Offscreen software
  previews do not validate the live Skia font rendering.

## Validation

- Slint 1.14.1 Python compiler: the final UI compiles.
- Cargo check: passes with the resolved Slint 1.15.1 dependency.
- Cargo test: 8 offline tests pass; the network test is ignored in this suite.
- Explicit live DuckDuckGo test: passes; the synthetic Rust query returned 10 sources.
- Cargo build: debug Windows desktop binary built using the isolated GNU toolchain.
- Five real Slint screens rendered offscreen at 1180×820 and 940×720; inspected
  previews and refined alignment, input styling and settings column proportions.
- git diff --check: passes.

Previews in screenshots/atelier-*.png use fictional fixture content. They do not
read personal notes, start the server, or demonstrate real Android/Ollama activity.
The temporary-database tests do not change data/notes.db.

## Open the local desktop build

Double-click start-desktop.cmd in the project directory. It runs
`target/x86_64-pc-windows-msvc/debug/notes_ai.exe` with the correct working directory for the existing
`data/` folder. The binary is a local debug build, not an installer/release package.
On another checkout, build it with `cargo build --target x86_64-pc-windows-msvc` first.

## Remaining boundaries

- Real keyboard/clipboard interactions and the GPU-rendered live desktop app still
  need a user smoke test. Offscreen rendering exercises layout, not those behaviours.
- Android still has its initial native interface; no premium Android redesign or
  APK/device validation is included here. See SYNC.md.
- Internet results rely on DuckDuckGo HTML markup and can be rate-limited or
  challenged. The integration is not an official search API.
- View from the latest-filed notice opens the collection without scrolling to the
  exact note. Unread state currently belongs to the desktop library, not each phone.
- A changed embedding model does not automatically regenerate the existing index.
- Import/export were implemented in the follow-up below; selected-collection export remains pending.


## Font rendering follow-up

The user reported uneven small text/input glyphs in the redesign. Local history
contains two commits; commit 6734cd0 already enables `renderer-winit-skia`.
The redesign's switch to FemtoVG was reverted. The launcher now sets
`SLINT_BACKEND=winit-skia` locally and opens the new MSVC binary, not the earlier
GNU/FemtoVG executable in target/debug.

The Skia/MSVC build succeeds and all 8 offline tests pass on that target.
A native Skia test window rendered successfully at Windows scale 1.25
(1475×1025 pixels); its own renderer snapshot was inspected, including the model
inputs and small grey text. This is separate from the earlier software previews.
The native probe uses fictional settings and never reads the user's database.
FemtoVG's snapshot output was black in this probe, so it was not used as a valid
visual A/B comparison. Final confirmation in the user's full-size window is still
needed, especially the scrolled Reset control.


## Backup restoration and data transfer follow-up

Implemented SQLite/JSON import with a preview and merge confirmation, automatic
pre-import SQLite safety snapshots, and SQLite/JSON/Markdown export through native
file dialogs in Preferences. Exports are written atomically and cannot target the
active database. Imported historical notes are marked read. Dates, text, category
membership and compatible search vectors are preserved. Original backup files are
opened through an immutable read-only SQLite URI.

Added 6 transfer tests: preservation of current notes and intentional duplicates,
queue receipt persistence after AI processing, JSON/SQLite round trips, invalid
archive rejection, model mismatch/reindex handling, and safety snapshot contents.
All 14 offline tests pass on Windows/MSVC; the external search test remains opt-in.
The AppContainer prevents tempfile atomic renames, so the file-transfer tests were
run in the normal Windows process context using temporary fixture databases.

Restored the requested notes_old.db: 88 notes / 14 collections / 88 vectors were
merged with the current 5 notes / 2 collections. Final state is 93 notes, 16
collections and 93 vectors, with an empty queue. Exact note text and dates and
category membership were compared to the union of both inputs. The source file's
SHA-256 was unchanged. Both a new SQLite backup and JSON export were generated
under data/exports/ and successfully read back through the importer. A copy of the
original current database is retained under data/backups/.

The current Skia/MSVC binary includes the transfer UI and headless maintenance
commands. File dialog interaction still needs the user's manual smoke test.

Android follow-up (2026-10-18): built dist/NotesAI-0.1-debug.apk using JDK/SDK/Gradle and caches on D, without Android Studio. Six sync-state tests pass; lint has zero errors and two adaptive-icon advisory warnings. APK v2 signature and zip alignment verified. Physical Pixel 9a checks remain pending; sync runs while the app is visible. See android/README.md.


Android Atelier 0.2 (2026-10-18): native graphite/ivory/gold UI, serif headings,
Capture composer and outbox preview, Library with local text search and category
filters, Connect with QR/manual pairing, connection indicator and bottom navigation.
Draft and selected tab survive Activity recreation. Existing preference keys,
state schema, package ID and signing key are retained; versionCode increased to 2.
Output: dist/NotesAI-0.2-debug.apk. Build, six sync-state tests, lint (zero errors),
APK signature and zip alignment passed. Visual/device validation on Pixel remains
pending. No background-sync change in this update.

Collections and date sorting (Android 0.3): Library now starts with collection
cards, ordered by last opening (persisted locally). Notes are displayed only
inside the selected collection; Back returns to collection cards. Search applies
to collection names in the overview and note text inside a collection. A dedicated
Date button switches newest/oldest order; Android remembers the choice. Desktop
collection detail also has a separate Date button. Sorting parses legacy English,
ISO and numeric dates; undated/unrecognized notes stay last, without rewriting
stored dates. New phone outbox notes record a local creation timestamp.
Android: eight tests passed, lint has no errors, APK signature/alignment verified.
Rust date-order regression test passed. Physical Pixel UI checks remain pending.

Android 0.4 + desktop operation sync (2026-10-18): note text editing, manual
collection selection/creation and confirmed deletion; durable offline operation
queue and optimistic display; explicit conflicts preserving both intents. Protocol
v2 accepts old clients; old PC versions do not discard pending phone edits. Uploads,
mutations, receipts and snapshots are transactional. Stale classification/embedding
results cannot overwrite changed/deleted notes. Original dates remain unchanged.
Automatic classification can look up an unfamiliar literal name via DuckDuckGo,
with bounded snippets/cache, one lookup and fallback, plus a Preferences toggle
and desktop phase display. No browser window or result-page fetching is involved.
Validation: 19 Rust tests passed (one live web test ignored); 13 Android tests
passed; lint zero errors (eight advisory warnings); APK signature/alignment pass.
No live model-driven lookup or physical Pixel action-dialog test is claimed.
The running desktop locks its main executable; newly linked binary is staged at
dist/notes_ai-update.exe and start-desktop.cmd installs it after the old app closes.
