; Instalador Windows do GraphDraw (NSIS 3, licença zlib).
; Linha de comando como graphdraw-cli.exe: no Windows "graphdraw.exe" e
; "GraphDraw.exe" são o mesmo nome.
; Gerado por scripts/package-windows.sh, que passa:
;   /DVERSION=x.y.z  /DSTAGE=<pasta com os arquivos>  /DOUTFILE=<setup.exe>
; Instala só para o usuário (sem administrador) em
; %LOCALAPPDATA%\Programs\GraphDraw.

Unicode true
SetCompressor /SOLID lzma
RequestExecutionLevel user

!include "MUI2.nsh"

!define APP "GraphDraw"
!define EXE "GraphDraw.exe"
!define UNINST_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP}"
!define PROGID "GraphDraw.Document"

Name "${APP} ${VERSION}"
OutFile "${OUTFILE}"
InstallDir "$LOCALAPPDATA\Programs\${APP}"
InstallDirRegKey HKCU "Software\${APP}" "InstallDir"
BrandingText "${APP} ${VERSION}"

!define MUI_ICON "${STAGE}\app.ico"
!define MUI_UNICON "${STAGE}\app.ico"
!define MUI_ABORTWARNING
!define MUI_FINISHPAGE_RUN "$INSTDIR\${EXE}"

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "${STAGE}\LICENSE.txt"
!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "PortugueseBR"
!insertmacro MUI_LANGUAGE "English"

LangString SecApp ${LANG_PORTUGUESEBR} "GraphDraw (obrigatório)"
LangString SecApp ${LANG_ENGLISH} "GraphDraw (required)"
LangString SecDesk ${LANG_PORTUGUESEBR} "Atalho na área de trabalho"
LangString SecDesk ${LANG_ENGLISH} "Desktop shortcut"
LangString SecGdr ${LANG_PORTUGUESEBR} "Abrir arquivos .gdr com o GraphDraw"
LangString SecGdr ${LANG_ENGLISH} "Open .gdr files with GraphDraw"
LangString SecCdr ${LANG_PORTUGUESEBR} "Abrir arquivos .cdr com o GraphDraw"
LangString SecCdr ${LANG_ENGLISH} "Open .cdr files with GraphDraw"
LangString DocName ${LANG_PORTUGUESEBR} "Desenho GraphDraw"
LangString DocName ${LANG_ENGLISH} "GraphDraw drawing"

Section "!$(SecApp)" SEC_APP
  SectionIn RO
  SetOutPath "$INSTDIR"
  File "${STAGE}\${EXE}"
  File "${STAGE}\graphdraw-cli.exe"
  File "${STAGE}\LICENSE.txt"
  File "${STAGE}\THIRD_PARTY.md"
  SetOutPath "$INSTDIR\cdr-bridge"
  File /r "${STAGE}\cdr-bridge\*.*"
  SetOutPath "$INSTDIR"
  WriteUninstaller "$INSTDIR\Desinstalar.exe"
  CreateShortcut "$SMPROGRAMS\${APP}.lnk" "$INSTDIR\${EXE}"
  WriteRegStr HKCU "Software\${APP}" "InstallDir" "$INSTDIR"
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayName" "${APP}"
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${UNINST_KEY}" "Publisher" "Projeto GraphDraw"
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayIcon" "$INSTDIR\${EXE},0"
  WriteRegStr HKCU "${UNINST_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${UNINST_KEY}" "UninstallString" '"$INSTDIR\Desinstalar.exe"'
  WriteRegDWORD HKCU "${UNINST_KEY}" "NoModify" 1
  WriteRegDWORD HKCU "${UNINST_KEY}" "NoRepair" 1
  ; Tipo de documento: ícone do .gdr (índice 1 do executável) e abrir.
  WriteRegStr HKCU "Software\Classes\${PROGID}" "" "$(DocName)"
  WriteRegStr HKCU "Software\Classes\${PROGID}\DefaultIcon" "" "$INSTDIR\${EXE},1"
  WriteRegStr HKCU "Software\Classes\${PROGID}\shell\open\command" "" '"$INSTDIR\${EXE}" "%1"'
  WriteRegStr HKCU "Software\Classes\Applications\${EXE}\shell\open\command" "" '"$INSTDIR\${EXE}" "%1"'
  WriteRegStr HKCU "Software\Classes\Applications\${EXE}\SupportedTypes" ".gdr" ""
  WriteRegStr HKCU "Software\Classes\Applications\${EXE}\SupportedTypes" ".cdr" ""
SectionEnd

Section "$(SecDesk)" SEC_DESK
  CreateShortcut "$DESKTOP\${APP}.lnk" "$INSTDIR\${EXE}"
SectionEnd

Section "$(SecGdr)" SEC_GDR
  WriteRegStr HKCU "Software\Classes\.gdr" "" "${PROGID}"
  WriteRegStr HKCU "Software\Classes\.gdr\OpenWithProgids" "${PROGID}" ""
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'
SectionEnd

; Desmarcado: quem usa outro programa para .cdr não perde a associação.
Section /o "$(SecCdr)" SEC_CDR
  WriteRegStr HKCU "Software\Classes\.cdr\OpenWithProgids" "${PROGID}" ""
  WriteRegStr HKCU "Software\Classes\.cdr" "" "${PROGID}"
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'
SectionEnd

Section "Uninstall"
  Delete "$SMPROGRAMS\${APP}.lnk"
  Delete "$DESKTOP\${APP}.lnk"
  RMDir /r "$INSTDIR\cdr-bridge"
  Delete "$INSTDIR\${EXE}"
  Delete "$INSTDIR\graphdraw-cli.exe"
  Delete "$INSTDIR\LICENSE.txt"
  Delete "$INSTDIR\THIRD_PARTY.md"
  Delete "$INSTDIR\Desinstalar.exe"
  RMDir "$INSTDIR"
  ; Só desfaz a associação se ainda é a nossa.
  ReadRegStr $0 HKCU "Software\Classes\.gdr" ""
  StrCmp $0 "${PROGID}" 0 +2
    DeleteRegKey HKCU "Software\Classes\.gdr"
  ReadRegStr $0 HKCU "Software\Classes\.cdr" ""
  StrCmp $0 "${PROGID}" 0 +2
    DeleteRegValue HKCU "Software\Classes\.cdr" ""
  DeleteRegValue HKCU "Software\Classes\.cdr\OpenWithProgids" "${PROGID}"
  DeleteRegKey HKCU "Software\Classes\${PROGID}"
  DeleteRegKey HKCU "Software\Classes\Applications\${EXE}"
  DeleteRegKey HKCU "${UNINST_KEY}"
  DeleteRegKey HKCU "Software\${APP}"
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'
SectionEnd
