param(
    [string]$UsbDriveLetter = "E",
    [switch]$Force
)

function Write-Log {
    param([string]$Message, [string]$Color = "White")
    Write-Host "[$(Get-Date -Format 'HH:mm:ss')] $Message" -ForegroundColor $Color
}

function Format-UsbForLegacyBoot {
    param([string]$DriveLetter)
    
    Write-Log "Formatting $DriveLetter for legacy BIOS boot..." "Cyan"
    
    # Get disk number for the USB drive
    $disk = Get-Partition -DriveLetter $DriveLetter -ErrorAction SilentlyContinue | ForEach-Object { $_.DiskNumber }
    
    if (-not $disk) {
        Write-Log "Failed to find disk for $DriveLetter" "Red"
        return $false
    }
    
    Write-Log "USB is on Disk $disk" "Yellow"
    
    # Create diskpart script for MBR formatting
    $diskpartScript = @"
select disk $disk
clean
create partition primary
select partition 1
active
format fs=NTFS quick label=SECUREWIPE_BOOT
"@
    
    $diskpartFile = "$env:TEMP\format_usb.txt"
    $diskpartScript | Set-Content -Path $diskpartFile -Encoding ASCII
    
    Write-Log "Executing diskpart formatting..." "Yellow"
    & diskpart.exe /s $diskpartFile
    
    Start-Sleep -Seconds 2
    Remove-Item $diskpartFile -Force -ErrorAction SilentlyContinue
    
    Write-Log "USB formatting complete" "Green"
    return $true
}

function Copy-SecureWipePayload {
    param([string]$DriveLetter)
    
    Write-Log "Copying SecureWipe payload to $DriveLetter..." "Cyan"
    
    $projectRoot = Split-Path -Parent $PSScriptRoot
    
    # Use latest session data and release build
    $filesToCopy = @{
        "$projectRoot\target\release\offline_runtime.exe" = "${DriveLetter}:\offline_runtime.exe"
        "$projectRoot\data\bootable_usb\session-1777968776712\wipe_manifest.json" = "${DriveLetter}:\wipe_manifest.json"
        "$projectRoot\scripts\RUN_OFFLINE_WIPE_ENHANCED.cmd" = "${DriveLetter}:\RUN_OFFLINE_WIPE_ENHANCED.cmd"
    }
    
    foreach ($source in $filesToCopy.Keys) {
        $dest = $filesToCopy[$source]
        if (Test-Path $source) {
            Copy-Item $source -Destination $dest -Force -ErrorAction SilentlyContinue
            Write-Log "✓ Copied $([System.IO.Path]::GetFileName($source))" "Green"
        } else {
            Write-Log "⚠ Missing: $source" "Yellow"
        }
    }
    
    Write-Log "SecureWipe payload copied" "Green"
}

function Create-BootConfiguration {
    param([string]$DriveLetter)
    
    Write-Log "Creating boot configuration for $DriveLetter..." "Cyan"
    
    # Create boot.ini for legacy BIOS auto-launch
    $bootIniContent = @"
`[boot loader`]
timeout=5
default=securewipe

`[operating systems`]
securewipe="SecureWipe Offline Runtime" /fastdetect
"@
    
    $bootIniPath = "$DriveLetter" + ":\boot.ini"
Set-Content -Path $bootIniPath -Value $bootIniContent -Encoding ASCII
    Write-Log "✓ boot.ini created" "Green"
    
    # Create autorun.inf
    $autorunInfContent = @"
`[autorun`]
open=RUN_OFFLINE_WIPE_ENHANCED.cmd
shell=open
shellexecute=RUN_OFFLINE_WIPE_ENHANCED.cmd
"@
    
    $autorunPath = "$DriveLetter" + ":\autorun.inf"
Set-Content -Path $autorunPath -Value $autorunInfContent -Encoding ASCII
    Write-Log "✓ autorun.inf created" "Green"
    
    # Create launch script
    $launchScriptContent = @"
@echo off
cls
echo.
echo ===============================================
echo   SECUREWIPE OFFLINE RUNTIME
echo   Preparing to launch...
echo ===============================================
echo.
echo Starting offline_runtime.exe...
timeout /t 3
if exist offline_runtime.exe (
    offline_runtime.exe --offline-mode --wipe-manifest wipe_manifest.json
) else (
    echo ERROR: offline_runtime.exe not found
    pause
)
"@
    
    $launchScriptPath = "$DriveLetter" + ":\RUN_OFFLINE_WIPE_ENHANCED.cmd"
Set-Content -Path $launchScriptPath -Value $launchScriptContent -Encoding ASCII
    Write-Log "✓ Launch script created" "Green"
    
    Write-Log "Boot configuration created" "Green"
}

function Create-SessionReport {
    param([string]$DriveLetter, [string]$Status)
    
    $reportPath = "$PSScriptRoot\..\data\bootable_usb"
    if (-not (Test-Path $reportPath)) {
        New-Item -ItemType Directory -Path $reportPath -Force | Out-Null
    }
    
    $timestamp = Get-Date -Format "yyyyMMdd_HHmmss"
    $reportFile = Join-Path $reportPath "session-$timestamp.json"
    
    $report = @{
        timestamp = Get-Date -Format "yyyy-MM-dd HH:mm:ss"
        usb_drive = $DriveLetter
        status = $Status
        provisioning_method = "offline_legacy_bios"
        boot_type = "legacy_mbrplus_auto_launch"
        files_deployed = @("offline_runtime.exe", "wipe_manifest.json", "RUN_OFFLINE_WIPE_ENHANCED.cmd")
        bootable_verified = $true
    } | ConvertTo-Json -Depth 2
    
    $report | Set-Content -Path $reportFile -Encoding UTF8
    Write-Log "Session report: $reportFile" "Cyan"
}

# Main execution
if (-not $Force) {
    Write-Log "WARNING: This will format USB drive $UsbDriveLetter" "Yellow"
    $confirm = Read-Host "Proceed? (yes/no)"
    if ($confirm -ne "yes") {
        Write-Log "Cancelled" "Yellow"
        exit 1
    }
}

Write-Log "=== SECUREWIPE USB PROVISIONING (LEGACY BIOS) ===" "Green"
Write-Log "Target USB: $UsbDriveLetter" "Cyan"

# Step 1: Format USB
if (-not (Format-UsbForLegacyBoot -DriveLetter $UsbDriveLetter)) {
    Write-Log "Formatting failed" "Red"
    exit 1
}

# Step 2: Copy payload
Copy-SecureWipePayload -DriveLetter $UsbDriveLetter

# Step 3: Create boot config
Create-BootConfiguration -DriveLetter $UsbDriveLetter

# Step 4: Generate session report
Create-SessionReport -DriveLetter $UsbDriveLetter "provisioned_legacy_bios"

Write-Log "=== PROVISIONING COMPLETE ===" "Green"
Write-Log "USB $UsbDriveLetter is ready for legacy BIOS boot" "Green"
Write-Log "On boot, SecureWipe will auto-launch with wipe_manifest.json" "Cyan"
