; Genius Installer Manager — NSIS installer hooks.
;
; Updates run the installer with /UPDATE. In that mode Tauri's template only
; renames an existing "${PRODUCTNAME}.lnk" and never creates one. After the rename
; from "PR Extension Manager" there is no such shortcut, and the old one is removed
; when the app cleans up the legacy install on first start (src/legacy.rs), which
; would leave the app with nothing to launch it from. So make sure the shortcuts
; exist after every install or update.

!macro NSIS_HOOK_POSTINSTALL
  ${IfNot} ${FileExists} "$SMPROGRAMS\${PRODUCTNAME}.lnk"
    CreateShortcut "$SMPROGRAMS\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
  ${EndIf}

  ; Desktop icon only for people who had one on the old app.
  ${If} ${FileExists} "$DESKTOP\PR Extension Manager.lnk"
  ${AndIfNot} ${FileExists} "$DESKTOP\${PRODUCTNAME}.lnk"
    CreateShortcut "$DESKTOP\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
  ${EndIf}
!macroend
