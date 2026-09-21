#pragma code-page(65001)

[Setup]
AppName=ConnectSync
AppVersion=0.2.0
AppPublisher=Kaan Alper
AppPublisherURL=https://github.com/KaanAlper/ConnectSync
DefaultDirName={autopf}\ConnectSync
DefaultGroupName=ConnectSync
UninstallDisplayIcon={app}\connect_sync.exe
Compression=lzma2
SolidCompression=yes
OutputDir=target
OutputBaseFilename=ConnectSync_Setup
ArchitecturesAllowed=x64
ArchitecturesInstallIn64BitMode=x64

[Tasks]
Name: "desktopicon"; Description: "Masaüstüne kısayol oluştur"; GroupDescription: "Ek görevler:"

[Files]
Source: "target\release\connect_sync.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "assets\app_icon.ico"; DestDir: "{app}\assets"; Flags: ignoreversion

[Icons]
Name: "{group}\ConnectSync"; Filename: "{app}\connect_sync.exe"; IconFilename: "{app}\assets\app_icon.ico"
Name: "{commondesktop}\ConnectSync"; Filename: "{app}\connect_sync.exe"; IconFilename: "{app}\assets\app_icon.ico"; Tasks: desktopicon

[Run]
Filename: "{app}\connect_sync.exe"; Description: "ConnectSync Uygulamasını Başlat"; Flags: nowait postinstall skipifsilent
