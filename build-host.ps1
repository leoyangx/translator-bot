# 构建宿主 + NSIS 安装包
# 本地（无 Visual Studio）：GNU 工具链 + w64devkit
# CI（windows-latest 自带 VS）：加 -Msvc
# 说明：项目路径含中文时 windres 会失败，脚本会先镜像到纯英文目录再构建。
param(
    [switch]$Msvc,
    [switch]$NoBundle
)

$repo = (Resolve-Path "$PSScriptRoot").Path
$work = "C:\Temp\translatorbot-build"
$cargo = "$env:USERPROFILE\.cargo\bin\cargo.exe"
$tauri = "$env:USERPROFILE\.cargo\bin\cargo-tauri.exe"

if (-not (Test-Path $cargo)) {
    $cmd = Get-Command cargo -ErrorAction SilentlyContinue
    if ($cmd) { $cargo = $cmd.Source } else { throw "未找到 cargo，请先安装 Rust（rustup）" }
}
if (-not (Test-Path $tauri)) {
    $cmd = Get-Command cargo-tauri -ErrorAction SilentlyContinue
    if ($cmd) { $tauri = $cmd.Source } else { throw "未找到 cargo-tauri，请先 cargo install tauri-cli" }
}

$backend = Join-Path $repo "dist\TranslatorBotBackend"
if (-not (Test-Path $backend)) { throw "缺少 $backend，请先用 PyInstaller 打包后端" }

# 纯英文构建目录
Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force "$work\host-tauri", "$work\dist" | Out-Null
Copy-Item "$repo\host-tauri\*" "$work\host-tauri\" -Recurse -Force -Exclude "target", "gen"
Copy-Item "$repo\assets" "$work\assets" -Recurse -Force
New-Item -ItemType Directory -Force "$work\host-tauri\web" | Out-Null
Copy-Item "$repo\app\console\web\*" "$work\host-tauri\web\" -Recurse -Force
Copy-Item $backend "$work\dist\TranslatorBotBackend" -Recurse -Force

$env:CARGO_TARGET_DIR = "C:\Temp\tauri-target"
$env:CARGO_PROFILE_DEV_PANIC = "abort"
$env:CARGO_PROFILE_RELEASE_PANIC = "abort"

if ($Msvc) {
    Write-Host "工具链：MSVC"
    $env:RUSTFLAGS = ""
    $env:CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER = $null
} else {
    $mingw = "C:\Temp\w64devkit\w64devkit\bin"
    if (-not (Test-Path "$mingw\gcc.exe")) { throw "未找到 w64devkit：$mingw（或改用 -Msvc）" }
    Write-Host "工具链：GNU ($mingw)"
    $env:PATH = "$env:USERPROFILE\.cargo\bin;$mingw;$env:PATH"
    $env:CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER = "$mingw\gcc.exe"
    $env:RUSTFLAGS = "-C target-feature=+crt-static -L native=C:\Temp\gnu-libs"
    New-Item -ItemType Directory -Force "C:\Temp\gnu-libs" | Out-Null
    & "$mingw\ar.exe" rcs "C:\Temp\gnu-libs\libgcc_eh.a" | Out-Null
}

Push-Location "$work\host-tauri"
try {
    if ($NoBundle) {
        & $cargo build --release
    } else {
        & $tauri build --bundles nsis
    }
    if ($LASTEXITCODE -ne 0) { throw "构建失败（退出码 $LASTEXITCODE）" }
} finally {
    Pop-Location
}

$target = "C:\Temp\tauri-target\release"
$exe = Join-Path $target "TranslatorBot.exe"
if (-not (Test-Path $exe)) { throw "未找到产物：$exe" }
New-Item -ItemType Directory -Force "$repo\dist\TranslatorBot" | Out-Null
Copy-Item $exe "$repo\dist\TranslatorBot\TranslatorBot.exe" -Force
Write-Host "已更新 dist\TranslatorBot\TranslatorBot.exe"

$nsisDir = Join-Path $target "bundle\nsis"
if (Test-Path $nsisDir) {
    Get-ChildItem $nsisDir -File | ForEach-Object {
        Copy-Item $_.FullName (Join-Path $repo "dist") -Force
        Write-Host ("已生成 dist\{0} ({1:N1} MB)" -f $_.Name, ($_.Length / 1MB))
    }
}
