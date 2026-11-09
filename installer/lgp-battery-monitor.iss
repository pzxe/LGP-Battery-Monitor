; Inno Setup script. Build with: ISCC /DAppVersion=0.1.0-beta.1 installer\lgp-battery-monitor.iss
#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif

[Setup]
AppId={{B7C2E0A4-5D3F-4C77-9A0E-6F1B2C9D41A8}
AppName=LGP Battery Monitor
AppVersion={#AppVersion}
AppPublisher=LGP Battery Monitor contributors
AppPublisherURL=https://github.com/pzxe/LGP-Battery-Monitor
AppSupportURL=https://github.com/pzxe/LGP-Battery-Monitor/issues
DefaultDirName={autopf}\LGP Battery Monitor
DefaultGroupName=LGP Battery Monitor
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir=..\dist
OutputBaseFilename=LGP-Battery-Monitor-{#AppVersion}-setup
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
UninstallDisplayIcon={app}\lgp-battery-monitor.exe
SetupIconFile=..\assets\icon.ico
LicenseFile=..\LICENSE
CloseApplications=yes

[Files]
Source: "..\target\release\lgp-battery-monitor.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\LICENSE"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\LGP Battery Monitor"; Filename: "{app}\lgp-battery-monitor.exe"
Name: "{autodesktop}\LGP Battery Monitor"; Filename: "{app}\lgp-battery-monitor.exe"; Tasks: desktopicon

[Tasks]
Name: "desktopicon"; Description: "Create a &desktop shortcut"; Flags: unchecked
Name: "startup"; Description: "Start LGP Battery Monitor with &Windows"; Flags: unchecked

[Registry]
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "LGP Battery Monitor"; ValueData: """{app}\lgp-battery-monitor.exe"" --autostart"; Flags: uninsdeletevalue; Tasks: startup
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueName: "LGP Battery Monitor"; Flags: dontcreatekey uninsdeletevalue
Root: HKCU; Subkey: "Software\Classes\AppUserModelId\LGPBatteryMonitor"; Flags: dontcreatekey uninsdeletekey

[Run]
Filename: "{app}\lgp-battery-monitor.exe"; Description: "Launch LGP Battery Monitor"; Flags: nowait postinstall skipifsilent

[UninstallRun]
Filename: "{sys}\taskkill.exe"; Parameters: "/F /IM lgp-battery-monitor.exe"; Flags: runhidden; RunOnceId: "KillApp"
