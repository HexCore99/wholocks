#ifndef MyAppVersion
  #define MyAppVersion "0.9.0"
#endif

#define MyAppName "WhoLocks"
#define MyAppPublisher "HexCore99"
#define MyAppUrl "https://github.com/HexCore99/wholocks"
#define MyAppExeName "WhoLocks.exe"

[Setup]
AppId={{812E39C7-EEAB-4A65-937E-9A793FD668E1}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppUrl}
AppSupportURL={#MyAppUrl}/issues
AppUpdatesURL={#MyAppUrl}/releases
DefaultDirName={localappdata}\Programs\WhoLocks
DefaultGroupName=WhoLocks
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir=output
OutputBaseFilename=WhoLocks-Setup-v{#MyAppVersion}
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
ChangesAssociations=yes
CloseApplications=force
RestartApplications=no
UninstallDisplayIcon={app}\{#MyAppExeName}
VersionInfoVersion={#MyAppVersion}.0
VersionInfoCompany={#MyAppPublisher}
VersionInfoDescription=WhoLocks installer
VersionInfoProductName={#MyAppName}
VersionInfoProductVersion={#MyAppVersion}

[Files]
Source: "..\target\release\wholocks-gui.exe"; DestDir: "{app}"; DestName: "{#MyAppExeName}"; Flags: ignoreversion
Source: "..\target\release\wholocks.exe"; DestDir: "{app}"; DestName: "WhoLocks-cli.exe"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\WhoLocks"; Filename: "{app}\{#MyAppExeName}"

[Registry]
Root: HKCU; Subkey: "Software\Classes\AllFilesystemObjects\shell\WhoLocks"; ValueType: string; ValueName: ""; ValueData: "WhoLocks"; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\AllFilesystemObjects\shell\WhoLocks"; ValueType: string; ValueName: "Icon"; ValueData: "{app}\{#MyAppExeName}"
Root: HKCU; Subkey: "Software\Classes\AllFilesystemObjects\shell\WhoLocks\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#MyAppExeName}"" ""%1"""

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "Launch WhoLocks"; Flags: nowait postinstall skipifsilent
