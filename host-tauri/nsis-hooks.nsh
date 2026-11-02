!macro NSIS_HOOK_PREINSTALL
  ; 覆盖安装前先结束旧进程，避免文件占用导致安装失败
  nsExec::Exec 'taskkill /F /IM TranslatorBot.exe'
  nsExec::Exec 'taskkill /F /IM TranslatorBotBackend.exe'
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ; WebView2 加载器已静态链接进 exe，不需要额外 dll
  Delete "$INSTDIR\WebView2Loader.dll"
!macroend
