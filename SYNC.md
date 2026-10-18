# Local phone sync — implementation and verification

## Connection model

The desktop process hosts an HTTP API on all IPv4 interfaces, port 8743 by default.
It is disabled until enabled in Settings. `localhost` cannot be used from a phone:
use the displayed LAN address, or another PC IPv4 when a VPN/multiple adapters
cause automatic address selection to choose the wrong interface.

Both devices must share a trusted local network. Internet access is not required
for sync. No cloud relay, public hosting, UPnP or firewall changes are included.
Windows may require allowing this application on the private network.
Closing the desktop app or sleeping the PC stops the server.

## Desktop settings

- Enable phone sync to save and start the server immediately. To change the port,
  type a number from 1024 to 65535 and press Save changes or Enter. Scrolling over
  the port field does not change its value.
- Green/red server status reflects binding the listening socket.
- Copy the address and pairing key, or scan the QR in Notes AI Phone.
- Devices show model name and online/offline status. Online means an authenticated
  successful sync within 45 seconds; it is not a permanent socket connection.
- Reset pairing key revokes the shared key for all phones. Reconnect each phone.
- QR contains the same shared secret as manual pairing; keep it private.

This first version uses HTTP plus a bearer key, suitable only for a trusted LAN.
It does not encrypt notes in transit. Do not expose the port to the internet.
A QR fills the connection form; the user presses Save connection & sync to apply.
Google Play services supplies the Android scanner and may need a module download
on its first use. Manual setup remains available.

## Android

Build without Android Studio using `powershell -ExecutionPolicy Bypass -File scripts/build-android.ps1`.
JDK 17, SDK 35 and Gradle 8.11.1 are installed under `D:\RustProjects\.notes-ai-tools\android-build`.
The script redirects SDK, Gradle, Android user files and temporary files to D.
The APK is copied to `dist/NotesAI-0.4-debug.apk`. See `android/README.md`.
Pixel 9a is the intended physical test device; no device test is claimed.

The client saves a durable local outbox before clearing the editor. It attempts
sync immediately after adding a note and every 15 seconds while the activity is
visible. When reopened it also retries. Closed-app/background sync is not yet
implemented. Server outages, Wi-Fi outages and wrong keys leave the outbox intact.
The latest PC snapshot is kept locally for offline reading.

The outbox and snapshot currently use a single private SharedPreferences JSON
state with synchronous disk commits. This is an MVP store, not the final scalable
SQLite/Room storage for a large note library. Editing, deletion and moving notes
are available offline and sync through a durable operation queue. Category unread
badges and per-device permission management are still pending.

## Protocol v1

`POST /api/sync`, header `Authorization: Bearer <pairing key>`.
Request: `{device_id, device_name, notes: [{id: UUID, text}]}`.
Response: `{accepted: [UUID], notes: [{id, text, category, date, status}]}`.
Status is `queued` or `classified`; a missing category means awaiting AI.
The client sends bounded batches and only removes IDs acknowledged by the PC.
PC acceptance inserts queue rows and durable deduplication receipts transactionally.
AI completion inserts the note and embeddings, maps its receipt, and removes its
queue row in one transaction. Failed classification/embedding leaves it queued.
Receipts remain after deletion to prevent retries resurrecting accepted notes.
Desktop notes and queue items are included in each full snapshot; deletions and
category renames are reflected in the next snapshot. Phone-origin UUID stays
stable through AI classification. Snapshot paging/change cursors are future work.

## Protocol v2: phone changes

The response adds `protocol: 2`, all `categories` (including empty ones), and
`operations: [{id, outcome}]`. Requests optionally include operations with a UUID,
target wire ID, action (`update` or `delete`), full base snapshot, desired text
and optional category. Old clients continue using uploads and snapshots.

Upload acceptance, edits, operation receipts and the response snapshot share one
transaction. Repeated operation IDs return the recorded outcome. Current text,
category, status and date must match the base snapshot. Otherwise the PC returns
`conflict` or `missing` and keeps its version. The phone retains the proposed
change and offers Keep PC, Apply mine, or Save my copy if the PC note was deleted.
Resolving against an old displayed version can conflict again rather than silently
overwrite a newer PC change. Deleting an already missing note is acknowledged.

The phone overlays pending changes for offline reading and hides pending deletions.
One outstanding operation per note is allowed; its controls stay disabled until
acknowledgement. Other notes remain editable. Old desktop servers never acknowledge
edits: the phone retains them and asks for a PC update. New-note uploads always
precede operations targeting them in the same batch. Text changes invalidate
embeddings, and stale AI/embedding results cannot restore older text or deleted notes.
Edits preserve the original note date. Manual collection entry can create a new class.

## Automatic classification lookup

Enabled by default following the requested opt-in for this update; disable
"Use web context for unfamiliar names" in PC Preferences and save to turn it off.
The model first returns a fallback category and optionally a short literal public
name requiring context. Validated terms (up to 80 characters / six words, without
URL/email/contact-number patterns) are searched once with DuckDuckGo. Up to three
title/snippet results are treated as untrusted context, never executable commands.
No result pages are fetched or browser windows opened. Results, including failures,
are cached in RAM for up to 24 hours with a 128-entry limit. Failed lookup uses
the fallback category; failed Ollama/embeddings leave the original note queued.
Desktop Capture displays the processing phase. Live model lookup behavior and
the new phone action dialogs still need physical-device/manual validation.

## Verification status

The desktop now has an isolated Rust/GNU toolchain available in the local workspace;
its dependencies are resolved in Cargo.lock. Rust tests cover settings migration,
transactional classification, unread-count persistence and resets, authenticated
sync validation, idempotent uploads before/after classification and deletion.
The final validation results for the atelier update are recorded in UPDATE.md.

Android APK compilation and 13 local sync/edit/date-order tests pass. Physical checks remain:

1. Install the debug APK on Pixel 9a.
2. Pair manually and through QR; verify incorrect key rejection.
3. Create notes offline, restart the phone app, reconnect; each appears once on PC.
4. Interrupt transfer after PC commits but before response; retry must not duplicate.
5. Stop Ollama, send notes, restart Ollama; queue eventually classifies.
6. Create/delete desktop notes; confirm snapshots update phone and remain offline.
7. Stop/start server, change port, reset key; verify status and reconnect behavior.
8. Validate private-network firewall access on the actual PC.

Offscreen UI previews contain fictional content and do not exercise the network,
Ollama inference, clipboard, or Android behaviour.
