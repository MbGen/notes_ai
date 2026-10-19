#define AppVersion "0.5.1"
#ifndef BundleDir
  #error BundleDir must point to a clean installer payload
#endif
#ifndef OutputDir
  #define OutputDir "..\..\dist"
#endif
[Setup]
AppId={{9AC73915-812D-4931-903F-1904F54B23D1}
AppName=Notes AI
AppVersion={#AppVersion}
AppPublisher=Notes AI
AppPublisherURL=https://github.com/MbGen/notes_ai
DefaultDirName={localappdata}\Programs\Notes AI
DisableDirPage=no
DefaultGroupName=Notes AI
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
OutputDir={#OutputDir}
OutputBaseFilename=NotesAI-{#AppVersion}-Setup-x64
SetupIconFile=..\app.ico
UninstallDisplayIcon={app}\NotesAI.exe
Compression=lzma2/normal
SolidCompression=yes
WizardStyle=modern
CloseApplications=yes
RestartApplications=no
LicenseFile=..\..\LICENSE
[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; GroupDescription: "Shortcuts:"
[Files]
Source: "{#BundleDir}\NotesAI.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#BundleDir}\*.dll"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#BundleDir}\LICENSE.txt"; DestDir: "{app}"
Source: "{#BundleDir}\README.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#BundleDir}\laya\*"; DestDir: "{app}\laya"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "initial-settings.json"; DestDir: "{app}\data"; DestName: "settings.json"; Flags: onlyifdoesntexist uninsneveruninstall
[Icons]
Name: "{group}\Notes AI"; Filename: "{app}\NotesAI.exe"; WorkingDir: "{app}"
Name: "{autodesktop}\Notes AI"; Filename: "{app}\NotesAI.exe"; WorkingDir: "{app}"; Tasks: desktopicon
[Run]
Filename: "{app}\NotesAI.exe"; Description: "Open Notes AI"; Flags: nowait postinstall skipifsilent
