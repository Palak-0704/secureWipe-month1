param(
    [Parameter(Mandatory=$true)] [string]$UsbDeviceId,
    [Parameter(Mandatory=$false)] [string]$PayloadPath,
    [Parameter(Mandatory=$false)] [string]$DefaultImage,
    [switch]$Force
)

function Write-Log {
    param([string]$Message, [string]$Color = "White")
    Write-Host "[$(Get-Date -Format 'HH:mm:ss')] $Message" -ForegroundColor $Color
}

function Get-ProjectRoot {
    $current = $PSScriptRoot
    while ($current -and -not (Test-Path (Join-Path $current "Month1-Submission"))) {
        $current = Split-Path $current -Parent
    }
    return $current
}

# Check for Administrative privileges
$currentPrincipal = New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if (-not $currentPrincipal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    Write-Log "ERROR: This script must be run as Administrator." "Red"
    if ($null -ne $Host.UI -and $null -ne $Host.UI.RawUI) {
        Read-Host "Press Enter to exit"
    }
    exit 1
}

function Get-DiskNumberFromId {
    param([string]$Id)
    if ($Id -match "PhysicalDrive(\d+)") { return [int]$Matches[1] }
    return $null
}

function Get-DriveLetterFromDisk {
    param([int]$DiskNumber)
    $partition = Get-Partition -DiskNumber $DiskNumber -ErrorAction SilentlyContinue | Where-Object { $_.DriveLetter } | Select-Object -First 1
    if ($partition) { return $partition.DriveLetter }
    return $null
}

function Format-UsbForVentoy {
    param([int]$DiskNumber)
    
    Write-Log "Formatting Disk $DiskNumber for Ventoy..." "Cyan"

    # Safety Check: Ensure we aren't formatting the System Drive
    $disk = Get-Disk -Number $DiskNumber
    if ($disk.IsSystem) {
        Write-Log "CRITICAL ERROR: Disk $DiskNumber is the System Drive. Aborting." "Red"
        exit 1
    }
    
    # Create diskpart script for Ventoy formatting
    $diskpartScript = @"
select disk $DiskNumber
clean
convert gpt
create partition primary
select partition 1
format fs=exfat quick label=VENTOY
assign
exit
"@
    
    $diskpartFile = "$env:TEMP\ventoy_format.txt"
    $diskpartScript | Set-Content -Path $diskpartFile -Encoding ASCII
    
    Write-Log "Executing diskpart formatting..." "Yellow"
    & diskpart.exe /s $diskpartFile
    
    Start-Sleep -Seconds 2
    Remove-Item $diskpartFile -Force -ErrorAction SilentlyContinue
    
    Write-Log "Disk formatting complete" "Green"
    return $true
}

function Install-Ventoy {
    param([string]$DriveLetter)

    Write-Log "Installing Ventoy on $DriveLetter..." "Cyan"

    $root = Get-ProjectRoot

    # Validate drive letter before proceeding
    if (-not (Test-Path "${DriveLetter}:\")) {
        Write-Log "Drive $DriveLetter is not accessible" "Red"
        return $false
    }

    # Try to find Ventoy2Disk in project tools or scripts folder
    $ventoyInstallerPath = Join-Path $root "tools\ventoy\Ventoy2Disk.exe"
    if (-not (Test-Path $ventoyInstallerPath)) {
        $ventoyInstallerPath = Join-Path $PSScriptRoot "Ventoy2Disk.exe"
    }

    if (-not (Test-Path $ventoyInstallerPath)) {
        Write-Log "Ventoy2Disk.exe not found. Please place it in 'tools\ventoy' or the 'scripts' folder." "Red"
        return $false
    }

    $ventoyCommand = "`"$ventoyInstallerPath`" -i -g -f $DriveLetter" # -i: install, -g: GPT partition style, -f: force install
    Write-Log "Executing: $ventoyCommand" "Yellow"

    $ventoyResult = Invoke-Expression -Command $ventoyCommand 2>&1
    if ($LASTEXITCODE -ne 0) {
        Write-Log "Ventoy installation failed: $ventoyResult" "Red"
        return $false
    }

    Write-Log "Ventoy installed successfully on $DriveLetter" "Green"
    return $true
}

function Copy-SecureWipeISO {
    param([string]$DriveLetter)

    Write-Log "Copying SecureWipe ISO to $DriveLetter..." "Cyan"

    $root = Get-ProjectRoot
    $isoPath = Join-Path $root "Month1-Submission\dist\securewipe.iso"

    if (-not (Test-Path $isoPath)) {
        Write-Log "SecureWipe ISO not found at $isoPath. Please ensure the file exists." "Red"
        return $false
    }

    Copy-Item -Path $isoPath -Destination (Join-Path "${DriveLetter}:\" (Split-Path $isoPath -Leaf)) -Force
    Write-Log "SecureWipe ISO copied successfully" "Green"
    return $true
}

function Configure-VentoyAutoBoot {
    param([string]$DriveLetter, [string]$IsoPath)
    $isoName = Split-Path $IsoPath -Leaf
    $ventoyDir = "${DriveLetter}:\ventoy"
    if (-not (Test-Path $ventoyDir)) { New-Item -Path $ventoyDir -ItemType Directory | Out-Null }

    $json = "{`n  `"control`": [`n    { `"VTOY_DEFAULT_IMAGE`": `"/$isoName`" },`n    { `"VTOY_MENU_TIMEOUT`": `"2`" }`n  ]`n}"
    $json | Set-Content -Path "${ventoyDir}\ventoy.json" -Encoding UTF8NoBOM
    Write-Log "Ventoy auto-boot configured for $isoName" "Green"
}

function Copy-ProjectPayload {
    param([string]$SourcePath, [string]$DriveLetter)
    if (-not $SourcePath -or -not (Test-Path $SourcePath)) { return $true }
    Write-Log "Loading project files into USB..." "Cyan"
    Copy-Item -Path "${SourcePath}\*" -Destination "${DriveLetter}:\" -Recurse -Force
    return $true
}

# Main execution
$DiskNumber = Get-DiskNumberFromId -Id $UsbDeviceId
if ($null -eq $DiskNumber) {
    Write-Log "Invalid Device ID: $UsbDeviceId" "Red"
    exit 1
}

$InitialLetter = Get-DriveLetterFromDisk -DiskNumber $DiskNumber
$DriveLetter = if ($InitialLetter) { $InitialLetter } else { "F" } # Default fallback

if (-not $Force) {
    Write-Log "WARNING: This will format Disk $DiskNumber" "Yellow"
    $confirm = Read-Host "Proceed? (yes/no)"
    if ($confirm -ne "yes") {
        Write-Log "Cancelled" "Yellow"
        exit 1
    }
}

Write-Log "=== VENTOY USB PROVISIONING ===" "Green"
Write-Log "Target Disk: $DiskNumber" "Cyan"

# Step 1: Format Disk
if (-not (Format-UsbForVentoy -DiskNumber $DiskNumber)) {
    Write-Log "Formatting failed" "Red"
    exit 1
}

# Step 2: Re-verify letter after format (Windows might reassign it)
Start-Sleep -Seconds 2
$NewLetter = Get-DriveLetterFromDisk -DiskNumber $DiskNumber
if ($NewLetter) { $DriveLetter = $NewLetter }
Write-Log "Using Drive Letter: $DriveLetter" "Cyan"

# Step 3: Install Ventoy
Install-Ventoy -DriveLetter $DriveLetter

# Step 4: Copy SecureWipe ISO
Copy-SecureWipeISO -DriveLetter $DriveLetter

# Step 5: Load Project Payload (Manifests, Runtime, Runners)
Copy-ProjectPayload -SourcePath $PayloadPath -DriveLetter $DriveLetter

# Step 6: Configure Auto-Start
$isoToBoot = if ($DefaultImage) { $DefaultImage } else { "securewipe.iso" }
Configure-VentoyAutoBoot -DriveLetter $DriveLetter -IsoPath $isoToBoot

Write-Log "=== PROVISIONING COMPLETE ===" "Green"
Write-Log "Disk $DiskNumber is now a Ventoy bootable drive with SecureWipe ISO" "Green"