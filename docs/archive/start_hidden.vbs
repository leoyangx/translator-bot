' 无窗口启动机器人（开机自启用的就是它）
Option Explicit
Dim fso, sh, dir
Set fso = CreateObject("Scripting.FileSystemObject")
Set sh = CreateObject("WScript.Shell")
dir = fso.GetParentFolderName(WScript.ScriptFullName)
sh.CurrentDirectory = dir
sh.Run """.venv\Scripts\pythonw.exe"" -m app.supervisor", 0, False
