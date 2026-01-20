#!/bin/bash
# Aegis DB Installation Script
# Installs the 'aegis' command globally

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INSTALL_DIR="${AEGIS_INSTALL_DIR:-$HOME/.local/bin}"
AEGIS_HOME="$SCRIPT_DIR"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

echo ""
echo -e "${CYAN}╔═══════════════════════════════════════════╗${NC}"
echo -e "${CYAN}║         ${GREEN}Aegis DB Installer${CYAN}                ║${NC}"
echo -e "${CYAN}║     Multi-Paradigm Database Platform      ║${NC}"
echo -e "${CYAN}╚═══════════════════════════════════════════╝${NC}"
echo ""

# Check for required tools
check_requirements() {
    local missing=()

    if ! command -v cargo &> /dev/null; then
        missing+=("cargo (Rust)")
    fi

    if ! command -v trunk &> /dev/null; then
        missing+=("trunk (cargo install trunk)")
    fi

    if [ ${#missing[@]} -ne 0 ]; then
        echo -e "${RED}Missing required tools:${NC}"
        for tool in "${missing[@]}"; do
            echo "  - $tool"
        done
        echo ""
        echo "Please install them and try again."
        exit 1
    fi
}

# Create install directory if needed
setup_install_dir() {
    if [ ! -d "$INSTALL_DIR" ]; then
        echo -e "${BLUE}[INFO]${NC} Creating $INSTALL_DIR..."
        mkdir -p "$INSTALL_DIR"
    fi
}

# Check if install dir is in PATH
check_path() {
    if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
        echo ""
        echo -e "${YELLOW}[NOTE]${NC} $INSTALL_DIR is not in your PATH."
        echo ""
        echo "Add this line to your ~/.bashrc or ~/.zshrc:"
        echo ""
        echo -e "  ${GREEN}export PATH=\"\$PATH:$INSTALL_DIR\"${NC}"
        echo ""
        echo "Then run: source ~/.bashrc (or ~/.zshrc)"
        echo ""
        NEEDS_PATH_UPDATE=true
    fi
}

# Build the project
build_project() {
    echo -e "${BLUE}[INFO]${NC} Building Aegis Server..."
    cd "$AEGIS_HOME"
    cargo build -p aegis-server --release

    echo -e "${BLUE}[INFO]${NC} Building Aegis Dashboard..."
    cd "$AEGIS_HOME/crates/aegis-dashboard"
    trunk build --release

    echo -e "${GREEN}[OK]${NC} Build complete!"
}

# Install the aegis command
install_command() {
    echo -e "${BLUE}[INFO]${NC} Installing 'aegis' command to $INSTALL_DIR..."

    # Create the aegis wrapper script
    cat > "$INSTALL_DIR/aegis" << EOF
#!/bin/bash
# Aegis DB CLI - Installed $(date +%Y-%m-%d)
# Home: $AEGIS_HOME

export AEGIS_HOME="$AEGIS_HOME"
exec "\$AEGIS_HOME/scripts/dev.sh" "\$@"
EOF

    chmod +x "$INSTALL_DIR/aegis"
    echo -e "${GREEN}[OK]${NC} Installed 'aegis' command"
}

# Main installation
main() {
    echo -e "${BLUE}[INFO]${NC} Checking requirements..."
    check_requirements
    echo -e "${GREEN}[OK]${NC} All requirements met"
    echo ""

    setup_install_dir

    echo ""
    read -p "Build Aegis DB? This may take a few minutes. [Y/n] " -n 1 -r
    echo ""

    if [[ ! $REPLY =~ ^[Nn]$ ]]; then
        build_project
    fi

    echo ""
    install_command
    check_path

    echo ""
    echo -e "${GREEN}╔═══════════════════════════════════════════╗${NC}"
    echo -e "${GREEN}║       Installation Complete!              ║${NC}"
    echo -e "${GREEN}╚═══════════════════════════════════════════╝${NC}"
    echo ""
    echo "Usage:"
    echo ""
    echo -e "  ${CYAN}aegis start${NC}     Start server + dashboard"
    echo -e "  ${CYAN}aegis stop${NC}      Stop all services"
    echo -e "  ${CYAN}aegis status${NC}    Check service status"
    echo -e "  ${CYAN}aegis restart${NC}   Restart all services"
    echo -e "  ${CYAN}aegis logs${NC}      View logs"
    echo ""
    echo "After starting, access:"
    echo ""
    echo -e "  Dashboard:  ${CYAN}http://localhost:8000${NC}"
    echo -e "  API:        ${CYAN}http://localhost:9090${NC}"
    echo ""
    echo -e "  Login:      ${CYAN}demo / demo${NC}"
    echo ""

    if [ "$NEEDS_PATH_UPDATE" = true ]; then
        echo -e "${YELLOW}Don't forget to update your PATH! (see above)${NC}"
        echo ""
    fi
}

# Handle uninstall
if [ "$1" = "uninstall" ]; then
    echo -e "${BLUE}[INFO]${NC} Uninstalling aegis..."
    rm -f "$INSTALL_DIR/aegis"
    echo -e "${GREEN}[OK]${NC} Uninstalled"
    exit 0
fi

main
