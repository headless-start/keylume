; Keylume's look for its Windows installer. Tauri's NSIS template includes this file
; before it builds the pages, so these settings theme the whole wizard:
; the app's dark header, welcome and finish pages (art from tools/make_installer_art.py),
; a read-me page, and a smooth progress bar in the app's blue.

!define MUI_BGCOLOR "0D0F14"
!define MUI_TEXTCOLOR "EEF1F7"
!define MUI_INSTFILESPAGE_COLORS "3D6BFF 0D0F14"
!define MUI_INSTFILESPAGE_PROGRESSBAR "smooth colored"

!define MUI_WELCOMEPAGE_TITLE "Welcome to Keylume"
!define MUI_WELCOMEPAGE_TEXT "Lighting and keys for your keyboard, with tens of thousands of designs to choose from.$\r$\n$\r$\nKeylume works entirely on this PC: no account, no cloud, no telemetry.$\r$\n$\r$\nClick Next to continue."

!define MUI_LICENSEPAGE_TEXT_TOP "A few things to know before you install."
!define MUI_LICENSEPAGE_BUTTON "&Next >"
!define MUI_LICENSEPAGE_TEXT_BOTTOM "Click Next to choose where to install Keylume."

; The licences (Keylume's own and third-party ones), beside the app (tools/third_party_notices.mjs writes the file before
; each build). The path is taken here, where this file is, not where the macro is used.
!define KEYLUME_NOTICES "${__FILEDIR__}\..\THIRD-PARTY-NOTICES.txt"

!macro NSIS_HOOK_POSTINSTALL
  SetOutPath "$INSTDIR"
  File "/oname=THIRD-PARTY-NOTICES.txt" "${KEYLUME_NOTICES}"
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  Delete "$INSTDIR\THIRD-PARTY-NOTICES.txt"
!macroend

!define MUI_FINISHPAGE_TITLE "Keylume is ready"
!define MUI_FINISHPAGE_TEXT "Plug in your keyboard and pick a look from the library.$\r$\n$\r$\nClick Finish to close this window."
