#!/usr/bin/env bash
# Varg Installer for Linux / macOS
# Usage: curl -sSfL https://raw.githubusercontent.com/LupusMalusDeviant/VARG/main/install.sh | sh

set -e

REPO="LupusMalusDeviant/VARG"
INSTALL_DIR="$HOME/.varg/bin"

echo ""
echo "============================================"
echo "      Varg Installer for Linux / macOS     "
echo "============================================"
echo ""

# ── Step 1: Check for Rust / cargo ───────────────────────────────────────────

if ! command -v cargo > /dev/null 2>&1; then
    if ! command -v rustup > /dev/null 2>&1; then
        echo "Rust not found. Installing via rustup..."
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        # shellcheck disable=SC1091
        source "$HOME/.cargo/env"
        echo "Rust installed successfully."
    else
        echo "rustup found but cargo not in PATH. Sourcing cargo env..."
        # shellcheck disable=SC1091
        source "$HOME/.cargo/env"
    fi
else
    echo "Rust found: $(rustc --version)"
fi

# ── Step 2: Detect OS ─────────────────────────────────────────────────────────

OS="$(uname -s)"
case "$OS" in
    Linux*)   PLATFORM="linux" ;;
    Darwin*)  PLATFORM="macos" ;;
    *)
        echo "Unsupported OS: $OS"
        exit 1
        ;;
esac
echo "Detected platform: $PLATFORM"

# ── Step 3: Fetch latest GitHub release ──────────────────────────────────────

echo ""
echo "Fetching latest Varg release from GitHub..."
RELEASE_JSON="$(curl -sL "https://api.github.com/repos/${REPO}/releases/latest")"

TAG_NAME="$(echo "$RELEASE_JSON" | grep '"tag_name"' | head -1 | sed 's/.*"tag_name": *"\([^"]*\)".*/\1/')"
echo "Latest release: $TAG_NAME"

# ── Step 4: Find the right asset ─────────────────────────────────────────────

DOWNLOAD_URL="$(echo "$RELEASE_JSON" \
    | grep '"browser_download_url"' \
    | grep -i "$PLATFORM" \
    | head -1 \
    | sed 's/.*"browser_download_url": *"\([^"]*\)".*/\1/')"

if [ -z "$DOWNLOAD_URL" ]; then
    # Fallback: look for zip with linux in name generically
    DOWNLOAD_URL="$(echo "$RELEASE_JSON" \
        | grep '"browser_download_url"' \
        | grep -i 'linux\|macos\|darwin' \
        | head -1 \
        | sed 's/.*"browser_download_url": *"\([^"]*\)".*/\1/')"
fi

if [ -z "$DOWNLOAD_URL" ]; then
    echo "Error: No $PLATFORM asset found in release $TAG_NAME."
    echo "Available assets:"
    echo "$RELEASE_JSON" | grep '"browser_download_url"' | sed 's/.*: *"\([^"]*\)".*/  \1/'
    exit 1
fi

echo "Downloading: $DOWNLOAD_URL"

# ── Step 5: Download and extract ─────────────────────────────────────────────

TEMP_DIR="$(mktemp -d)"
TEMP_ZIP="$TEMP_DIR/varg.zip"

curl -sL "$DOWNLOAD_URL" -o "$TEMP_ZIP"

cd "$TEMP_DIR"
unzip -q "$TEMP_ZIP"

# Find the vargc binary (may be at root or in a subdirectory)
VARGC_BIN="$(find "$TEMP_DIR" -name "vargc" -not -name "*.zip" | head -1)"

if [ -z "$VARGC_BIN" ]; then
    echo "Error: vargc binary not found in the downloaded archive."
    rm -rf "$TEMP_DIR"
    exit 1
fi

# ── Step 6: Install ───────────────────────────────────────────────────────────

mkdir -p "$INSTALL_DIR"
cp "$VARGC_BIN" "$INSTALL_DIR/vargc"
chmod +x "$INSTALL_DIR/vargc"
rm -rf "$TEMP_DIR"

echo "Installed vargc to $INSTALL_DIR/vargc"

# ── Step 7: Add to PATH in shell config files ─────────────────────────────────

EXPORT_LINE="export PATH=\"\$HOME/.varg/bin:\$PATH\""

for RC_FILE in "$HOME/.bashrc" "$HOME/.zshrc"; do
    if [ -f "$RC_FILE" ] || [ "$RC_FILE" = "$HOME/.bashrc" ]; then
        if ! grep -qF '.varg/bin' "$RC_FILE" 2>/dev/null; then
            echo "" >> "$RC_FILE"
            echo "# Varg compiler" >> "$RC_FILE"
            echo "$EXPORT_LINE" >> "$RC_FILE"
            echo "Added PATH entry to $RC_FILE"
        else
            echo "$RC_FILE already contains .varg/bin — skipping."
        fi
    fi
done

# ── Done ──────────────────────────────────────────────────────────────────────

echo ""
echo "vargc installed successfully."
echo "Restart your terminal or run: source ~/.bashrc"
echo ""
echo "Quick start:"
echo "  vargc --version"
echo "  vargc build hello.varg"
