# 构建 Tauri 宿主（Rust）
# 依赖：rustup（stable-x86_64-pc-windows-gnu）、w64devkit 解压在 C:\Temp\w64devkit
# 说明：项目路径含中文时 windres 会失败，脚本会自动镜像到 C:\Temp 下再构建。
$ErrorActionPreference = "Stop"

$repo = (Resolve-Path "$PSScriptRoot").Path
$work = "C:\Temp\translatorbot-host-build"
$mingw = "C:\Temp\w64devkit\w64devkit\bin"
$cargo = "$env:USERPROFILE\.cargo\bin\cargo.exe"

if (-not (Test-Path $cargo)) { throw "未找到 cargo，请先安装 Rust（rustup）" }
if (-not (Test-Path "$mingw\gcc.exe")) { throw "未找到 w64devkit：$mingw" }

# 纯英文构建目录
Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force "$work\host-tauri" | Out-Null
Copy-Item "$repo\host-tauri\*" "$work\host-tauri\" -Recurse -Force
Copy-Item "$repo\assets" "$work\assets" -Recurse -Force

$env:PATH = "$env:USERPROFILE\.cargo\bin;$mingw;$env:PATH"
$env:CARGO_TARGET_DIR = "C:\Temp\tauri-target"
$env:CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER = "$mingw\gcc.exe"
$env:RUSTFLAGS = "-C target-feature=+crt-static -L native=C:\Temp\gnu-libs"
$env:CARGO_PROFILE_DEV_PANIC = "abort"
$env:CARGO_PROFILE_RELEASE_PANIC = "abort"

# Rust 的 GNU 目标会链接 -lgcc_eh，w64devkit 未提供，这里补一个空归档
New-Item -ItemType Directory -Force "C:\Temp\gnu-libs" | Out-Null
& "$mingw\ar.exe" rcs "C:\Temp\gnu-libs\libgcc_eh.a" | Out-Null

& $cargo build --release --manifest-path "$work\host-tauri\Cargo.toml"
if ($LASTEXITCODE -ne 0) { throw "构建失败" }

$out = "$env:CARGO_TARGET_DIR\release\translatorbot-app.exe"
Copy-Item $out "$repo\dist\TranslatorBot\TranslatorBot.exe" -Force
Write-Host "已更新 dist\TranslatorBot\TranslatorBot.exe"
