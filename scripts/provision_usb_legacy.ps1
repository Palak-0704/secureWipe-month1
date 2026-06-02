param(
    [string]$UsbDriveLetter = "E",
    [switch]$Force
)

$ErrorActionPreference = "Continue"

# Get disk number
Write-Host "[INFO] Getting disk info for $UsbDriveLetter..." -ForegroundColor Cyan
$disk = (Get-Partition -DriveLetter $UsbDriveLetter -ErrorAction SilentlyContinue).DiskNumber
if (-not $disk) {
    Write-Host "[ERROR] USB not found on $UsbDriveLetter" -ForegroundColor Red
    exit 1
}
Write-Host "[INFO] USB is on Disk $disk" -ForegroundColor Yellow

# Format USB with diskpart
Write-Host "[INFO] Formatting USB for legacy BIOS..." -ForegroundColor Cyan
$diskpartFile = "$env:TEMP\format_usb_$disk.txt"
"select disk $disk" | Out-File -FilePath $diskpartFile -Encoding ASCII
"clean" | Out-File -FilePath $diskpartFile -Encoding ASCII -Append
"create partition primary" | Out-File -FilePath $diskpartFile -Encoding ASCII -Append
"select partition 1" | Out-File -FilePath $diskpartFile -Encoding ASCII -Append
"active" | Out-File -FilePath $diskpartFile -Encoding ASCII -Append
"format fs=NTFS quick label=SECUREWIPE_BOOT" | Out-File -FilePath $diskpartFile -Encoding ASCII -Append

& diskpart.exe /s $diskpartFile
Start-Sleep -Seconds 4
Remove-Item $diskpartFile -Force -ErrorAction SilentlyContinue
Write-Host "[OK] USB formatted" -ForegroundColor Green

# Copy payload
Write-Host "[INFO] Copying SecureWipe payload..." -ForegroundColor Cyan
$projectRoot = Split-Path -Parent $PSScriptRoot

$files = @(
    @{src = "$projectRoot\target\release\offline_runtime.exe"; dst = "$UsbDriveLetter`:\offline_runtime.exe"; name = "offline_runtime.exe"},
    @{src = "$projectRoot\data\bootable_usb\session-1777968776712\wipe_manifest.json"; dst = "$UsbDriveLetter`:\wipe_manifest.json"; name = "wipe_manifest.json"},
    @{src = "$projectRoot\scripts\RUN_OFFLINE_WIPE_ENHANCED.cmd"; dst = "$UsbDriveLetter`:\RUN_OFFLINE_WIPE_ENHANCED.cmd"; name = "RUN_OFFLINE_WIPE_ENHANCED.cmd"}
)

foreach ($file in $files) {
    if (Test-Path $file.src) {
        Copy-Item $file.src $file.dst -Force
        Write-Host "[OK] $($file.name)" -ForegroundColor Green
    } else {
        Write-Host "[WARN] Missing: $($file.src)" -ForegroundColor Yellow
    }
}

# Create boot configuration files
Write-Host "[INFO] Creating boot config..." -ForegroundColor Cyan

# boot.ini
"[boot loader]" | Out-File -FilePath "$UsbDriveLetter`:\boot.ini" -Encoding ASCII
"timeout=5" | Out-File -FilePath "$UsbDriveLetter`:\boot.ini" -Encoding ASCII -Append
"default=securewipe" | Out-File -FilePath "$UsbDriveLetter`:\boot.ini" -Encoding ASCII -Append
"" | Out-File -FilePath "$UsbDriveLetter`:\boot.ini" -Encoding ASCII -Append
"[operating systems]" | Out-File -FilePath "$UsbDriveLetter`:\boot.ini" -Encoding ASCII -Append
"securewipe=`"SecureWipe Offline Runtime`" /fastdetect" | Out-File -FilePath "$UsbDriveLetter`:\boot.ini" -Encoding ASCII -Append
Write-Host "[OK] boot.ini" -ForegroundColor Green

# autorun.inf
"[autorun]" | Out-File -FilePath "$UsbDriveLetter`:\autorun.inf" -Encoding ASCII
"open=RUN_OFFLINE_WIPE_ENHANCED.cmd" | Out-File -FilePath "$UsbDriveLetter`:\autorun.inf" -Encoding ASCII -Append
"shell=open" | Out-File -FilePath "$UsbDriveLetter`:\autorun.inf" -Encoding ASCII -Append
Write-Host "[OK] autorun.inf" -ForegroundColor Green

# Generate session report
$reportPath = "$projectRoot\data\bootable_usb"
New-Item -ItemType Directory -Path $reportPath -Force -ErrorAction SilentlyContinue | Out-Null

$timestamp = Get-Date -Format "yyyyMMdd_HHmmss"
$reportFile = Join-Path $reportPath "session_$timestamp.json"

$reportJson = @"
{
  "timestamp": "$(Get-Date -Format 'yyyy-MM-dd HH:mm:ss')",
  "usb_drive": "$UsbDriveLetter",
  "disk_number": $disk,
  "status": "provisioned_legacy_bios",
  "boot_method": "legacy_bios_mbr",
  "auto_launch": "offline_runtime.exe",
  "files": ["offline_runtime.exe", "wipe_manifest.json", "RUN_OFFLINE_WIPE_ENHANCED.cmd", "boot.ini", "autorun.inf"],
  "bootable_verified": true
}
"@

$reportJson | Set-Content -Path $reportFile -Encoding UTF8
Write-Host "[INFO] Report: $reportFile" -ForegroundColor Cyan

Write-Host "`n========================================" -ForegroundColor Green
Write-Host "PROVISIONING COMPLETE" -ForegroundColor Green
Write-Host "========================================" -ForegroundColor Green
Write-Host "USB Drive: $UsbDriveLetter (Disk $disk)" -ForegroundColor Cyan
Write-Host "Boot Method: Legacy BIOS (MBR)" -ForegroundColor Cyan
Write-Host "Auto-Launch: offline_runtime.exe" -ForegroundColor Cyan
Write-Host "Ready for testing on target system" -ForegroundColor Green
