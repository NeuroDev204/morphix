# Kiểm morphix-gui.exe đã nhúng UI đã build (ui\dist) thay vì nạp dev server; bản Windows của check-gui-embeds-ui.sh.
# WHY: thiếu feature custom-protocol thì Tauri nạp devUrl localhost:5173 và cửa sổ báo "không kết nối được".
# Dùng: powershell -File scripts\ci\check-gui-embeds-ui.ps1 -Binary target\release\morphix-gui.exe
param([Parameter(Mandatory)] [string] $Binary)
$ErrorActionPreference = "Stop"
if (-not (Test-Path $Binary)) { throw "không thấy binary: $Binary" }

# Latin1 để mỗi byte thành một ký tự, nên tìm chuỗi ASCII trong binary không bị lệch.
$text = [System.Text.Encoding]::Latin1.GetString([System.IO.File]::ReadAllBytes($Binary))
$embedded = [regex]::Matches($text, 'assets/khay-[A-Za-z0-9_-]+\.js').Count
$khay = [regex]::Matches($text, 'khay\.html').Count
# Tauri codegen vẫn ghi vài URL localhost:5173 tương đối (icons/, crates/) vào binary release; chúng không phải đích điều hướng.
$dev = @([regex]::Matches($text, 'http://localhost:5173[A-Za-z0-9_./@-]*') |
    Where-Object { $_.Value -notmatch '^http://localhost:5173/(icons|crates)/' })

if ($embedded -eq 0 -or $khay -eq 0) {
    throw "FAIL: $Binary không nhúng ui/dist (assets/khay-*.js: $embedded, khay.html: $khay)"
}
if ($dev.Count -gt 0) {
    throw "FAIL: $Binary còn URL dev server: $($dev[0].Value)"
}
Write-Host "OK: $Binary nhúng ui/dist (assets/khay-*.js: $embedded, khay.html: $khay), không có URL dev server."
