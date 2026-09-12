#!/usr/bin/env bash
# scripts/install.sh — install agentatlas and activate its skill for every coding agent found.
#
#   curl -fsSL https://raw.githubusercontent.com/adibfarrasy/agentatlas-rs/main/scripts/install.sh | bash
#
# Installs the `agentatlas` binary, copies the `agentatlas` skill into each agent's skill
# directory that exists on this machine (Claude Code, Cursor, Codex, opencode), and — for the
# hook-capable agents (Claude Code, opencode) — installs a pre-tool-use hook that nudges them to
# reach for `agentatlas` before blind grep + whole-file reads.
#
# Env overrides:
#   AGENTATLAS_REPO           "owner/repo" on GitHub; default adibfarrasy/agentatlas-rs.
#   AGENTATLAS_VERSION        a release tag to fetch (e.g. "v0.1.0"); default: latest release.
#   AGENTATLAS_INSTALL_PREFIX install prefix; the binary lands in "$PREFIX/bin". Default: ~/.local.
#   AGENTATLAS_SOURCE=1       force a from-source build (cargo) even when a release binary exists.
set -eu

repo="${AGENTATLAS_REPO:-adibfarrasy/agentatlas-rs}"
prefix="${AGENTATLAS_INSTALL_PREFIX:-$HOME/.local}"
binDir="$prefix/bin"
version="${AGENTATLAS_VERSION:-}"

command -v curl >/dev/null 2>&1 || { echo "install.sh: curl is required" >&2; exit 2; }

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
    # fetch the canonical SKILL.md from the repo
    for skillUrl in \
        "https://raw.githubusercontent.com/${repo}/refs/heads/main/skills/agentatlas/SKILL.md" \
        "https://raw.githubusercontent.com/${repo}/main/skills/agentatlas/SKILL.md"
    do
        if skillText="$( curl -fsSL "$skillUrl" 2>/dev/null )"; then
            break
        fi
    done
    [ -n "${skillText:-}" ] || { echo "install.sh: could not fetch the skill from the repo" >&2; return 1; }

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
    [ "$installed" != "0" ]
}

# ── 2b. the pre-tool-use hook ─────────────────────────────────────────────────────────────────────────
# A passive SKILL.md only works if the agent *chooses* to reach for it. A prehook actively nudges:
# Claude Code gets a PreToolUse hook that injects the reminder before Bash/Read/Grep/Glob; opencode
# gets a plugin that appends it to the first user message of each session. Both are non-blocking.
fetch_hook_asset() { # $1 = repo-relative path; echoes the fetched text on stdout
    local path="$1" t
    for hookUrl in \
        "https://raw.githubusercontent.com/${repo}/refs/heads/main/${path}" \
        "https://raw.githubusercontent.com/${repo}/main/${path}"
    do
        if t="$( curl -fsSL "$hookUrl" 2>/dev/null )"; then
            printf '%s\n' "$t"
            return 0
        fi
    done
    return 1
}

merge_claude_hook() { # $1 = settings.json path, $2 = absolute hook script path; merges a PreToolUse group
    local settings="$1" hook="$2"
    if command -v python3 >/dev/null 2>&1; then
        HOOK="$hook" python3 - "$settings" <<'PY'
import json, os, sys
path, hook = sys.argv[1], os.environ["HOOK"]
try:
    data = json.load(open(path)) if os.path.exists(path) else {}
except (OSError, ValueError):
    data = {}
pre = data.setdefault("hooks", {}).setdefault("PreToolUse", [])
pre = [g for g in pre
       if not (g.get("matcher") == "Bash|Read|Grep|Glob"
               and any(h.get("command") == hook for h in g.get("hooks", [])))]
pre.append({"matcher": "Bash|Read|Grep|Glob",
            "hooks": [{"type": "command", "command": hook}]})
data["hooks"]["PreToolUse"] = pre
with open(path, "w") as f:
    json.dump(data, f, indent=2)
    f.write("\n")
PY
        return 0
    fi
    echo "install.sh: no python3 to merge Claude settings.json — add this PreToolUse hook manually:" >&2
    echo "  $hook" >&2
    return 1
}

install_prehook_claude() {
    local base="${CLAUDE_CONFIG_DIR:-$HOME/.claude}"
    [ -d "$base" ] || return 1
    local hooksDir="$base/hooks"
    mkdir -p "$hooksDir"
    if ! fetch_hook_asset "scripts/hooks/agentatlas-prehook.sh" > "$hooksDir/agentatlas-prehook.sh"; then
        echo "install.sh: could not fetch the Claude Code prehook" >&2; return 1
    fi
    if ! fetch_hook_asset "scripts/hooks/agentatlas-reminder.txt" > "$hooksDir/agentatlas-reminder.txt"; then
        echo "install.sh: could not fetch the prehook reminder" >&2; return 1
    fi
    chmod +x "$hooksDir/agentatlas-prehook.sh"
    merge_claude_hook "$base/settings.json" "$hooksDir/agentatlas-prehook.sh"
    echo "install.sh: activated agentatlas prehook for Claude Code ($hooksDir/agentatlas-prehook.sh)"
    return 0
}

install_prehook_opencode() {
    local base="${XDG_CONFIG_HOME:-$HOME/.config}/opencode"
    [ -d "$base" ] || return 1
    local pluginsDir="$base/plugins"
    mkdir -p "$pluginsDir"
    if ! fetch_hook_asset "scripts/hooks/agentatlas-opencode.ts" > "$pluginsDir/agentatlas-hooks.ts"; then
        echo "install.sh: could not fetch the opencode prehook" >&2; return 1
    fi
    if ! fetch_hook_asset "scripts/hooks/agentatlas-reminder.txt" > "$pluginsDir/agentatlas-reminder.txt"; then
        echo "install.sh: could not fetch the prehook reminder" >&2; return 1
    fi
    echo "install.sh: activated agentatlas prehook for opencode ($pluginsDir/agentatlas-hooks.ts)"
    return 0
}

install_prehook() {
    prehook_installed=0
    if install_prehook_claude; then prehook_installed=1; fi
    if install_prehook_opencode; then prehook_installed=1; fi
    if [ "$prehook_installed" = "0" ]; then
        echo "install.sh: no hook-capable agent dirs (Claude Code, opencode) found — the prehook is at scripts/hooks/ in the repo; copy it manually when you install one"
    fi
    [ "$prehook_installed" = "1" ]
}

install_binary

# ── 3. the skills: one-time choice, asked once ──────────────────────────────────────────────────────────
printf '%s\n' "Install the agentatlas skill + pre-tool-use hook into your coding agents (Claude Code, Cursor, Codex, opencode)?" 
printf '%s\n' "It teaches them to reach for agentatlas before blind grep + whole-file reads. [y/N]"
# read from the terminal, not stdin: under `curl | bash` stdin is the pipe (script bytes),
# and a `read` that consumes it leaves bash parsing a mangled tail.
if [ -t 0 ]; then
    read -r answer || answer=""
else
    read -r answer < /dev/tty || answer=""
fi
case "$answer" in
    y|Y|yes|YES)
        if install_skill; then skills_installed=1; else skills_installed=0; fi
        install_prehook
        ;;
    *) echo "install.sh: skills skipped"; skills_installed=0; prehook_installed=0 ;;
esac

echo
echo "agentatlas installed. Make sure $binDir is on your PATH:"
echo "  export PATH=\"$binDir:\$PATH\""
echo "Try it:  agentatlas --help"
if [ "${skills_installed:-0}" = "1" ]; then
    echo "The skill now teaches Claude / Cursor / Codex / opencode to reach for agentatlas before grep."
fi
if [ "${prehook_installed:-0}" = "1" ]; then
    echo "A pre-tool-use hook now nudges Claude Code and opencode to run agentatlas before grep/read."
fi