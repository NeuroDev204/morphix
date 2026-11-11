# Tải ffmpeg, PDFium và bootstrapper WebView2 đã ghim trong packaging\windows-deps.txt, kiểm SHA256 hoặc chữ ký,
# rồi đặt vào installer\bundle, installer\store-package và third_party\pdfium\lib (cho cargo test).
# Dùng chung cho CI và máy dựng Windows: powershell -File scripts\ci\setup-windows-deps.ps1
$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"

$root = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
$deps = @{}
foreach ($line in Get-Content (Join-Path $root "packaging\windows-deps.txt")) {
    if ($line -match '^\s*([A-Z0-9_]+)=(.+?)\s*$') { $deps[$Matches[1]] = $Matches[2] }
}
foreach ($name in "FFMPEG_URL", "FFMPEG_SHA256", "PDFIUM_URL", "PDFIUM_SHA256", "WEBVIEW2_URL") {
    if (-not $deps.ContainsKey($name)) { throw "packaging\windows-deps.txt thiếu $name" }
}

$work = Join-Path ([System.IO.Path]::GetTempPath()) ("morphix-deps-" + [guid]::NewGuid())
New-Item -ItemType Directory -Path $work | Out-Null

function Get-Pinned([string] $Url, [string] $Sha256, [string] $Destination) {
    Invoke-WebRequest -Uri $Url -OutFile $Destination -UseBasicParsing
    $actual = (Get-FileHash -Algorithm SHA256 -Path $Destination).Hash.ToLowerInvariant()
    if ($actual -ne $Sha256.ToLowerInvariant()) {
        throw "SHA256 của $Url không khớp: cần $Sha256, tải về $actual"
    }
}

$bundle = Join-Path $root "installer\bundle"
$store = Join-Path $root "installer\store-package"
$pdfiumLib = Join-Path $root "third_party\pdfium\lib"
New-Item -ItemType Directory -Force -Path $bundle, $store, $pdfiumLib | Out-Null

# PDFium: pdfium.dll nằm trong bin\ của gói tải về.
$pdfiumTgz = Join-Path $work "pdfium.tgz"
Get-Pinned $deps.PDFIUM_URL $deps.PDFIUM_SHA256 $pdfiumTgz
$pdfiumDir = Join-Path $work "pdfium"
New-Item -ItemType Directory -Path $pdfiumDir | Out-Null
tar -xzf $pdfiumTgz -C $pdfiumDir
if ($LASTEXITCODE -ne 0) { throw "không giải nén được PDFium (tar thoát $LASTEXITCODE)" }
$pdfiumDll = Join-Path $pdfiumDir "bin\pdfium.dll"
if (-not (Test-Path $pdfiumDll)) { throw "gói PDFium không có bin\pdfium.dll" }
foreach ($destination in $bundle, $store, $pdfiumLib) { Copy-Item $pdfiumDll (Join-Path $destination "pdfium.dll") -Force }

# ffmpeg: chỉ lấy ffmpeg.exe (Morphix không dùng ffprobe hay ffplay).
$ffmpegZip = Join-Path $work "ffmpeg.zip"
Get-Pinned $deps.FFMPEG_URL $deps.FFMPEG_SHA256 $ffmpegZip
$ffmpegDir = Join-Path $work "ffmpeg"
Expand-Archive -Path $ffmpegZip -DestinationPath $ffmpegDir
$ffmpegExe = Get-ChildItem -Path $ffmpegDir -Recurse -Filter "ffmpeg.exe" | Select-Object -First 1
if (-not $ffmpegExe) { throw "gói ffmpeg không có ffmpeg.exe" }
foreach ($destination in $bundle, $store) { Copy-Item $ffmpegExe.FullName (Join-Path $destination "ffmpeg.exe") -Force }

# WebView2 bootstrapper: không ghim được hash, nên bắt buộc chữ ký hợp lệ của Microsoft.
$webview = Join-Path $bundle "MicrosoftEdgeWebview2Setup.exe"
Invoke-WebRequest -Uri $deps.WEBVIEW2_URL -OutFile $webview -UseBasicParsing
$signature = Get-AuthenticodeSignature -FilePath $webview
if ($signature.Status -ne "Valid" -or $signature.SignerCertificate.Subject -notmatch "O=Microsoft Corporation") {
    throw "bootstrapper WebView2 có chữ ký không hợp lệ hoặc không phải của Microsoft Corporation: $($signature.Status)"
}

Remove-Item -Recurse -Force $work
Write-Host "Đã đặt ffmpeg.exe, pdfium.dll và bootstrapper WebView2."
