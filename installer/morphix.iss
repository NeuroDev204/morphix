; Trình cài đặt Morphix. Build: ISCC.exe installer\morphix.iss (sau khi build release và sign-package.ps1).
#define AppVersion "0.1.0"
#define CertThumbprint "7CED6128E137D6F4F814D8F540B57E79D2355983"

[Setup]
AppId={{6B2E4F1A-3C7D-4E9B-8F21-5A0D7C9E1B42}
AppName=Morphix
AppVersion={#AppVersion}
AppPublisher=Morphix
DefaultDirName={autopf}\Morphix
DefaultGroupName=Morphix
OutputDir=output
OutputBaseFilename=MorphixSetup
Compression=lzma2
SolidCompression=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
PrivilegesRequired=admin
UninstallDisplayIcon={app}\morphix-gui.exe

[Files]
Source: "..\target\release\morphix-gui.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\target\release\morphix.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\target\release\morphix_shell.dll"; DestDir: "{app}"; Flags: ignoreversion
Source: "package\morphix-shell.msix"; DestDir: "{app}"; Flags: ignoreversion
Source: "morphix-dev.cer"; DestDir: "{app}"; Flags: ignoreversion
Source: "register-package.ps1"; DestDir: "{app}"; Flags: ignoreversion
Source: "bundle\ffmpeg.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "bundle\pdfium.dll"; DestDir: "{app}"; Flags: ignoreversion
Source: "bundle\PDFium-LICENSE.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "bundle\FFMPEG-NOTICE.txt"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\Morphix"; Filename: "{app}\morphix-gui.exe"

[Run]
; Quyền admin chỉ dùng để tin cậy chứng chỉ ở kho máy (cần cho gói sparse).
Filename: "{sys}\certutil.exe"; Parameters: "-addstore Root ""{app}\morphix-dev.cer"""; Flags: runhidden waituntilterminated
Filename: "{sys}\certutil.exe"; Parameters: "-addstore TrustedPeople ""{app}\morphix-dev.cer"""; Flags: runhidden waituntilterminated
; Đăng ký gói cho người dùng đang cài (không chạy bằng admin).
Filename: "{sys}\WindowsPowerShell\v1.0\powershell.exe"; Parameters: "-NoProfile -ExecutionPolicy Bypass -File ""{app}\register-package.ps1"" -AppDir ""{app}"""; Flags: runhidden waituntilterminated runasoriginaluser

[UninstallRun]
Filename: "{sys}\WindowsPowerShell\v1.0\powershell.exe"; Parameters: "-NoProfile -ExecutionPolicy Bypass -File ""{app}\register-package.ps1"" -Remove"; Flags: runhidden waituntilterminated
Filename: "{sys}\certutil.exe"; Parameters: "-delstore Root {#CertThumbprint}"; Flags: runhidden waituntilterminated
Filename: "{sys}\certutil.exe"; Parameters: "-delstore TrustedPeople {#CertThumbprint}"; Flags: runhidden waituntilterminated
