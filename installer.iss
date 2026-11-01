; Per-user, no admin. Build with: ISCC /DAppVersion=0.2.0 installer.iss
#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif

[Setup]
AppId={{7B3E2C54-9A1D-4F6E-8C21-5D0E6A4F1B92}
AppName=ROWMI Roam
AppVersion={#AppVersion}
AppPublisher=ROWMI
PrivilegesRequired=lowest
DefaultDirName={localappdata}\Programs\ROWMI Roam
DisableDirPage=yes
DisableProgramGroupPage=yes
OutputDir=target\release
OutputBaseFilename=roam-setup
SetupIconFile=roam.ico
UninstallDisplayIcon={app}\roam.exe
UninstallDisplayName=ROWMI Roam
Compression=lzma2
SolidCompression=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible

[Files]
Source: "target\release\roam.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "assets\fonts\*-OFL.txt"; DestDir: "{app}\licences"; Flags: ignoreversion

[Icons]
Name: "{userprograms}\ROWMI Roam"; Filename: "{app}\roam.exe"

[Run]
Filename: "{app}\roam.exe"; Description: "Open ROWMI Roam"; Flags: nowait postinstall skipifsilent

[UninstallDelete]
Type: files; Name: "{app}\roam.old"
Type: files; Name: "{app}\roam.new"
