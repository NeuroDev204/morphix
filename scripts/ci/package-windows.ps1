# Dựng MorphixSetup.exe (kèm .sha256) và gói msix chưa ký cho Store từ bản release đã build.
# Cần: target\release\{morphix-gui.exe,morphix.exe,morphix_shell.dll} và các file do setup-windows-deps.ps1 đặt sẵn.
# Dùng: powershell -File scripts\ci\package-windows.ps1 -Version 0.1.2 -OutDir dist
param(
    [Parameter(Mandatory)] [string] $Version,
    [Parameter(Mandatory)] [string] $OutDir
)
$ErrorActionPreference = "Stop"

$root = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
$release = Join-Path $root "target\release"
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$OutDir = (Resolve-Path $OutDir).Path

# 1. MorphixSetup.exe bằng Inno Setup; phiên bản truyền vào, không ghi cứng trong morphix.iss.
$iscc = Join-Path ${env:ProgramFiles(x86)} "Inno Setup 6\ISCC.exe"
if (-not (Test-Path $iscc)) {
    choco install innosetup -y --no-progress
    if (-not (Test-Path $iscc)) { throw "không tìm thấy Inno Setup 6 sau khi cài" }
}
& $iscc "/DAppVersion=$Version" "/O$OutDir" (Join-Path $root "installer\morphix.iss")
if ($LASTEXITCODE -ne 0) { throw "ISCC thất bại (mã $LASTEXITCODE)" }
$setup = Join-Path $OutDir "MorphixSetup.exe"
$hash = (Get-FileHash -Algorithm SHA256 -Path $setup).Hash.ToLowerInvariant()
# WHY ghi bằng .NET: cùng dạng `sha256sum` (hash, hai dấu cách, tên file, LF), không BOM.
[System.IO.File]::WriteAllText("$setup.sha256", "$hash  MorphixSetup.exe`n")

# 2. Gói msix chưa ký để tải lên Partner Center (Store ký lại). Không gồm thư mục listing (ảnh và mô tả trang Store).
$stage = Join-Path ([System.IO.Path]::GetTempPath()) ("morphix-msix-" + [guid]::NewGuid())
New-Item -ItemType Directory -Path $stage | Out-Null
Get-ChildItem (Join-Path $root "installer\store-package") |
    Where-Object { $_.Name -ne "listing" } |
    Copy-Item -Destination $stage -Recurse
# WHY không có morphix_shell.dll: menu của bản Store chạy qua COM của morphix-gui.exe, không nạp DLL.
foreach ($name in "morphix-gui.exe", "morphix.exe") { Copy-Item (Join-Path $release $name) $stage }

$manifest = Join-Path $stage "AppxManifest.xml"
$xml = [System.IO.File]::ReadAllText($manifest)
$pattern = '(<Identity\b[^>]*\bVersion=")[^"]+(")'
if (-not [regex]::IsMatch($xml, $pattern)) { throw "AppxManifest.xml không có thuộc tính Version trong <Identity>" }
$xml = [regex]::Replace($xml, $pattern, "`${1}$Version.0`$2")
[System.IO.File]::WriteAllText($manifest, $xml, (New-Object System.Text.UTF8Encoding($true)))

$makeappx = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin\*\x64\makeappx.exe" |
    Sort-Object FullName | Select-Object -Last 1
if (-not $makeappx) { throw "không tìm thấy makeappx.exe (Windows SDK)" }
& $makeappx.FullName pack /d $stage /p (Join-Path $OutDir "Morphix-$Version.msix") /nv /o
if ($LASTEXITCODE -ne 0) { throw "makeappx thất bại (mã $LASTEXITCODE)" }
Remove-Item -Recurse -Force $stage
Write-Host "Đã dựng $setup và Morphix-$Version.msix"
