# Install the todo-shim binary to a stable location in the user's PATH (Windows)
# Run: iwr -useb <url> | iex
#
# The shim will automatically find the Right Now app and forward commands
# to the real todo binary, surviving app reinstalls.

$ErrorActionPreference = "Stop"

$InstallDir = "$env:USERPROFILE\bin"
$BinaryName = "todo.exe"

function Detect-Platform {
    $arch = if ([Environment]::Is64BitOperatingSystem) {
        if ([System.Runtime.InteropServices.RuntimeInformation]::ProcessArchitecture -eq "Arm64") {
            "aarch64"
        } else {
            "x86_64"
        }
    } else {
        Write-Error "32-bit systems are not supported"
        exit 1
    }

    return "windows-$arch"
}

function Build-Local {
    $srcDir = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
    $cargoDir = Join-Path $srcDir "src-tauri"

    Write-Host "Building todo-shim locally..."
    Push-Location $cargoDir
    try {
        cargo build --release --bin todo-shim
    } finally {
        Pop-Location
    }

    $binaryPath = Join-Path $srcDir "target\release\todo-shim.exe"
    if (-not (Test-Path $binaryPath)) {
        Write-Error "Build failed - binary not found at $binaryPath"
        exit 1
    }

    return $binaryPath
}

function Install-Shim {
    param([string]$SourceBinary)

    # Create install directory if it doesn't exist
    if (-not (Test-Path $InstallDir)) {
        New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    }

    # Copy the binary
    $destPath = Join-Path $InstallDir $BinaryName
    Copy-Item $SourceBinary $destPath -Force

    Write-Host "Installed $BinaryName to $destPath"
}

function Check-Path {
    $userPath = [Environment]::GetEnvironmentVariable("PATH", "User")

    if ($userPath -notlike "*$InstallDir*") {
        Write-Host ""
        Write-Host "Warning: $InstallDir is not in your PATH" -ForegroundColor Yellow
        Write-Host ""
        Write-Host "To add it permanently, run:"
        Write-Host ""
        Write-Host "  `$currentPath = [Environment]::GetEnvironmentVariable('PATH', 'User')"
        Write-Host "  [Environment]::SetEnvironmentVariable('PATH', `"`$env:USERPROFILE\bin;`$currentPath`", 'User')"
        Write-Host ""
        Write-Host "Then restart your terminal."
        Write-Host ""
    }
}

function Main {
    Write-Host "Installing Right Now todo CLI shim..."
    Write-Host ""

    $platform = Detect-Platform
    Write-Host "Detected platform: $platform"

    # Check if we're running from the repo (development mode)
    $scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
    $cargoToml = Join-Path (Split-Path -Parent $scriptDir) "src-tauri\Cargo.toml"

    if (Test-Path $cargoToml) {
        Write-Host "Running from source repository - building locally"
        $binaryPath = Build-Local
    } else {
        # TODO: Download from releases when available
        Write-Error "Pre-built binaries not yet available. Please run this script from the right-now repository."
        exit 1
    }

    Install-Shim -SourceBinary $binaryPath
    Check-Path

    Write-Host ""
    Write-Host "Installation complete!" -ForegroundColor Green
    Write-Host ""
    Write-Host "Usage:"
    Write-Host "  todo list          # List all sessions"
    Write-Host "  todo start <task>  # Start a new session"
    Write-Host "  todo --help        # Show all commands"
}

Main
