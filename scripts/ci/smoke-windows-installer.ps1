# Kiểm MorphixSetup.exe như người dùng thật trên Windows (máy CI có WebView2): cài, chạy CLI, mở GUI, chạy việc qua
# đường IPC của menu chuột phải, thoát êm, gỡ cài. Quay màn hình và chụp GUI vào thư mục đầu ra.
# Usage: scripts/ci/smoke-windows-installer.ps1 -Setup <MorphixSetup.exe> -Out <thư mục>
param(
    [Parameter(Mandatory = $true)][string]$Setup,
    [Parameter(Mandatory = $true)][string]$Out
)
$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
New-Item -ItemType Directory -Force -Path $Out | Out-Null
$Out = (Resolve-Path $Out).Path
$failures = New-Object System.Collections.Generic.List[string]
function Pass([string]$what) { Write-Host "OK: $what" -ForegroundColor Green }
function Fail([string]$what) { Write-Host "LỖI: $what" -ForegroundColor Red; $failures.Add($what) }
function Check([bool]$condition, [string]$what) { if ($condition) { Pass $what } else { Fail $what } }

# ffmpeg đầy đủ (có gdigrab) chỉ để quay màn hình và tạo file mẫu; ghim SHA256 như bản Windows cũ.
$toolZip = Join-Path $env:RUNNER_TEMP "ffmpeg-full.zip"
Invoke-WebRequest -UseBasicParsing -OutFile $toolZip `
    -Uri "https://github.com/GyanD/codexffmpeg/releases/download/9.0.2/ffmpeg-9.0.2-essentials_build.zip"
if ((Get-FileHash -Algorithm SHA256 $toolZip).Hash -ne "60F467265B1E312373DBCD92200C2618A74850F98D3D078E94296BB3FA2047BA") {
    throw "ffmpeg dùng để quay sai SHA256"
}
Expand-Archive $toolZip -DestinationPath (Join-Path $env:RUNNER_TEMP "ffmpeg-full") -Force
$tool = (Get-ChildItem (Join-Path $env:RUNNER_TEMP "ffmpeg-full") -Recurse -Filter ffmpeg.exe | Select-Object -First 1).FullName

$recording = Start-Process -FilePath $tool -PassThru -WindowStyle Hidden -RedirectStandardError (Join-Path $Out "recorder.log") `
    -ArgumentList "-y -f gdigrab -framerate 8 -i desktop -c:v libx264 -preset veryfast -crf 28 -pix_fmt yuv420p `"$Out\screen.mkv`""
try {
    Write-Host "### 1. Cài $Setup"
    $setupLog = Join-Path $Out "setup.log"
    $install = Start-Process -FilePath $Setup -Wait -PassThru `
        -ArgumentList "/SILENT", "/SUPPRESSMSGBOXES", "/NORESTART", "/LOG=`"$setupLog`""
    Check ($install.ExitCode -eq 0) "bộ cài thoát 0 (thực tế $($install.ExitCode))"
    Check (Select-String -Path $setupLog -SimpleMatch "Need to restart Windows? No" -Quiet) "không đòi khởi động lại máy"

    $app = Join-Path $env:LOCALAPPDATA "Programs\Morphix"
    $morphix = Join-Path $app "morphix.exe"
    $gui = Join-Path $app "morphix-gui.exe"
    Check (Test-Path $morphix) "morphix.exe nằm ở $app"
    $version = & $morphix --version
    Check ($version -match "^morphix \d+\.\d+\.\d+") "$version"
    $doctor = & $morphix doctor | Out-String
    Write-Host $doctor
    Check ($doctor -match "windows_context_menu: installed") "menu chuột phải đã đăng ký"
    Check ($doctor -match "ffmpeg: .*ffmpeg\.exe") "tìm thấy ffmpeg.exe đi kèm"
    $dll = (Get-ItemProperty "HKCU:\Software\Classes\CLSID\{2131F98F-8888-464C-B3D7-80E2BBEEDDE1}\InprocServer32").'(default)'
    Check ((Test-Path $dll) -and ($dll -like "$app\morphix_shell_*.dll")) "DLL menu: $dll"

    Write-Host "### 2. Chuyển đổi bằng CLI"
    $work = Join-Path $env:RUNNER_TEMP "morphix-smoke"
    New-Item -ItemType Directory -Force -Path $work | Out-Null
    Copy-Item (Join-Path $Root "crates\morphix-gui\icons\128x128.png") (Join-Path $work "anh.png")
    & $tool -v error -y -f lavfi -i "testsrc=size=320x240:rate=25:duration=3" -f lavfi -i "sine=frequency=440:duration=3" -shortest "$work\video.mkv"
    Push-Location $work
    foreach ($job in @("webp anh.png", "pdf anh.png", "avif anh.png", "mp4 video.mkv", "mp3 video.mkv")) {
        $to, $file = $job.Split(" ")
        $result = & $morphix convert --to $to $file 2>&1 | Out-String
        Check ($LASTEXITCODE -eq 0 -and $result -match "^ok") "convert --to $job"
    }
    Rename-Item anh.png goc.png
    $result = & $morphix convert --to png anh.avif 2>&1 | Out-String
    Check ($LASTEXITCODE -eq 0 -and (Test-Path anh.png)) "AVIF -> PNG"
    Pop-Location

    Write-Host "### 3. Mở Morphix từ Start menu, rồi chạy việc qua đường IPC của menu chuột phải"
    $shortcut = Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs\Morphix\Morphix.lnk"
    Check (Test-Path $shortcut) "có lối tắt Start menu"
    (New-Object -ComObject Shell.Application).MinimizeAll()
    Start-Sleep -Seconds 2
    function Wait-KhayVisible {
        foreach ($i in 1..40) {
            $window = Get-Process morphix-gui -ErrorAction SilentlyContinue | Where-Object { $_.MainWindowHandle -ne 0 }
            if ($window) { return $true }
            Start-Sleep -Milliseconds 500
        }
        return $false
    }
    Start-Process -FilePath $shortcut
    Check (Wait-KhayVisible) "bấm Morphix trong Start menu hiện cửa sổ Khay"
    Start-Sleep -Seconds 3
    Add-Type -AssemblyName System.Windows.Forms, System.Drawing
    $bounds = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
    $shot = New-Object System.Drawing.Bitmap $bounds.Width, $bounds.Height
    [System.Drawing.Graphics]::FromImage($shot).CopyFromScreen($bounds.Location, [System.Drawing.Point]::Empty, $bounds.Size)
    $shot.Save((Join-Path $Out "khay.png"))
    # Đóng Khay chỉ ẩn nó; bấm lối tắt lần nữa (app đang chạy) phải hiện lại.
    Get-Process morphix-gui | Where-Object { $_.MainWindowHandle -ne 0 } | ForEach-Object { $_.CloseMainWindow() | Out-Null }
    Start-Sleep -Seconds 2
    Check ((Get-Process morphix-gui -ErrorAction SilentlyContinue).Count -eq 1) "đóng Khay chỉ ẩn, app vẫn chạy"
    Start-Process -FilePath $shortcut
    Check (Wait-KhayVisible) "bấm lại lối tắt khi app đang chạy hiện lại Khay"
    Copy-Item (Join-Path $work "goc.png") (Join-Path $work "menu.png")
    & $morphix menu --action convert:webp (Join-Path $work "menu.png")
    $made = $false
    foreach ($i in 1..30) { if (Test-Path (Join-Path $work "menu.webp")) { $made = $true; break }; Start-Sleep -Seconds 1 }
    Check $made "menu --action convert:webp qua GUI tạo menu.webp"

    & $morphix quit
    $gone = $false
    foreach ($i in 1..20) { if (-not (Get-Process morphix-gui -ErrorAction SilentlyContinue)) { $gone = $true; break }; Start-Sleep -Milliseconds 500 }
    Check $gone "morphix quit đóng GUI"

    Write-Host "### 4. Gỡ cài đặt"
    Start-Process -FilePath $gui
    Start-Sleep -Seconds 5
    $uninstall = Start-Process -FilePath (Join-Path $app "unins000.exe") -Wait -PassThru -ArgumentList "/SILENT", "/SUPPRESSMSGBOXES", "/NORESTART"
    Check ($uninstall.ExitCode -eq 0) "bộ gỡ thoát 0"
    Start-Sleep -Seconds 3
    Check (-not (Get-Process morphix-gui -ErrorAction SilentlyContinue)) "bộ gỡ đóng GUI đang chạy"
    Check (-not (Test-Path $morphix)) "đã xoá morphix.exe"
    Check (-not (Get-ChildItem $app -Filter "morphix_shell*.dll" -ErrorAction SilentlyContinue)) "đã xoá DLL menu"
    Check (-not (Test-Path "HKCU:\Software\Classes\*\shellex\ContextMenuHandlers\Morphix")) "đã xoá menu chuột phải"
    Check (-not (Get-ItemProperty "HKCU:\Software\Microsoft\Windows\CurrentVersion\Run" -Name Morphix -ErrorAction SilentlyContinue)) "đã xoá autostart"
}
finally {
    $log = Join-Path $env:LOCALAPPDATA "Morphix\data\morphix.log"
    if (Test-Path $log) { Copy-Item $log (Join-Path $Out "morphix.log") }
    Stop-Process -Id $recording.Id -ErrorAction SilentlyContinue
    Start-Sleep -Seconds 2
    & $tool -v error -y -i "$Out\screen.mkv" -c copy -movflags +faststart "$Out\screen.mp4"
}
if ($failures.Count -gt 0) { throw "$($failures.Count) kiểm tra thất bại: $($failures -join '; ')" }
Write-Host "Tất cả kiểm tra đều qua." -ForegroundColor Green
