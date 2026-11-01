# 构建 WiX MSI 安装包
# 依赖：dist\TranslatorBot\TranslatorBot.exe（宿主）+ dist\TranslatorBotBackend\（PyInstaller 后端）
# WiX 3.14 二进制会自动下载到 C:\Temp\wix（无需 .NET SDK）
param(
    [string]$Version = "0.0.2"
)

$ErrorActionPreference = "Stop"
$repo = (Resolve-Path "$PSScriptRoot").Path
$wixVersion = "wix3141rtm"
$wixZipUrl = "https://github.com/wixtoolset/wix3/releases/download/$wixVersion/wix314-binaries.zip"
$wixDir = "C:\Temp\wix"
$stage = "C:\Temp\tb-stage"
$obj = "C:\Temp\tb-obj"

$hostExe = Join-Path $repo "dist\TranslatorBot\TranslatorBot.exe"
$backendDir = Join-Path $repo "dist\TranslatorBotBackend"
if (-not (Test-Path $hostExe)) { throw "缺少 $hostExe，请先运行 build-host.ps1" }
if (-not (Test-Path $backendDir)) { throw "缺少 $backendDir，请先用 PyInstaller 打包后端" }

# WiX
if (-not (Test-Path "$wixDir\candle.exe")) {
    Write-Host "下载 WiX 3.14 ..."
    New-Item -ItemType Directory -Force $wixDir | Out-Null
    $zip = "$env:TEMP\wix314.zip"
    Invoke-WebRequest -Uri $wixZipUrl -OutFile $zip
    Expand-Archive $zip -DestinationPath $wixDir -Force
}
$candle = Join-Path $wixDir "candle.exe"
$light = Join-Path $wixDir "light.exe"
$heat = Join-Path $wixDir "heat.exe"

# 暂存目录（纯英文路径，避免 WiX 处理中文路径出错）
Remove-Item -Recurse -Force $stage, $obj -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force "$stage\backend", $obj | Out-Null
Copy-Item $hostExe "$stage\TranslatorBot.exe" -Force
Copy-Item "$backendDir\*" "$stage\backend\" -Recurse -Force
if (Test-Path "$repo\dist\updater.exe") { Copy-Item "$repo\dist\updater.exe" "$stage\backend\updater.exe" -Force }

# 采集后端文件
& $heat dir "$stage\backend" -cg BackendFiles -dr BackendDir -gg -g1 -sfrag -srd `
    -var var.BackendSourceDir -out "$obj\backend.wxs" | Out-Null
if ($LASTEXITCODE -ne 0) { throw "heat 失败" }

# 编译 / 链接
& $candle -nologo -dStageDir="$stage" -dBackendSourceDir="$stage\backend" -dProductVersion="$Version" -out "$obj\" `
    "$repo\installer\Product.wxs" "$obj\backend.wxs"
if ($LASTEXITCODE -ne 0) { throw "candle 失败" }

$msi = Join-Path $repo "dist\TranslatorBot-Setup-$Version.msi"
& $light -nologo -ext WixUIExtension -loc "$repo\installer\zh-cn.wxl" -sval -o $msi "$obj\Product.wixobj" "$obj\backend.wixobj"
if ($LASTEXITCODE -ne 0) { throw "light 失败" }

Write-Host ("生成: {0} ({1:N1} MB)" -f $msi, ((Get-Item $msi).Length / 1MB))
