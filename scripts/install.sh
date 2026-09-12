#!/usr/bin/env bash
# scripts/install.sh — install agentatlas and activate its skill for every coding agent found.
#
#   curl -fsSL https://raw.githubusercontent.com/adibfarrasy/agentatlas.rs/main/scripts/install.sh | bash
#
# Installs the `agentatlas` binary and copies the `agentatlas` skill into each agent's skill
# directory that exists on this machine: Claude Code, Cursor, Codex, opencode. The skill is what
# teaches the agent to reach for `agentatlas` before blind grep + whole-file reads.
#
# Env overrides:
#   AGENTATLAS_REPO           "owner/repo" on GitHub; default adibfarrasy/agentatlas.rs.
#   AGENTATLAS_VERSION        a release tag to fetch (e.g. "v0.1.0"); default: latest release.
#   AGENTATLAS_INSTALL_PREFIX install prefix; the binary lands in "$PREFIX/bin". Default: ~/.local.
#   AGENTATLAS_SOURCE=1       force a from-source build (cargo) even when a release binary exists.
#   AGENTATLAS_YES=1          skip the interactive confirmation (CI / scripted installs).
#   AGENTATLAS_SKIP_SKILLS=1  install the binary only, leave agent configs alone.
set -eu

repo="${AGENTATLAS_REPO:-adibfarrasy/agentatlas.rs}"
prefix="${AGENTATLAS_INSTALL_PREFIX:-$HOME/.local}"
binDir="$prefix/bin"
version="${AGENTATLAS_VERSION:-}"

command -v curl >/dev/null 2>&1 || { echo "install.sh: curl is required" >&2; exit 2; }

confirm() {
    [ "${AGENTATLAS_YES:-0}" = "1" ] && return 0
    printf '%s\n' "This installs agentatlas to $prefix and copies its skill into your agents' configs."
    printf '%s\n' "Continue? [y/N]"
    read -r answer || exit 1
    case "$answer" in
        y|Y|yes|YES) return 0 ;;
        *) echo "aborted."; exit 1 ;;
    esac
}

# ── 1. the binary ──────────────────────────────────────────────────────────────────────────────────────
install_binary() {
    osName="$( uname -s )"
    archName="$( uname -m )"
    case "$osName" in
        Darwin) assetOs=macos ;;
        Linux)  assetOs=linux ;;
        *) assetOs="" ;;
    esac
    case "$archName" in
        arm64|aarch64) assetArch=arm64 ;;
        x86_64|amd64)  assetArch=x64 ;;
        *) assetArch="" ;;
    esac

    if [ "${AGENTATLAS_SOURCE:-0}" != "1" ] && [ -n "$assetOs" ] && [ -n "$assetArch" ]; then
        # try a prebuilt release binary
        if [ -n "$version" ]; then
            apiUrl="https://api.github.com/repos/${repo}/releases/tags/${version}"
        else
            apiUrl="https://api.github.com/repos/${repo}/releases/latest"
        fi
        assetName="agentatlas-${version}-${assetOs}-${assetArch}.tar.gz"
        assetUrl="$( curl -fsSL "$apiUrl" 2>/dev/null | sed -n "s/.*\"browser_download_url\": \"\(.*${assetOs}-${assetArch}\.tar\.gz\)\".*/\1/p" | head -1 || true )"
        if [ -n "$assetUrl" ]; then
            echo "install.sh: downloading $assetUrl"
            tmp="$( mktemp -d )"
            curl -fsSL "$assetUrl" | tar -xz -C "$tmp"
            mkdir -p "$binDir"
            install -m 0755 "$tmp/agentatlas" "$binDir/agentatlas"
            rm -rf "$tmp"
            echo "install.sh: installed $binDir/agentatlas"
            return 0
        fi
        echo "install.sh: no release binary for ${assetOs}/${assetArch} — building from source"
    else
        echo "install.sh: building from source"
    fi

    # fallback: build from source (requires cargo)
    command -v cargo >/dev/null 2>&1 || { echo "install.sh: no prebuilt release and cargo is missing — cannot build from source" >&2; exit 2; }
    tmp="$( mktemp -d )"
    git clone -q --depth 1 "https://github.com/${repo}.git" "$tmp/agentatlas" || { echo "install.sh: clone failed" >&2; rm -rf "$tmp"; exit 1; }
    ( cd "$tmp/agentatlas" && cargo build --release )
    mkdir -p "$binDir"
    install -m 0755 "$tmp/agentatlas/target/release/agentatlas" "$binDir/agentatlas"
    rm -rf "$tmp"
    echo "install.sh: built and installed $binDir/agentatlas"
}

# ── 2. the skill ───────────────────────────────────────────────────────────────────────────────────────
install_skill() {
    [ "${AGENTATLAS_SKIP_SKILLS:-0}" = "1" ] && { echo "install.sh: skills skipped (AGENTATLAS_SKIP_SKILLS=1)"; return 0; }
    # fetch the canonical SKILL.md from the repo
    skillUrl="https://raw.githubusercontent.com/${repo}/main/skills/agentatlas/SKILL.md"
    skillText="$( curl -fsSL "$skillUrl" 2>/dev/null )" || { echo "install.sh: could not fetch the skill from $skillUrl" >&2; return 1; }

    installed=0
    install_to() { # $1 = agent name, $2 = skill dir (only installed if it already exists)
        local dir="$2/agentatlas"
        if [ -d "$2" ]; then
            mkdir -p "$dir"
            printf '%s\n' "$skillText" > "$dir/SKILL.md"
            echo "install.sh: activated agentatlas skill for $1 ($dir/SKILL.md)"
            installed=1
        fi
    }
    install_to "Claude Code" "${CLAUDE_CONFIG_DIR:-$HOME/.claude}/skills"
    install_to "Cursor"      "$HOME/.cursor/skills"
    install_to "Codex"       "${CODEX_HOME:-$HOME/.codex}/skills"
    install_to "Codex"       "${AGENTS_HOME:-$HOME/.agents}/skills"
    install_to "opencode"    "${XDG_CONFIG_HOME:-$HOME/.config}/opencode/skills"
    if [ "$installed" = "0" ]; then
        echo "install.sh: no coding-agent skill dirs found — the skill is at skills/agentatlas/SKILL.md in the repo; copy it manually when you install an agent"
    fi
}

confirm
install_binary
install_skill

echo
echo "agentatlas installed. Make sure $binDir is on your PATH:"
echo "  export PATH=\"$binDir:\$PATH\""
echo "Try it:  agentatlas --help"
echo "The skill now teaches Claude / Cursor / Codex / opencode to reach for agentatlas before grep."