$ErrorActionPreference = "Stop"

$startup = [Environment]::GetFolderPath("Startup")
$lnkPath = Join-Path $startup "KoreanTranslatorBot.lnk"

if (Test-Path $lnkPath) {
    Remove-Item $lnkPath -Force
    Write-Output "REMOVED"
} else {
    Write-Output "NOT_FOUND"
}
