[Setup]
AppName=ConnectSync
AppVersion=0.1.0
DefaultDirName={autopf}\ConnectSync
DefaultGroupName=ConnectSync
UninstallDisplayIcon={app}\connect_sync.exe
Compression=lzma2
SolidCompression=yes
OutputDir=Output
OutputBaseFilename=ConnectSync_Setup

; Modern ve temiz görünüm
WizardStyle=modern
; Koyu tema efekti vermek için (Inno Setup 6.1+ destekliyorsa veya VCL stili eklendiğinde)
; Burada temel pencere renklerini koyu gri / lacivert tonlarına ayarlıyoruz.
WindowVisible=yes
WindowShowCaption=no
WindowResizable=no
BackColor=$322a24  ; Koyu Lacivert/Gri tonu (RGB: 36, 42, 50 - BGR formatinda $322A24)
BackColor2=$322a24
BackColorDirection=toptobottom

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
; Derlenmiş .exe dosyasının yolu. Windows'ta derledikten sonra burayı düzeltin:
Source: "target\release\connect_sync.exe"; DestDir: "{app}"; Flags: ignoreversion
; Source: "logo.png"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\ConnectSync"; Filename: "{app}\connect_sync.exe"
Name: "{autodesktop}\ConnectSync"; Filename: "{app}\connect_sync.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\connect_sync.exe"; Description: "ConnectSync Uygulamasını Başlat"; Flags: nowait postinstall skipifsilent
