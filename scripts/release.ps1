# Gix Release Build Script
# This script builds a signed release and generates the update manifest

param(
    [string]$KeyPath = "$env:USERPROFILE\.tauri\gix.key",
    [string]$Password
)

$ErrorActionPreference = "Stop"

# Colors for output
function Write-Step { param($msg) Write-Host "`n==> $msg" -ForegroundColor Cyan }
function Write-Success { param($msg) Write-Host "    $msg" -ForegroundColor Green }
function Write-Error { param($msg) Write-Host "    $msg" -ForegroundColor Red }

Write-Host "`n========================================" -ForegroundColor Yellow
Write-Host "       Gix Release Builder" -ForegroundColor Yellow
Write-Host "========================================`n" -ForegroundColor Yellow

# Check if key exists
if (-not (Test-Path $KeyPath)) {
    Write-Error "Signing key not found at: $KeyPath"
    Write-Host "Run: npm run tauri -- signer generate -w $KeyPath"
    exit 1
}

# Get password if not provided
if (-not $Password) {
    $SecurePassword = Read-Host "Enter signing key password" -AsSecureString
    $Password = [Runtime.InteropServices.Marshal]::PtrToStringAuto(
        [Runtime.InteropServices.Marshal]::SecureStringToBSTR($SecurePassword)
    )
}

# Read version from package.json
Write-Step "Reading version..."
$packageJson = Get-Content "package.json" | ConvertFrom-Json
$version = $packageJson.version
Write-Success "Version: $version"

# Set environment variables for signing
Write-Step "Setting up signing..."
$env:TAURI_SIGNING_PRIVATE_KEY = Get-Content $KeyPath -Raw
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = $Password
Write-Success "Signing key loaded"

# Build the app
Write-Step "Building Gix v$version (this may take a few minutes)..."
npm run tauri build

if ($LASTEXITCODE -ne 0) {
    Write-Error "Build failed!"
    exit 1
}
Write-Success "Build complete!"

# Find the built files
$bundlePath = "src-tauri\target\release\bundle\nsis"
$exeName = "Gix_${version}_x64-setup.exe"
$exePath = Join-Path $bundlePath $exeName
$sigPath = "$exePath.sig"

if (-not (Test-Path $exePath)) {
    Write-Error "Installer not found: $exePath"
    exit 1
}

if (-not (Test-Path $sigPath)) {
    Write-Error "Signature not found: $sigPath"
    exit 1
}

Write-Success "Found: $exeName"
Write-Success "Found: $exeName.sig"

# Read the signature
$signature = Get-Content $sigPath -Raw

# Generate latest.json
Write-Step "Generating update manifest..."
$pubDate = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
$repoUrl = "https://github.com/Justar96/gix-portal"

$manifest = @{
    version = $version
    notes = "Update to version $version"
    pub_date = $pubDate
    platforms = @{
        "windows-x86_64" = @{
            signature = $signature.Trim()
            url = "$repoUrl/releases/download/v$version/$exeName"
        }
    }
} | ConvertTo-Json -Depth 4

$manifestPath = Join-Path $bundlePath "latest.json"
$manifest | Out-File -FilePath $manifestPath -Encoding utf8
Write-Success "Created: latest.json"

# Summary
Write-Host "`n========================================" -ForegroundColor Green
Write-Host "       Build Complete!" -ForegroundColor Green
Write-Host "========================================" -ForegroundColor Green
Write-Host "`nFiles ready for GitHub Release:" -ForegroundColor White
Write-Host "  $bundlePath\$exeName" -ForegroundColor Gray
Write-Host "  $bundlePath\latest.json" -ForegroundColor Gray

Write-Host "`nNext steps:" -ForegroundColor Yellow
Write-Host "  1. Go to: $repoUrl/releases/new" -ForegroundColor Gray
Write-Host "  2. Tag: v$version" -ForegroundColor Gray
Write-Host "  3. Upload: $exeName and latest.json" -ForegroundColor Gray
Write-Host "  4. Publish the release!" -ForegroundColor Gray
Write-Host ""

# Open the bundle folder
explorer $bundlePath
