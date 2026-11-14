; Per-user Anchored installer.
; The database and the sealed key live in %APPDATA%\Anchored and are not removed.
Unicode True
ManifestDPIAware True
RequestExecutionLevel user
SetCompressor /SOLID lzma

!include "MUI2.nsh"
!include "FileFunc.nsh"
!include "LogicLib.nsh"

!ifndef VERSION
  !define VERSION "1.1.0"
!endif

Name "Anchored"
OutFile "dist-desktop\Anchored-Setup-${VERSION}.exe"
InstallDir "$LOCALAPPDATA\Programs\Anchored"
InstallDirRegKey HKCU "Software\Anchored" "InstallDir"
ShowInstDetails show
ShowUninstDetails show

!define MUI_ABORTWARNING
!define MUI_WELCOMEPAGE_TITLE "Install Anchored"
!define MUI_WELCOMEPAGE_TEXT "Anchored opens in its own window. Your books stay encrypted in %APPDATA%\Anchored\anchored.db. Windows seals the database key for your account in %APPDATA%\Anchored\db-key.bin. You do not type a key. A copy of the database file by itself cannot be read."
!define MUI_FINISHPAGE_RUN "$INSTDIR\Anchored.exe"
!define MUI_FINISHPAGE_RUN_TEXT "Open Anchored"
!define MUI_FINISHPAGE_TEXT "Anchored is on the Start menu and the desktop. Open the Anchored shortcut. It opens a window on this PC."

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"

VIProductVersion "${VERSION}.0"
VIFileVersion "${VERSION}.0"
VIAddVersionKey "ProductName" "Anchored"
VIAddVersionKey "FileDescription" "Anchored desktop"
VIAddVersionKey "FileVersion" "${VERSION}"
VIAddVersionKey "ProductVersion" "${VERSION}"
VIAddVersionKey "LegalCopyright" "Anchored"

Section "Anchored"
  SetOutPath "$INSTDIR"
  File "dist-desktop\stage\Anchored.exe"
  SetOutPath "$INSTDIR\static"
  File "dist-desktop\stage\static\app.css"
  SetOutPath "$INSTDIR"

  CreateDirectory "$SMPROGRAMS\Anchored"
  CreateShortcut "$SMPROGRAMS\Anchored\Anchored.lnk" "$INSTDIR\Anchored.exe" "" "$INSTDIR\Anchored.exe" 0
  CreateShortcut "$DESKTOP\Anchored.lnk" "$INSTDIR\Anchored.exe" "" "$INSTDIR\Anchored.exe" 0

  WriteUninstaller "$INSTDIR\Uninstall.exe"
  WriteRegStr HKCU "Software\Anchored" "InstallDir" "$INSTDIR"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Anchored" "DisplayName" "Anchored"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Anchored" "UninstallString" "$\"$INSTDIR\Uninstall.exe$\""
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Anchored" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Anchored" "Publisher" "Anchored"
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Anchored" "NoModify" 1
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Anchored" "NoRepair" 1
  ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
  IntFmt $0 "0x%08X" $0
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Anchored" "EstimatedSize" "$0"
SectionEnd

Section "Uninstall"
  Delete "$INSTDIR\Anchored.exe"
  Delete "$INSTDIR\static\app.css"
  Delete "$INSTDIR\Uninstall.exe"
  RMDir "$INSTDIR\static"
  RMDir "$INSTDIR"
  Delete "$DESKTOP\Anchored.lnk"
  Delete "$SMPROGRAMS\Anchored\Anchored.lnk"
  RMDir "$SMPROGRAMS\Anchored"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Anchored"
  DeleteRegKey HKCU "Software\Anchored"
  ; %APPDATA%\Anchored (anchored.db and db-key.bin) stays on this PC.
SectionEnd
