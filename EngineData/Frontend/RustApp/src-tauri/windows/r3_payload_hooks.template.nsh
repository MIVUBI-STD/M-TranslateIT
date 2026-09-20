; TranslateIT R3 external offline payload lifecycle.
!include "FileFunc.nsh"
; build_r3_external_payload.py renders trusted identity/hash values into an ignored
; generated hook used only by the current Tauri/NSIS build.

!define TRANSLATEIT_R3_PAYLOAD_SCHEMA "@@PAYLOAD_SCHEMA@@"
!define TRANSLATEIT_R3_INSTALLED_RUNTIME_SCHEMA "@@INSTALLED_RUNTIME_SCHEMA@@"
!define TRANSLATEIT_R3_APP_VERSION "@@APP_VERSION@@"
!define TRANSLATEIT_R3_PAYLOAD_FILENAME "@@PAYLOAD_FILENAME@@"
!define TRANSLATEIT_R3_PAYLOAD_SHA256 "@@PAYLOAD_SHA256@@"
!define TRANSLATEIT_R3_PAYLOAD_EXPANDED_BYTES "@@PAYLOAD_EXPANDED_BYTES@@"

!macro NSIS_HOOK_PREINSTALL
  InitPluginsDir
  File /oname=$PLUGINSDIR\translateit-r3-payload-installer.ps1 "@@INSTALLER_HELPER_SOURCE@@"

  ${GetParameters} $R8
  ClearErrors
  ${GetOptions} $R8 "/UPDATE" $R9
  ${IfNot} ${Errors}
    DetailPrint "TranslateIT: app-update mode; validating installed runtime without replacing the external payload..."
    ExecWait '"$SYSDIR\WindowsPowerShell\v1.0\powershell.exe" -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$PLUGINSDIR\translateit-r3-payload-installer.ps1" -Mode VerifyInstalled -ExpectedPayloadSchema "${TRANSLATEIT_R3_PAYLOAD_SCHEMA}" -ExpectedInstalledRuntimeSchema "${TRANSLATEIT_R3_INSTALLED_RUNTIME_SCHEMA}" -ExpectedAppVersion "${TRANSLATEIT_R3_APP_VERSION}" -InstallRoot "$INSTDIR"' $0
    ${If} $0 != 0
      MessageBox MB_ICONSTOP|MB_OK "This TranslateIT update needs a newer full runtime payload. Install the matching TranslateIT-Setup.exe + TranslateIT-Payload.7z release instead."
      Abort
    ${EndIf}
    Goto translateit_r3_preinstall_done
  ${EndIf}

  DetailPrint "TranslateIT: validating colocated offline payload..."
  IfFileExists "$EXEDIR\${TRANSLATEIT_R3_PAYLOAD_FILENAME}" translateit_r3_payload_present translateit_r3_payload_missing

  translateit_r3_payload_missing:
    MessageBox MB_ICONSTOP|MB_OK "TranslateIT-Payload.7z is missing. Keep TranslateIT-Setup.exe and TranslateIT-Payload.7z in the same folder, then run Setup again."
    Abort

  translateit_r3_payload_present:
    ExecWait '"$SYSDIR\WindowsPowerShell\v1.0\powershell.exe" -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$PLUGINSDIR\translateit-r3-payload-installer.ps1" -Mode Verify -PayloadPath "$EXEDIR\${TRANSLATEIT_R3_PAYLOAD_FILENAME}" -ExpectedSha256 "${TRANSLATEIT_R3_PAYLOAD_SHA256}" -ExpectedPayloadSchema "${TRANSLATEIT_R3_PAYLOAD_SCHEMA}" -ExpectedInstalledRuntimeSchema "${TRANSLATEIT_R3_INSTALLED_RUNTIME_SCHEMA}" -ExpectedAppVersion "${TRANSLATEIT_R3_APP_VERSION}" -ExpectedExpandedBytes "${TRANSLATEIT_R3_PAYLOAD_EXPANDED_BYTES}"' $0
    ${If} $0 != 0
      MessageBox MB_ICONSTOP|MB_OK "TranslateIT offline payload validation failed (code $0). Setup will stop instead of installing incomplete, modified, or mismatched runtime assets."
      Abort
    ${EndIf}

  translateit_r3_preinstall_done:
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ${GetParameters} $R8
  ClearErrors
  ${GetOptions} $R8 "/UPDATE" $R9
  ${IfNot} ${Errors}
    DetailPrint "TranslateIT: app-update mode; preserving the verified installed runtime payload."
    Goto translateit_r3_postinstall_done
  ${EndIf}

  DetailPrint "TranslateIT: installing verified offline runtime payload..."
  ExecWait '"$SYSDIR\WindowsPowerShell\v1.0\powershell.exe" -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$PLUGINSDIR\translateit-r3-payload-installer.ps1" -Mode Install -PayloadPath "$EXEDIR\${TRANSLATEIT_R3_PAYLOAD_FILENAME}" -ExpectedSha256 "${TRANSLATEIT_R3_PAYLOAD_SHA256}" -ExpectedPayloadSchema "${TRANSLATEIT_R3_PAYLOAD_SCHEMA}" -ExpectedInstalledRuntimeSchema "${TRANSLATEIT_R3_INSTALLED_RUNTIME_SCHEMA}" -ExpectedAppVersion "${TRANSLATEIT_R3_APP_VERSION}" -ExpectedExpandedBytes "${TRANSLATEIT_R3_PAYLOAD_EXPANDED_BYTES}" -InstallRoot "$INSTDIR"' $0
  ${If} $0 == 3010
    DetailPrint "TranslateIT: VB-CABLE was newly installed; Windows restart is required."
    SetRebootFlag true
    MessageBox MB_ICONINFORMATION|MB_OK "TranslateIT installed the required VB-CABLE driver. Restart Windows before using Meeting Microphone."
  ${ElseIf} $0 != 0
    MessageBox MB_ICONSTOP|MB_OK "TranslateIT offline runtime installation failed (code $0). Re-run Setup with the original colocated payload; do not install Python, models, or another TranslateIT component manually."
    Abort
  ${EndIf}

  translateit_r3_postinstall_done:
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ; Only TranslateIT-owned external runtime bytes are removed here. The VB-CABLE
  ; system driver may be shared with other software and UserData lives outside
  ; $INSTDIR, so both are intentionally preserved.
  DetailPrint "TranslateIT: removing external application runtime payload..."
  RMDir /r "$INSTDIR\EngineData\Backend\LocalWorker\PythonRuntime"
  RMDir /r "$INSTDIR\EngineData\Backend\RuntimeAssets\ASR\ModelData"
  RMDir /r "$INSTDIR\EngineData\Backend\RuntimeAssets\Translation\ModelData"
  RMDir /r "$INSTDIR\EngineData\Backend\RuntimeAssets\Voice\BuiltInVoices"
  RMDir /r "$INSTDIR\EngineData\Backend\RuntimeAssets\Voice\GPTSoVITS"
  RMDir /r "$INSTDIR\EngineData\Backend\RuntimeAssets\AudioProvider\VBCABLE\Package"
  Delete "$INSTDIR\EngineData\Backend\TRANSLATEIT_INSTALLED_RUNTIME.json"
  RMDir /r "$INSTDIR\.translateit-r3-stage"
  RMDir /r "$INSTDIR\.translateit-r3-backup"
!macroend
