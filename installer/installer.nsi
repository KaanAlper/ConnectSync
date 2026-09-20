; ConnectSync Windows installer (NSIS, Modern UI 2)
Unicode true
!define APP_NAME "ConnectSync"
!define APP_EXE  "connect_sync.exe"      ; cargo binary adı
!ifndef VERSION
  !define VERSION "0.0.0"
!endif

Name "${APP_NAME}"
OutFile "ConnectSync-Setup-${VERSION}.exe"
InstallDir "$LOCALAPPDATA\Programs\${APP_NAME}"   ; admin gerektirmez
RequestExecutionLevel user
SetCompressor /SOLID lzma

!include "MUI2.nsh"
!define MUI_ICON   "..\assets\icon.ico"
!define MUI_UNICON "..\assets\icon.ico"
!define MUI_BGCOLOR "1E1E2E"
!define MUI_TEXTCOLOR "CDD6F4"
!define MUI_FINISHPAGE_RUN "$INSTDIR\${APP_EXE}"
!define MUI_FINISHPAGE_RUN_TEXT $(FinishPageText)

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"
!insertmacro MUI_LANGUAGE "Turkish"
!insertmacro MUI_LANGUAGE "German"
!insertmacro MUI_LANGUAGE "Spanish"
!insertmacro MUI_LANGUAGE "French"
!insertmacro MUI_LANGUAGE "SimpChinese"
!insertmacro MUI_LANGUAGE "Korean"
!insertmacro MUI_LANGUAGE "Japanese"

LangString FinishPageText ${LANG_ENGLISH} "Launch ConnectSync"
LangString FinishPageText ${LANG_TURKISH} "ConnectSync'i başlat"
LangString FinishPageText ${LANG_GERMAN} "ConnectSync starten"
LangString FinishPageText ${LANG_SPANISH} "Iniciar ConnectSync"
LangString FinishPageText ${LANG_FRENCH} "Lancer ConnectSync"
LangString FinishPageText ${LANG_SIMPCHINESE} "启动 ConnectSync"
LangString FinishPageText ${LANG_KOREAN} "ConnectSync 시작"
LangString FinishPageText ${LANG_JAPANESE} "ConnectSync を起動"

# Dil secici diyalog
Function .onInit
  !insertmacro MUI_LANGDLL_DISPLAY
FunctionEnd


Section "Install"
  SetOutPath "$INSTDIR"
  File "..\target\release\${APP_EXE}"
  WriteUninstaller "$INSTDIR\Uninstall.exe"
  CreateDirectory "$SMPROGRAMS\${APP_NAME}"
  CreateShortcut "$SMPROGRAMS\${APP_NAME}\${APP_NAME}.lnk" "$INSTDIR\${APP_EXE}"
  CreateShortcut "$DESKTOP\${APP_NAME}.lnk" "$INSTDIR\${APP_EXE}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}" "DisplayName" "${APP_NAME}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}" "DisplayIcon" "$INSTDIR\${APP_EXE}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}" "UninstallString" "$INSTDIR\Uninstall.exe"
SectionEnd

Section "Uninstall"
  Delete "$INSTDIR\${APP_EXE}"
  Delete "$INSTDIR\Uninstall.exe"
  RMDir "$INSTDIR"
  Delete "$DESKTOP\${APP_NAME}.lnk"
  Delete "$SMPROGRAMS\${APP_NAME}\${APP_NAME}.lnk"
  RMDir "$SMPROGRAMS\${APP_NAME}"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}"
SectionEnd
