; Trình cài đặt Morphix cho bản exe tải về. Không cần chứng chỉ ký code và không cần quyền quản trị.
; Build: ISCC.exe /DAppVersion=<phiên bản> installer\morphix.iss (sau khi đã build release; CI làm việc này).
#ifndef AppVersion
  #error Truyền phiên bản bằng /DAppVersion=x.y.z
#endif

[Setup]
AppId={{6B2E4F1A-3C7D-4E9B-8F21-5A0D7C9E1B42}
AppName=Morphix
AppVersion={#AppVersion}
AppPublisher=Morphix
; WHY lowest: menu chuột phải đăng ký ở HKCU nên không cần quyền quản trị; {autopf} thành thư mục của người dùng.
PrivilegesRequired=lowest
DefaultDirName={autopf}\Morphix
DefaultGroupName=Morphix
OutputDir=output
OutputBaseFilename=MorphixSetup
Compression=lzma2
SolidCompression=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
UninstallDisplayIcon={app}\morphix-gui.exe
CloseApplications=yes
RestartApplications=no

[Files]
Source: "..\target\release\morphix-gui.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\target\release\morphix.exe"; DestDir: "{app}"; Flags: ignoreversion
; WHY restartreplace/uninsrestartdelete: Explorer nạp DLL này vào tiến trình của nó nên có thể đang giữ file khi cập nhật hoặc gỡ.
Source: "..\target\release\morphix_shell.dll"; DestDir: "{app}"; Flags: ignoreversion restartreplace uninsrestartdelete
Source: "bundle\ffmpeg.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "bundle\pdfium.dll"; DestDir: "{app}"; Flags: ignoreversion
Source: "bundle\PDFium-LICENSE.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "bundle\FFMPEG-NOTICE.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\LICENSE"; DestDir: "{app}"; DestName: "LICENSE.txt"; Flags: ignoreversion
Source: "..\packaging\LICENSES-THIRD-PARTY.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "bundle\MicrosoftEdgeWebview2Setup.exe"; DestDir: "{tmp}"; Flags: deleteafterinstall; Check: not WebView2Installed

[Icons]
; WHY AppUserModelID trùng `identifier` của Tauri: Windows chỉ hiện thông báo của app có lối tắt mang đúng ID này.
Name: "{group}\Morphix"; Filename: "{app}\morphix-gui.exe"; AppUserModelID: "vn.morphix.app"

[Run]
; Morphix cần WebView2 Runtime để mở cửa sổ; Windows 11 có sẵn, Windows 10 có thể thiếu.
Filename: "{tmp}\MicrosoftEdgeWebview2Setup.exe"; Parameters: "/silent /install"; StatusMsg: "Đang cài WebView2 Runtime..."; Flags: waituntilterminated; Check: not WebView2Installed
; Đăng ký menu chuột phải và tự chạy cùng Windows cho người dùng đang cài (HKCU).
Filename: "{app}\morphix.exe"; Parameters: "install-integration"; StatusMsg: "Đang đăng ký menu chuột phải..."; Flags: runhidden waituntilterminated

[UninstallRun]
Filename: "{sys}\taskkill.exe"; Parameters: "/F /IM morphix-gui.exe"; Flags: runhidden waituntilterminated skipifdoesntexist; RunOnceId: "MorphixStopGui"
Filename: "{app}\morphix.exe"; Parameters: "uninstall-integration"; Flags: runhidden waituntilterminated; RunOnceId: "MorphixUninstallIntegration"

[Code]
const
  WebView2Client = '{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}';

function WebView2Installed: Boolean;
var
  Version: String;
begin
  // WHY hai nơi: bản cài cho cả máy nằm ở HKLM (khung nhìn 32-bit), bản cài cho người dùng nằm ở HKCU.
  Result :=
    (RegQueryStringValue(HKLM32, 'SOFTWARE\Microsoft\EdgeUpdate\Clients\' + WebView2Client, 'pv', Version) or
     RegQueryStringValue(HKCU, 'Software\Microsoft\EdgeUpdate\Clients\' + WebView2Client, 'pv', Version)) and
    (Version <> '') and (Version <> '0.0.0.0');
end;
