# Varg Installer for Windows
# Usage: iex (irm https://raw.githubusercontent.com/LupusMalusDeviant/VARG/main/install.ps1)

Write-Host ""
Write-Host "============================================" -ForegroundColor Cyan
Write-Host "          Varg Installer for Windows        " -ForegroundColor Cyan
Write-Host "============================================" -ForegroundColor Cyan
Write-Host ""

# ── Step 1: Check for Rust / cargo ───────────────────────────────────────────

$cargo = Get-Command cargo -ErrorAction SilentlyContinue

if (-not $cargo) {
    Write-Host "Rust not found. Installing via rustup..." -ForegroundColor Yellow
    $rustupExe = [System.IO.Path]::GetTempFileName() + ".exe"
    Write-Host "Downloading rustup-init.exe..."
    Invoke-WebRequest -Uri "https://win.rustup.rs/x86_64" -OutFile $rustupExe -UseBasicParsing
    Write-Host "Running rustup-init.exe (this may take a few minutes)..."
    Start-Process -FilePath $rustupExe -ArgumentList "-y", "--no-modify-path" -Wait -NoNewWindow
    Remove-Item $rustupExe -ErrorAction SilentlyContinue
    Write-Host ""
    Write-Host "Rust installed successfully." -ForegroundColor Green
    Write-Host "IMPORTANT: Please restart your terminal, then re-run this installer so" -ForegroundColor Yellow
    Write-Host "           'cargo' is available on your PATH." -ForegroundColor Yellow
    exit 0
} else {
    $rustcVersion = & rustc --version 2>&1
    Write-Host "Rust found: $rustcVersion" -ForegroundColor Green
}

# ── Step 2: Create install directory ─────────────────────────────────────────

$installDir = Join-Path $HOME ".varg\bin"
New-Item -ItemType Directory -Path $installDir -Force | Out-Null
Write-Host "Install directory: $installDir"

# ── Step 3: Fetch latest GitHub release ──────────────────────────────────────

Write-Host ""
Write-Host "Fetching latest Varg release from GitHub..."
try {
    $release = Invoke-RestMethod -Uri "https://api.github.com/repos/LupusMalusDeviant/VARG/releases/latest" -UseBasicParsing
} catch {
    Write-Host "Error: Could not fetch release info from GitHub." -ForegroundColor Red
    Write-Host "Check your internet connection and try again."
    exit 1
}

$tagName = $release.tag_name
Write-Host "Latest release: $tagName"

# ── Step 4: Find the Windows asset ───────────────────────────────────────────

$asset = $release.assets | Where-Object { $_.name -like "*windows*" } | Select-Object -First 1
if (-not $asset) {
    $asset = $release.assets | Where-Object { $_.name -like "varg-v*-windows-x64.zip" } | Select-Object -First 1
}
if (-not $asset) {
    Write-Host "Error: No Windows asset found in release $tagName." -ForegroundColor Red
    Write-Host "Available assets:"
    $release.assets | ForEach-Object { Write-Host "  $($_.name)" }
    exit 1
}

$downloadUrl = $asset.browser_download_url
Write-Host "Downloading: $($asset.name)"
Write-Host "  from: $downloadUrl"

# ── Step 5: Download and extract ─────────────────────────────────────────────

$tempZip   = [System.IO.Path]::GetTempFileName() + ".zip"
$tempDir   = [System.IO.Path]::Combine([System.IO.Path]::GetTempPath(), "varg_install_$([System.Guid]::NewGuid().ToString('N'))")

try {
    Invoke-WebRequest -Uri $downloadUrl -OutFile $tempZip -UseBasicParsing
} catch {
    Write-Host "Error: Download failed. $_" -ForegroundColor Red
    exit 1
}

New-Item -ItemType Directory -Path $tempDir -Force | Out-Null
Expand-Archive -Path $tempZip -DestinationPath $tempDir -Force
Remove-Item $tempZip -ErrorAction SilentlyContinue

# Find vargc.exe (may be at root or in a subdirectory)
$vargcExe = Get-ChildItem -Path $tempDir -Filter "vargc.exe" -Recurse | Select-Object -First 1

if (-not $vargcExe) {
    Write-Host "Error: vargc.exe not found in the downloaded archive." -ForegroundColor Red
    Remove-Item $tempDir -Recurse -ErrorAction SilentlyContinue
    exit 1
}

$destExe = Join-Path $installDir "vargc.exe"
Copy-Item -Path $vargcExe.FullName -Destination $destExe -Force
Remove-Item $tempDir -Recurse -ErrorAction SilentlyContinue

# ── Step 6: Add to user PATH ──────────────────────────────────────────────────

$currentPath = [Environment]::GetEnvironmentVariable("PATH", "User")
if (-not ($currentPath -split ";" | Where-Object { $_ -eq $installDir })) {
    $newPath = "$currentPath;$installDir"
    [Environment]::SetEnvironmentVariable("PATH", $newPath, "User")
    Write-Host "Added $installDir to user PATH."
} else {
    Write-Host "$installDir is already in PATH."
}

# ── Done ──────────────────────────────────────────────────────────────────────

Write-Host ""
Write-Host "vargc installed to $destExe" -ForegroundColor Green
Write-Host "Run: vargc --version" -ForegroundColor Green
Write-Host ""
Write-Host "Restart your terminal for the PATH update to take effect." -ForegroundColor Yellow
