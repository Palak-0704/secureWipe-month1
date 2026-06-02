# Ventoy USB + SecureWipe ISO Runbook (Current)

## Goal
Create a Ventoy bootable USB (Legacy BIOS + UEFI) that boots a SecureWipe ISO and contains the SecureWipe offline payload overlay.

## Ground Rules
- Ventoy is responsible for USB bootability.
- SecureWipe scripts only verify Ventoy + copy artifacts.
- Anything that formats/partitions a USB is destructive; do it intentionally (Ventoy2Disk).

## 1) Install Ventoy (Destructive)
1. Download Ventoy2Disk from https://www.ventoy.net
2. Run `Ventoy2Disk.exe` as Administrator
3. Select the correct USB disk and click Install

## 2) Verify Ventoy From This Repo
From `Month1-Submission/`:
- `./scripts/verify_usb_ventoy.ps1 -DiskNumber <N>`

## 3) Build the SecureWipe ISO
From `Month1-Submission/`:
- `./scripts/create_securewipe_iso.ps1 -SourceDir ./payload -OutputIso ./dist/securewipe.iso -Force`

Notes:
- The script tries `oscdimg` first (Windows ADK), then WSL `genisoimage`.
- If neither is available it stages an ISO build folder under `%TEMP%` and exits non-zero.

## 4) Copy ISO to USB
- `Copy-Item ./dist/securewipe.iso -Destination <USB_DRIVE_LETTER>:\ -Force`

## 5) Optional: Auto-boot the ISO (Ventoy)
Create `<USB_DRIVE_LETTER>:\ventoy\ventoy.json` with:
- `VTOY_DEFAULT_IMAGE = "/securewipe.iso"`
- `VTOY_MENU_TIMEOUT = "2"`

## 6) Overlay SecureWipe Payload to the USB
From `Month1-Submission/scripts/` (or use full path):
- `./usb_provision_enhanced.ps1 -UsbDeviceId "disk<N>" -OutputPath "<payload_dir>"`

What it does (current behavior):
- Confirms the USB is Ventoy-prepared
- Copies SecureWipe handoff artifacts and runners
- Avoids/removes legacy extracted-boot artifacts (the project is Ventoy-first)

## 7) Expected USB Contents (Minimum)
- `securewipe.iso` at USB root
- `ventoy/ventoy.json` (optional)
- Offline handoff artifacts such as `wipe_manifest.json`, runner scripts, and (when available) `offline_runtime.exe`

## Troubleshooting (Most Common)
- Ventoy not detected: reinstall Ventoy2Disk; verify you selected correct disk number.
- No drive letter resolved: reinsert USB; confirm it mounts in Windows.
- `robocopy` exit code >= 8: close apps locking the USB; rerun elevated.
- BIOS can’t see USB: try USB 2.0 port; enable legacy/CSM if required on older machines.

## Important Note About Old Docs
Any older documentation that references WinPE, `bootsect`, or manual boot-file stitching is obsolete for the current Ventoy-first workflow.
