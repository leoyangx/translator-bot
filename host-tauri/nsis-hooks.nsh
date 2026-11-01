!macro NSIS_HOOK_POSTINSTALL
  ; WebView2 加载器已静态链接进 exe，不需要额外 dll
  Delete "$INSTDIR\WebView2Loader.dll"
!macroend
