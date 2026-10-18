# Notes AI for Android

Build from the repository root without Android Studio:

```powershell
./scripts/build-android.ps1 -Tasks @('assembleDebug','testDebugUnitTest','lintDebug')
```

Local tools: `D:\RustProjects\.notes-ai-tools\android-build` (JDK 17,
Gradle 8.11.1, Android SDK 35). The script keeps Gradle caches, Android user
files, temporary files and the project cache on D. It changes process variables
only. The generated `android/local.properties` is ignored by Git.

Output: `dist/NotesAI-0.4-debug.apk`. Preserve the signing key at
`D:\RustProjects\.notes-ai-tools\android-build\user-home\.android\debug.keystore`
to install subsequent debug builds as updates.

Copy the APK to the Pixel, open it and allow installation from the chosen file
source. Android Studio and USB debugging are unnecessary for this method.
Enable phone sync in desktop settings and save, then scan its QR in the phone
app or enter the PC address, port and pairing key manually. Both devices need
network access to the PC, normally the same Wi-Fi.

The phone uses Atelier styling with Capture, Library and Connect tabs. Library opens with collections ordered by recent visits. Select a collection to read or search its notes; a separate Date button toggles newest/oldest first. Notes are saved locally before upload; sync
retries while the app is visible and when reopened. Background sync is pending.
See ../SYNC.md for protocol details and the physical-device test checklist.

Validation: APK assembled, 13 local sync/edit/date-order tests passed, APK v2 signature and
zip alignment verified. Android Lint: zero errors, eight advisory
warnings. No physical Pixel test has been performed.

Use Edit / Move on a note to update its text or select/type a collection. Delete asks for confirmation. Changes are committed locally before sync, including while offline. Pending notes wait for acknowledgement; conflicting versions are preserved for an explicit choice. Both this APK and the updated desktop must be installed for edit sync.
