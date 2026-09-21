# -*- coding: utf-8 -*-
; ^ NSIS, BOM'suz betiği bu satır olmadan yerel ANSI kod sayfasıyla okur; Türkçe/Çince/Korece/Japonca
;   metinler bozulur. Dosyayı kaydederken UTF-8 kullan ve bu satırı (1. ya da 2. satır) silme.
;
; ConnectSync Windows installer (NSIS, Modern UI 2)
;
; Tasarım notları:
; - Kullanıcı başına kurulum ($LOCALAPPDATA\Programs, yönetici gerekmez). Uygulama içi güncelleyici
;   kendi .exe'sini değiştirebilsin diye bu şart: Program Files'a kurulsaydı yazma izni olmazdı.
; - Windows "Uygulamalar" listesine HKCU altındaki Uninstall anahtarıyla eklenir.
; - Masaüstü kısayolu isteğe bağlıdır (bileşen sayfasında işaretlenir/kaldırılır).
Unicode true
!define APP_NAME "ConnectSync"
!define APP_EXE  "connect_sync.exe"      ; cargo binary adı
!define APP_URL  "https://github.com/KaanAlper/ConnectSync"
!define UNINSTALL_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}"
!ifndef VERSION
  !define VERSION "0.0.0"
!endif

Name "${APP_NAME}"
OutFile "ConnectSync-Setup-${VERSION}.exe"
InstallDir "$LOCALAPPDATA\Programs\${APP_NAME}"   ; admin gerektirmez
RequestExecutionLevel user
SetCompressor /SOLID lzma

!include "MUI2.nsh"
; Uygulamanın exe/görev çubuğu ikonuyla aynı (build.rs bunu winres ile gömer)
!define MUI_ICON   "..\assets\app_icon.ico"
!define MUI_UNICON "..\assets\app_icon.ico"
!define MUI_BGCOLOR "1E1E2E"
!define MUI_TEXTCOLOR "CDD6F4"
!define MUI_FINISHPAGE_RUN "$INSTDIR\${APP_EXE}"
!define MUI_FINISHPAGE_RUN_TEXT $(FinishPageText)
!define MUI_COMPONENTSPAGE_NODESC

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_COMPONENTS
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

; Bileşen sayfası: zorunlu uygulama + isteğe bağlı masaüstü kısayolu
LangString SecAppName ${LANG_ENGLISH} "ConnectSync (required)"
LangString SecAppName ${LANG_TURKISH} "ConnectSync (gerekli)"
LangString SecAppName ${LANG_GERMAN} "ConnectSync (erforderlich)"
LangString SecAppName ${LANG_SPANISH} "ConnectSync (obligatorio)"
LangString SecAppName ${LANG_FRENCH} "ConnectSync (obligatoire)"
LangString SecAppName ${LANG_SIMPCHINESE} "ConnectSync（必需）"
LangString SecAppName ${LANG_KOREAN} "ConnectSync (필수)"
LangString SecAppName ${LANG_JAPANESE} "ConnectSync（必須）"

LangString SecDesktopName ${LANG_ENGLISH} "Desktop shortcut"
LangString SecDesktopName ${LANG_TURKISH} "Masaüstüne kısayol oluştur"
LangString SecDesktopName ${LANG_GERMAN} "Desktop-Verknüpfung"
LangString SecDesktopName ${LANG_SPANISH} "Acceso directo en el escritorio"
LangString SecDesktopName ${LANG_FRENCH} "Raccourci sur le bureau"
LangString SecDesktopName ${LANG_SIMPCHINESE} "桌面快捷方式"
LangString SecDesktopName ${LANG_KOREAN} "바탕 화면 바로 가기"
LangString SecDesktopName ${LANG_JAPANESE} "デスクトップ ショートカット"

# Dil secici diyalog
Function .onInit
  !insertmacro MUI_LANGDLL_DISPLAY
FunctionEnd


Section "$(SecAppName)" SecApp
  SectionIn RO   ; zorunlu, kaldırılamaz
  SetOutPath "$INSTDIR"
  File "..\target\release\${APP_EXE}"
  WriteUninstaller "$INSTDIR\Uninstall.exe"
  CreateDirectory "$SMPROGRAMS\${APP_NAME}"
  CreateShortcut "$SMPROGRAMS\${APP_NAME}\${APP_NAME}.lnk" "$INSTDIR\${APP_EXE}"

  ; Windows Ayarlar > Uygulamalar (ve Denetim Masası) listesi
  WriteRegStr   HKCU "${UNINSTALL_KEY}" "DisplayName"     "${APP_NAME}"
  WriteRegStr   HKCU "${UNINSTALL_KEY}" "DisplayVersion"  "${VERSION}"
  WriteRegStr   HKCU "${UNINSTALL_KEY}" "DisplayIcon"     "$INSTDIR\${APP_EXE}"
  WriteRegStr   HKCU "${UNINSTALL_KEY}" "Publisher"       "Kaan Alper"
  WriteRegStr   HKCU "${UNINSTALL_KEY}" "URLInfoAbout"    "${APP_URL}"
  WriteRegStr   HKCU "${UNINSTALL_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr   HKCU "${UNINSTALL_KEY}" "UninstallString" "$INSTDIR\Uninstall.exe"
  WriteRegDWORD HKCU "${UNINSTALL_KEY}" "NoModify" 1
  WriteRegDWORD HKCU "${UNINSTALL_KEY}" "NoRepair" 1
SectionEnd

Section "$(SecDesktopName)" SecDesktop
  CreateShortcut "$DESKTOP\${APP_NAME}.lnk" "$INSTDIR\${APP_EXE}"
SectionEnd

Section "Uninstall"
  Delete "$INSTDIR\${APP_EXE}"
  Delete "$INSTDIR\Uninstall.exe"
  ; Uygulama içi güncelleyicinin (src/updater.rs) bıraktığı olası dosyalar: bunlar silinmezse
  ; RMDir klasörü kaldıramaz.
  Delete "$INSTDIR\connect_sync.old"
  Delete "$INSTDIR\.connect_sync.exe.update"
  RMDir "$INSTDIR"
  Delete "$DESKTOP\${APP_NAME}.lnk"
  Delete "$SMPROGRAMS\${APP_NAME}\${APP_NAME}.lnk"
  RMDir "$SMPROGRAMS\${APP_NAME}"
  DeleteRegKey HKCU "${UNINSTALL_KEY}"
SectionEnd
