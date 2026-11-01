$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $PSScriptRoot
$pythonw = Join-Path $root ".venv\Scripts\pythonw.exe"

if (-not (Test-Path $pythonw)) {
    Write-Output "MISSING_VENV"
    exit 1
}

$startup = [Environment]::GetFolderPath("Startup")
$lnkPath = Join-Path $startup "KoreanTranslatorBot.lnk"

$shell = New-Object -ComObject WScript.Shell
$shortcut = $shell.CreateShortcut($lnkPath)
$shortcut.TargetPath = $pythonw
$shortcut.Arguments = "-m app.supervisor"
$shortcut.WorkingDirectory = $root
$shortcut.WindowStyle = 7
$shortcut.Description = "Korean translator bot"
$shortcut.Save()

Start-Process -FilePath $pythonw -ArgumentList "-m app.supervisor" -WorkingDirectory $root -WindowStyle Hidden

Write-Output "OK $lnkPath"
