# 构建 Tauri 宿主（Rust）
# 本地（无 Visual Studio）：默认走 GNU 工具链，需要 rustup 的 stable-x86_64-pc-windows-gnu + w64devkit
# CI（windows-latest 自带 VS）：加 -Msvc 用 MSVC 工具链，最稳
# 说明：项目路径含中文时 windres/rc 会失败，脚本会先镜像到纯英文目录再构建。
param([switch]$Msvc)

$repo = (Resolve-Path "$PSScriptRoot").Path
$work = "C:\Temp\translatorbot-host-build"
$cargo = "$env:USERPROFILE\.cargo\bin\cargo.exe"

if (-not (Test-Path $cargo)) {
    $cmd = Get-Command cargo -ErrorAction SilentlyContinue
    if ($cmd) { $cargo = $cmd.Source } else { throw "未找到 cargo，请先安装 Rust（rustup）" }
}

# 纯英文构建目录
Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force "$work\host-tauri" | Out-Null
Copy-Item "$repo\host-tauri\*" "$work\host-tauri\" -Recurse -Force -Exclude "target", "gen"
Copy-Item "$repo\assets" "$work\assets" -Recurse -Force

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
    # Rust 的 GNU 目标会链接 -lgcc_eh，w64devkit 未提供，这里补一个空归档
    New-Item -ItemType Directory -Force "C:\Temp\gnu-libs" | Out-Null
    & "$mingw\ar.exe" rcs "C:\Temp\gnu-libs\libgcc_eh.a" | Out-Null
}

& $cargo build --release --manifest-path "$work\host-tauri\Cargo.toml"
if ($LASTEXITCODE -ne 0) { throw "cargo build 失败（退出码 $LASTEXITCODE）" }

$out = "$env:CARGO_TARGET_DIR\release\translatorbot-app.exe"
if (-not (Test-Path $out)) { throw "未找到产物：$out" }
New-Item -ItemType Directory -Force "$repo\dist\TranslatorBot" | Out-Null
Copy-Item $out "$repo\dist\TranslatorBot\TranslatorBot.exe" -Force
Write-Host "已更新 dist\TranslatorBot\TranslatorBot.exe"
