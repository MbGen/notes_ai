# Build and package

Windows x64 uses Rust MSVC, the Windows SDK (`rc.exe`) and native build tools.

```powershell
cargo build --release --target x86_64-pc-windows-msvc
./scripts/package-windows.ps1
```

The resource compiler embeds the Windows icon/version metadata; Slint embeds UI
images. SQLite is compiled into the executable. Release GUI launches use the
executable's folder for `data/`; command-line maintenance retains the caller's
working directory. Extract to a writable folder, not Program Files. Existing users
can import their SQLite/JSON backup through Preferences. Personal databases,
pairing keys, local SDK paths and Android signing keys are excluded from Git and
public release packages.

The packaging script creates a fresh staging directory, copies a whitelist of
files, and produces `dist/NotesAI-0.4.0-windows-x64.zip` plus a standalone EXE.
Pass `-RuntimeDirectory` pointing at a Microsoft Visual C++ runtime directory to
bundle application-local runtime DLLs in the ZIP. Without these DLLs, the host
must already have the Visual C++ runtime. Ollama/models are installed separately.

Android:

```powershell
./scripts/build-android.ps1 -Tasks @('assembleDebug','testDebugUnitTest','lintDebug')
```

The current Android APK uses versionCode 4 / versionName 0.4 and the existing
local debug signing key, so it can update prior locally installed builds. It is
not a Play Store release. See `android/README.md` and `SYNC.md` for limitations.
