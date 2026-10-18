Notes AI 0.4.0 - Windows x64 portable

Extract this ZIP to a writable folder, for example D:\Apps\NotesAI.
Open NotesAI.exe. Its icon, UI images and SQLite engine are embedded.
Notes and connection settings are created in data\ beside the executable.
Keep this folder when updating. No personal notes or pairing keys are shipped.

To restore an existing library, use Preferences > Import backup and choose a
Notes AI SQLite/JSON export. Do not overwrite a database while Notes AI is open.

Ollama is required separately for AI classification and semantic search.
Default models:
  ollama pull qwen3:4b-thinking
  ollama pull nomic-embed-text-v2-moe:latest

For phone sync: enable it in Preferences and pair the Android app using the QR.
Keep both devices on the same trusted LAN. Sync uses local HTTP.
Android sync retries while its app is open; background sync is not included.

Automatic unfamiliar-name lookup sends a short name from a note to DuckDuckGo.
Disable "Use web context for unfamiliar names" in Preferences if desired.

Windows executables are not code-signed. APK is signed with the existing debug
key for compatibility with earlier locally installed Notes AI phone builds.

Source, setup and limitations: https://github.com/MbGen/notes_ai
