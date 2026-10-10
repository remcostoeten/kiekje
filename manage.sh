#!/usr/bin/env bash
set -euo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
readonly REPO
readonly APP_ID="dev.remcostoeten.Kiekje"
readonly GITHUB_URL="https://github.com/remcostoeten/kiekje"
readonly CONFIG_FILE="${XDG_CONFIG_HOME:-$HOME/.config}/kiekje/config.toml"
readonly DATA_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/kiekje"
readonly CACHE_DIR="${XDG_CACHE_HOME:-$HOME/.cache}/kiekje"
readonly AUTOSTART_FILE="${XDG_CONFIG_HOME:-$HOME/.config}/autostart/$APP_ID.desktop"
SCRIPT="$(basename "$0")"
readonly SCRIPT

ASSUME_YES=0

MENU_INDEX=0
MENU_TITLE=""
MENU_KEY=""
MENU_LABELS=()
MENU_DESCS=()
PROMPT_VALUE=""

function setup_colors() {
    if [[ -t 1 && -z "${NO_COLOR:-}" ]]; then
        ACCENT=$'\e[38;2;254;81;1m'
        DIM=$'\e[2m'
        BOLD=$'\e[1m'
        GREEN=$'\e[38;2;80;200;120m'
        YELLOW=$'\e[38;2;240;190;60m'
        RED=$'\e[38;2;235;80;80m'
        RESET=$'\e[0m'
    else
        ACCENT="" DIM="" BOLD="" GREEN="" YELLOW="" RED="" RESET=""
    fi
}

function banner() {
    printf '\n  %s%s■ KIEKJE%s %s// %s%s\n\n' "$ACCENT" "$BOLD" "$RESET" "$DIM" "$1" "$RESET"
}

function mascot() {
    local -a art=(
        "wwww................wwww"
        "w......................w"
        "w..........11122.......w"
        "........1111222333......"
        ".......11hh223333334...."
        "......11h222333333444..."
        "......1222233e33e44444.."
        "......2223333e33e44445.."
        "......2233333e44e444555."
        "......2333333444444555.."
        "......33333344444455555."
        ".......3333444444555555."
        ".......333444445555555.."
        "w......3444..45555..66.w"
        "w.......44....555......w"
        "wwww................wwww"
    )
    local -A rgb=(
        [w]="245;235;224" [h]="255;232;200" [e]="26;16;20"
        [1]="255;190;80" [2]="254;148;50" [3]="253;110;34"
        [4]="251;80;46" [5]="252;82;86" [6]="255;100;140"
    )
    local -a beside=("$@")
    local y x top bottom line
    for (( y = 0; y < ${#art[@]}; y += 2 )); do
        line="  "
        for (( x = 0; x < ${#art[y]}; x++ )); do
            top="${art[y]:x:1}" bottom="${art[y + 1]:x:1}"
            if [[ "$top" == . && "$bottom" == . ]]; then
                line+=" "
            elif [[ "$bottom" == . ]]; then
                line+=$'\e[38;2;'"${rgb[$top]}m▀$RESET"
            elif [[ "$top" == . ]]; then
                line+=$'\e[38;2;'"${rgb[$bottom]}m▄$RESET"
            else
                line+=$'\e[38;2;'"${rgb[$top]}"$'m\e[48;2;'"${rgb[$bottom]}m▀$RESET"
            fi
        done
        printf '%s   %s\n' "$line" "${beside[y / 2]:-}"
    done
}

function step() { printf '  %s›%s %s\n' "$ACCENT" "$RESET" "$*"; }
function ok() { printf '  %s✓%s %s\n' "$GREEN" "$RESET" "$*"; }
function warn() { printf '  %s!%s %s\n' "$YELLOW" "$RESET" "$*" >&2; }
function die() { printf '  %s✗%s %s\n' "$RED" "$RESET" "$*" >&2; exit 1; }
function detail() { printf '    %s%-12s%s %s\n' "$DIM" "$1" "$RESET" "$2"; }

function section() { printf '  %s%s%s\n' "$BOLD" "$1" "$RESET"; }
function row() { printf '    %s%-24s%s %s\n' "$ACCENT" "$1" "$RESET" "$2"; }

function confirm() {
    (( ASSUME_YES )) && return 0
    [[ -t 0 ]] || die "$1 Refusing without a terminal; pass --yes."
    local answer
    read -r -p "  ${YELLOW}?${RESET} $1 [y/N] " answer
    [[ "$answer" =~ ^[yY]$ ]]
}

function require() {
    command -v "$1" >/dev/null 2>&1 || die "$1 is not installed."
}

function captures_dir() {
    local folder=""
    if [[ -f "$CONFIG_FILE" ]]; then
        # Reads `folder = "..."` from the [storage] table only.
        folder="$(awk -F'"' '/^\[/{in_storage = ($0 ~ /^\[storage\]/)} in_storage && /^[[:space:]]*folder[[:space:]]*=/{print $2; exit}' "$CONFIG_FILE")"
    fi
    if [[ -n "$folder" ]]; then
        printf '%s\n' "${folder/#\~/$HOME}"
        return
    fi
    local pictures
    pictures="$(xdg-user-dir PICTURES 2>/dev/null || true)"
    [[ -n "$pictures" && "$pictures" != "$HOME" ]] || pictures="$HOME/Pictures"
    printf '%s/Kiekje\n' "$pictures"
}

function prefix_for() {
    if [[ "$1" == system ]]; then printf '/usr\n'; else printf '%s/.local\n' "$HOME"; fi
}

function installed_prefix() {
    local scope
    for scope in user system; do
        local prefix
        prefix="$(prefix_for "$scope")"
        [[ -x "$prefix/bin/kiekje" ]] && { printf '%s\n' "$scope"; return; }
    done
    return 1
}

function as_root_if() {
    if [[ "$1" == system && $EUID -ne 0 ]]; then
        shift
        sudo "$@"
    else
        shift
        "$@"
    fi
}

function stop_running() {
    pgrep -x kiekje >/dev/null 2>&1 || return 0
    step "Stopping the running instance"
    command -v kiekje >/dev/null 2>&1 && kiekje quit >/dev/null 2>&1 || pkill -x kiekje || true
}

function write_autostart() {
    local launch="$1"
    [[ "$launch" == /usr/* ]] && launch=kiekje
    mkdir -p "${AUTOSTART_FILE%/*}"
    cat >"$AUTOSTART_FILE" <<EOF
[Desktop Entry]
Type=Application
Name=Kiekje
Comment=Start the Kiekje tray at login
Exec=$launch daemon
Icon=$APP_ID
Terminal=false
NoDisplay=true
X-KDE-autostart-phase=2
X-KDE-DBUS-Restricted-Interfaces=org.kde.KWin.ScreenShot2
EOF
}

function start_daemon() {
    setsid -f "$1" daemon >/dev/null 2>&1 </dev/null
}

function refresh_desktop_caches() {
    local scope="$1" prefix="$2"
    if command -v gtk-update-icon-cache >/dev/null 2>&1 && [[ -f "$prefix/share/icons/hicolor/index.theme" ]]; then
        as_root_if "$scope" gtk-update-icon-cache -qtf "$prefix/share/icons/hicolor" || true
    fi
    if command -v update-desktop-database >/dev/null 2>&1; then
        as_root_if "$scope" update-desktop-database -q "$prefix/share/applications" || true
    fi
}

function help_main() {
    if [[ -n "$RESET" ]]; then
        printf '\n'
        mascot "" "" \
            "$ACCENT$BOLD■ KIEKJE$RESET" \
            "${DIM}screenshot tray app helper$RESET"
        printf '\n'
    else
        banner "screenshot tray app helper"
    fi
    printf '  %sUSAGE%s  %s <command> [options]\n\n' "$DIM" "$RESET" "$SCRIPT"
    section "COMMANDS"
    row "install"   "Build a release and install it"
    row "uninstall" "Remove an installed build"
    row "build"     "Compile, optionally with fmt, clippy and tests"
    row "update"    "Pull master, rebuild and reinstall"
    row "view"      "Open the gallery or the captures folder"
    row "clean"     "Delete captures from the captures folder"
    row "backup"    "Zip the captures folder"
    row "github"    "Open the repository in the browser"
    row "interactive" "Pick a command from a menu"
    row "help"      "Show help for a command"
    printf '\n'
    section "GLOBAL OPTIONS"
    row "-h, --help" "Show help; works after every command too"
    row "-y, --yes"  "Skip confirmation prompts"
    row "--no-color" "Disable colors (or set NO_COLOR)"
    printf '\n  %sRun %s help <command> for details.%s\n  %sWith no command on a terminal, %s opens the menu.%s\n\n' \
        "$DIM" "$SCRIPT" "$RESET" "$DIM" "$SCRIPT" "$RESET"
}

function help_interactive() {
    banner "interactive"
    printf '  %sUSAGE%s  %s interactive\n\n' "$DIM" "$RESET" "$SCRIPT"
    printf '  Opens a menu of every command, with a submenu where a command\n  takes options.\n\n'
    section "KEYS"
    row "up/down, j/k" "Move the selection"
    row "enter"       "Run the highlighted entry"
    row "1-9"         "Jump to and run that entry"
    row "escape, q"   "Go back; at the top level, leave the menu"
    printf '\n  %sWith no command at all, %s opens the menu on a terminal and prints\n  this help otherwise.%s\n\n' "$DIM" "$SCRIPT" "$RESET"
}

function help_install() {
    banner "install"
    printf '  %sUSAGE%s  %s install [--system] [--skip-build] [--no-autostart]\n\n' "$DIM" "$RESET" "$SCRIPT"
    section "OPTIONS"
    row "--user"       "Install to ~/.local (default, no sudo)"
    row "--system"     "Install to /usr with sudo"
    row "--skip-build" "Install the existing target/release binary"
    row "--no-autostart" "Do not start the tray at login or after installing"
    row "-h, --help"   "Show this help"
    printf '\n'
}

function help_uninstall() {
    banner "uninstall"
    printf '  %sUSAGE%s  %s uninstall [--system] [--purge]\n\n' "$DIM" "$RESET" "$SCRIPT"
    section "OPTIONS"
    row "--user"     "Remove the ~/.local install (default)"
    row "--system"   "Remove the /usr install with sudo"
    row "--purge"    "Also delete settings, library and cache; captures stay"
    row "-y, --yes"  "Do not ask before purging"
    row "-h, --help" "Show this help"
    printf '\n'
}

function help_build() {
    banner "build"
    printf '  %sUSAGE%s  %s build [--debug] [--check]\n\n' "$DIM" "$RESET" "$SCRIPT"
    section "OPTIONS"
    row "--release"  "Optimised build (default)"
    row "--debug"    "Debug build"
    row "--check"    "Run fmt, clippy and tests like CI before building"
    row "-h, --help" "Show this help"
    printf '\n'
}

function help_update() {
    banner "update"
    printf '  %sUSAGE%s  %s update [--no-install]\n\n' "$DIM" "$RESET" "$SCRIPT"
    section "OPTIONS"
    row "--no-install" "Pull and build, but leave the install alone"
    row "-h, --help"   "Show this help"
    printf '\n  %sPulls with --ff-only and refuses on a dirty tree. Reinstalls into the\n  scope that already has kiekje, or ~/.local when none does.%s\n\n' "$DIM" "$RESET"
}

function help_view() {
    banner "view"
    printf '  %sUSAGE%s  %s view [--folder | --settings]\n\n' "$DIM" "$RESET" "$SCRIPT"
    section "OPTIONS"
    row "--gallery"  "Open the kiekje gallery (default)"
    row "--folder"   "Open the captures folder in the file manager"
    row "--settings" "Open kiekje settings"
    row "--path"     "Print the captures folder and exit"
    row "-h, --help" "Show this help"
    printf '\n'
}

function help_clean() {
    banner "clean"
    printf '  %sUSAGE%s  %s clean [--older-than DAYS] [--dry-run] [--thumbs]\n\n' "$DIM" "$RESET" "$SCRIPT"
    section "OPTIONS"
    row "--older-than DAYS" "Only delete captures older than DAYS"
    row ""                  "Only files Kiekje recorded in its library are deleted"
    row "--dry-run"         "List what would be deleted"
    row "--thumbs"          "Also clear the thumbnail cache"
    row "-y, --yes"         "Do not ask for confirmation"
    row "-h, --help"        "Show this help"
    printf '\n  %sTip: run %s backup first.%s\n\n' "$DIM" "$SCRIPT" "$RESET"
}

function help_backup() {
    banner "backup"
    printf '  %sUSAGE%s  %s backup [-o FILE]\n\n' "$DIM" "$RESET" "$SCRIPT"
    section "OPTIONS"
    row "-o, --output FILE" "Write the zip here (default ~/kiekje-captures-<stamp>.zip)"
    row "--with-library"    "Include the library database"
    row "-h, --help"        "Show this help"
    printf '\n'
}

function help_github() {
    banner "github"
    printf '  %sUSAGE%s  %s github [page]\n\n' "$DIM" "$RESET" "$SCRIPT"
    section "PAGES"
    row "repo"     "Repository home (default)"
    row "issues"   "Issues"
    row "pulls"    "Pull requests"
    row "actions"  "CI runs"
    row "releases" "Releases"
    row "--print"  "Print the URL instead of opening it"
    printf '\n'
}

function help_help() {
    banner "help"
    printf '  %sUSAGE%s  %s help [command]\n\n' "$DIM" "$RESET" "$SCRIPT"
}

function wants_help() {
    local arg
    for arg in "$@"; do
        [[ "$arg" == -h || "$arg" == --help ]] && return 0
    done
    return 1
}

function unknown_option() {
    die "Unknown option '$2' for $1. Run $SCRIPT $1 --help."
}

function menu_render() {
    local index="$1" i jump="${#MENU_LABELS[@]}"
    (( jump > 9 )) && jump=9
    printf '\033[H\033[2J'
    printf '\n  %s%s■ KIEKJE%s %s// %s%s\n\n' "$ACCENT" "$BOLD" "$RESET" "$DIM" "$MENU_TITLE" "$RESET"
    for (( i = 0; i < ${#MENU_LABELS[@]}; i++ )); do
        if (( i == index )); then
            printf '  %s▸%s%s%2d%s  %s%-13s%s %s\n' \
                "$ACCENT" "$RESET" "$BOLD" "$((i + 1))" "$RESET" \
                "$BOLD" "${MENU_LABELS[i]}" "$RESET" "${MENU_DESCS[i]}"
        else
            printf '   %s%2d%s  %s%-13s%s %s\n' \
                "$DIM" "$((i + 1))" "$RESET" \
                "$DIM" "${MENU_LABELS[i]}" "$RESET" "${MENU_DESCS[i]}"
        fi
    done
    printf '\n  %s↑↓ or jk move%s  %senter run%s  %s1-%d jump%s  %sescape back%s\n' \
        "$DIM" "$RESET" "$DIM" "$RESET" "$DIM" "$jump" "$RESET" "$DIM" "$RESET"
}

function menu_read_key() {
    local key="" tail
    IFS= read -rsn1 key || return 1
    MENU_KEY="$key"
    [[ "$key" == $'\e' ]] || return 0
    if IFS= read -rsn1 -t 0.05 tail && [[ "$tail" == "[" || "$tail" == "O" ]]; then
        local code=""
        IFS= read -rsn1 -t 0.05 code || code=""
        case "$code" in
            A) MENU_KEY=up ;;
            B) MENU_KEY=down ;;
            *) MENU_KEY=escape ;;
        esac
    else
        MENU_KEY=escape
    fi
}

function menu_run() {
    MENU_TITLE="$1"
    shift
    MENU_LABELS=()
    MENU_DESCS=()
    local item
    for item in "$@"; do
        MENU_LABELS+=("${item%%|*}")
        MENU_DESCS+=("${item#*|}")
    done
    (( ${#MENU_LABELS[@]} )) || die "menu_run needs at least one item"
    local count="${#MENU_LABELS[@]}"
    MENU_INDEX=0
    while true; do
        menu_render "$MENU_INDEX"
        menu_read_key || return 1
        case "$MENU_KEY" in
            q|Q|escape) return 1 ;;
            "") return 0 ;;
            j|J|down) MENU_INDEX=$(( (MENU_INDEX + 1) % count )) ;;
            k|K|up) MENU_INDEX=$(( (MENU_INDEX - 1 + count) % count )) ;;
            [1-9])
                if (( 10#$MENU_KEY <= count )); then
                    MENU_INDEX=$((10#$MENU_KEY - 1))
                    return 0
                fi
                ;;
        esac
    done
}

function menu_pause() {
    local key
    printf '\n  %s·%s press any key to return to the menu' "$DIM" "$RESET"
    IFS= read -rsn1 key || true
}

function menu_action() {
    local status=0
    ( "$@" ) || status=$?
    if (( status != 0 )); then
        printf '\n'
        warn "$1 failed with exit status $status"
    fi
    menu_pause
}

function ask_value() {
    local prompt="$1" pattern="$2" hint="$3" answer
    PROMPT_VALUE=""
    while true; do
        printf '  %s?%s %s ' "$YELLOW" "$RESET" "$prompt"
        IFS= read -r answer || return 1
        if [[ "$answer" =~ $pattern ]]; then
            PROMPT_VALUE="$answer"
            return 0
        fi
        warn "$hint"
    done
}

function menu_install() {
    menu_run "install" \
        "user|Rebuild and install into ~/.local" \
        "user fast|Install the existing release binary into ~/.local" \
        "system|Rebuild and install into /usr with sudo" \
        "system fast|Install the existing release binary into /usr" \
        "back|Return to the menu" || return 0
    case "${MENU_LABELS[MENU_INDEX]}" in
        "user") menu_action cmd_install --user ;;
        "user fast") menu_action cmd_install --user --skip-build ;;
        "system") menu_action cmd_install --system ;;
        "system fast") menu_action cmd_install --system --skip-build ;;
    esac
}

function menu_uninstall() {
    menu_run "uninstall" \
        "detect|Remove whichever install is found" \
        "user|Remove the ~/.local install" \
        "system|Remove the /usr install" \
        "purge|Also delete settings, library and cache; captures stay" \
        "back|Return to the menu" || return 0
    case "${MENU_LABELS[MENU_INDEX]}" in
        detect) menu_action cmd_uninstall ;;
        user) menu_action cmd_uninstall --user ;;
        system) menu_action cmd_uninstall --system ;;
        purge) menu_action cmd_uninstall --purge ;;
    esac
}

function menu_build() {
    menu_run "build" \
        "release|Optimised build (default)" \
        "debug|Debug build" \
        "check|fmt, clippy and tests, then a release build" \
        "check debug|fmt, clippy and tests, then a debug build" \
        "back|Return to the menu" || return 0
    case "${MENU_LABELS[MENU_INDEX]}" in
        release) menu_action cmd_build --release ;;
        debug) menu_action cmd_build --debug ;;
        check) menu_action cmd_build --check ;;
        "check debug") menu_action cmd_build --check --debug ;;
    esac
}

function menu_update() {
    menu_run "update" \
        "all|Pull, rebuild and reinstall" \
        "build only|Pull and build, leave the install alone" \
        "back|Return to the menu" || return 0
    case "${MENU_LABELS[MENU_INDEX]}" in
        all) menu_action cmd_update ;;
        "build only") menu_action cmd_update --no-install ;;
    esac
}

function menu_view() {
    menu_run "view" \
        "gallery|Open the kiekje gallery" \
        "folder|Open the captures folder in the file manager" \
        "settings|Open kiekje settings" \
        "path|Print the captures folder" \
        "back|Return to the menu" || return 0
    case "${MENU_LABELS[MENU_INDEX]}" in
        gallery) menu_action cmd_view --gallery ;;
        folder) menu_action cmd_view --folder ;;
        settings) menu_action cmd_view --settings ;;
        path) menu_action cmd_view --path ;;
    esac
}

function menu_clean() {
    menu_run "clean captures" \
        "preview|List what would be deleted" \
        "all|Delete every capture, after confirmation" \
        "older|Delete captures older than a number of days" \
        "all + thumbs|Delete every capture and clear the thumbnail cache" \
        "back|Return to the menu" || return 0
    case "${MENU_LABELS[MENU_INDEX]}" in
        preview) menu_action cmd_clean --dry-run ;;
        all) menu_action cmd_clean ;;
        older)
            ask_value "Delete captures older than how many days?" '^[0-9]+$' "Enter a number of days." || return 0
            menu_action cmd_clean --older-than "$PROMPT_VALUE"
            ;;
        "all + thumbs") menu_action cmd_clean --thumbs ;;
    esac
}

function menu_backup() {
    menu_run "backup captures" \
        "default|Zip captures to ~/kiekje-captures-<stamp>.zip" \
        "with library|Default name, plus the library database" \
        "custom|Ask where to write the zip" \
        "back|Return to the menu" || return 0
    case "${MENU_LABELS[MENU_INDEX]}" in
        default) menu_action cmd_backup ;;
        "with library") menu_action cmd_backup --with-library ;;
        custom)
            ask_value "Where should the zip go?" '.+' "Enter a file path." || return 0
            menu_action cmd_backup -o "$PROMPT_VALUE"
            ;;
    esac
}

function menu_github() {
    menu_run "github" \
        "repo|Repository home" \
        "issues|Issues" \
        "pulls|Pull requests" \
        "actions|CI runs" \
        "releases|Releases" \
        "back|Return to the menu" || return 0
    case "${MENU_LABELS[MENU_INDEX]}" in
        back) return 0 ;;
        *) menu_action cmd_github "${MENU_LABELS[MENU_INDEX]}" ;;
    esac
}

function menu_help() {
    menu_run "help" \
        "main|Show the full command list" \
        "install|Options for install" \
        "uninstall|Options for uninstall" \
        "build|Options for build" \
        "update|Options for update" \
        "view|Options for view" \
        "clean|Options for clean" \
        "backup|Options for backup" \
        "github|Options for github" \
        "back|Return to the menu" || return 0
    case "${MENU_LABELS[MENU_INDEX]}" in
        back) return 0 ;;
        main) menu_action cmd_help ;;
        *) menu_action cmd_help "${MENU_LABELS[MENU_INDEX]}" ;;
    esac
}

function cmd_interactive() {
    (( $# == 0 )) || unknown_option interactive "$1"
    [[ -t 0 && -t 1 ]] || die "The menu needs a terminal; run $SCRIPT help instead."
    while true; do
        menu_run "interactive" \
            "install|Build a release and install it" \
            "uninstall|Remove an installed build" \
            "build|Compile, optionally with fmt, clippy and tests" \
            "update|Pull main, rebuild and reinstall" \
            "view|Open the gallery or the captures folder" \
            "clean|Delete captures from the captures folder" \
            "backup|Zip the captures folder" \
            "github|Open the repository in the browser" \
            "help|Show help for a command" \
            "quit|Leave the menu" || break
        case "${MENU_LABELS[MENU_INDEX]}" in
            install) menu_install ;;
            uninstall) menu_uninstall ;;
            build) menu_build ;;
            update) menu_update ;;
            view) menu_view ;;
            clean) menu_clean ;;
            backup) menu_backup ;;
            github) menu_github ;;
            help) menu_help ;;
            quit) break ;;
        esac
    done
    printf '%s' "$RESET"
    printf '\n'
}

function cmd_build() {
    local profile=release check=0
    while (( $# )); do
        case "$1" in
            --release) profile=release ;;
            --debug) profile=debug ;;
            --check) check=1 ;;
            *) unknown_option build "$1" ;;
        esac
        shift
    done
    require cargo
    banner "build"
    cd "$REPO"
    if (( check )); then
        step "cargo fmt --check"
        cargo fmt --all -- --check
        step "cargo clippy"
        cargo clippy --all-targets --all-features -- -D warnings
        step "cargo test"
        cargo test
        ok "Checks passed"
    fi
    step "cargo build ($profile)"
    if [[ "$profile" == release ]]; then cargo build --release; else cargo build; fi
    ok "Built target/$profile/kiekje"
    printf '\n'
}

function cmd_install() {
    local scope=user build=1 autostart=1
    while (( $# )); do
        case "$1" in
            --user) scope=user ;;
            --system) scope=system ;;
            --skip-build) build=0 ;;
            --no-autostart) autostart=0 ;;
            *) unknown_option install "$1" ;;
        esac
        shift
    done
    (( build )) && cmd_build --release
    local binary="$REPO/target/release/kiekje" prefix
    prefix="$(prefix_for "$scope")"
    [[ -x "$binary" ]] || die "No release binary; run $SCRIPT build."
    banner "install"
    stop_running
    step "Installing into $prefix"
    as_root_if "$scope" env PREFIX="$prefix" "$REPO/packaging/install.sh" "$binary"
    refresh_desktop_caches "$scope" "$prefix"
    ok "Installed $("$binary" version 2>/dev/null || printf 'kiekje')"
    detail "binary" "$prefix/bin/kiekje"
    detail "launcher" "$prefix/share/applications/$APP_ID.desktop"
    if (( autostart )); then
        write_autostart "$prefix/bin/kiekje"
        detail "autostart" "$AUTOSTART_FILE"
        start_daemon "$prefix/bin/kiekje"
        ok "Started the tray"
    fi
    if [[ "$scope" == user && ":$PATH:" != *":$HOME/.local/bin:"* ]]; then
        warn "$HOME/.local/bin is not on PATH."
    fi
    printf '\n'
}

function cmd_uninstall() {
    local scope="" purge=0
    while (( $# )); do
        case "$1" in
            --user) scope=user ;;
            --system) scope=system ;;
            --purge) purge=1 ;;
            -y|--yes) ASSUME_YES=1 ;;
            *) unknown_option uninstall "$1" ;;
        esac
        shift
    done
    banner "uninstall"
    if [[ -z "$scope" ]]; then
        scope="$(installed_prefix)" || scope=user
    fi
    local prefix
    prefix="$(prefix_for "$scope")"
    local -a targets=(
        "$prefix/bin/kiekje"
        "$prefix/share/applications/$APP_ID.desktop"
        "$prefix/share/licenses/kiekje"
    )
    local icon
    for icon in "$REPO"/resources/icons/hicolor/*/apps/*; do
        targets+=("$prefix/share/icons/hicolor/${icon#"$REPO/resources/icons/hicolor/"}")
    done
    local -a present=()
    local target
    for target in "${targets[@]}"; do
        [[ -e "$target" ]] && present+=("$target")
    done
    if (( ${#present[@]} == 0 )); then
        warn "Nothing installed under $prefix."
    else
        stop_running
        step "Removing ${#present[@]} paths from $prefix"
        as_root_if "$scope" rm -rf -- "${present[@]}"
        refresh_desktop_caches "$scope" "$prefix"
        ok "Uninstalled"
    fi
    if [[ -f "$AUTOSTART_FILE" ]]; then
        rm -f -- "$AUTOSTART_FILE"
        detail "autostart" "removed $AUTOSTART_FILE"
    fi
    if (( purge )); then
        local -a state=("${CONFIG_FILE%/*}" "$DATA_DIR" "$CACHE_DIR")
        for target in "${state[@]}"; do detail "purge" "$target"; done
        if confirm "Delete settings, library and cache?"; then
            rm -rf -- "${state[@]}"
            ok "Purged user state; captures in $(captures_dir) were kept"
        else
            warn "Purge skipped"
        fi
    fi
    printf '\n'
}

function cmd_update() {
    local install=1
    while (( $# )); do
        case "$1" in
            --no-install) install=0 ;;
            *) unknown_option update "$1" ;;
        esac
        shift
    done
    require git
    banner "update"
    cd "$REPO"
    [[ -z "$(git status --porcelain)" ]] || die "Working tree has changes; commit or stash them first."
    local before
    before="$(git rev-parse --short HEAD)"
    step "Pulling $(git rev-parse --abbrev-ref HEAD)"
    git pull --ff-only --quiet
    local after
    after="$(git rev-parse --short HEAD)"
    if [[ "$before" == "$after" ]]; then
        ok "Already up to date at $after"
    else
        ok "Updated $before → $after"
        git --no-pager log --oneline --no-decorate "$before..$after" | sed "s/^/    ${DIM}/;s/\$/${RESET}/"
    fi
    if (( install )); then
        local scope
        scope="$(installed_prefix)" || scope=user
        cmd_install "--$scope"
    else
        cmd_build --release
    fi
}

function cmd_view() {
    local mode=gallery
    while (( $# )); do
        case "$1" in
            --gallery) mode=gallery ;;
            --folder) mode=folder ;;
            --settings) mode=settings ;;
            --path) mode=path ;;
            *) unknown_option view "$1" ;;
        esac
        shift
    done
    local dir
    dir="$(captures_dir)"
    case "$mode" in
        path) printf '%s\n' "$dir" ;;
        folder)
            require xdg-open
            [[ -d "$dir" ]] || die "No captures folder at $dir yet."
            xdg-open "$dir" >/dev/null 2>&1 &
            ok "Opened $dir"
            ;;
        gallery|settings)
            local binary
            binary="$(command -v kiekje || true)"
            [[ -n "$binary" ]] || binary="$REPO/target/release/kiekje"
            [[ -x "$binary" ]] || die "kiekje is not installed or built; run $SCRIPT install."
            setsid -f "$binary" "$mode" >/dev/null 2>&1
            ok "Opened the $mode"
            ;;
    esac
}

function cmd_clean() {
    local days="" dry=0 thumbs=0
    while (( $# )); do
        case "$1" in
            --older-than)
                [[ "${2:-}" =~ ^[0-9]+$ ]] || die "--older-than needs a number of days."
                days="$2"
                shift
                ;;
            --dry-run) dry=1 ;;
            --thumbs) thumbs=1 ;;
            -y|--yes) ASSUME_YES=1 ;;
            *) unknown_option clean "$1" ;;
        esac
        shift
    done
    banner "clean captures"
    local dir
    dir="$(captures_dir)"
    [[ -d "$dir" ]] || die "No captures folder at $dir."
    local library="$DATA_DIR/library.db"
    [[ -f "$library" ]] || die "No library at $library, so there is no record of which files Kiekje made."
    command -v sqlite3 >/dev/null || die "clean needs sqlite3 to read which files Kiekje made."
    local cutoff=""
    [[ -n "$days" ]] && cutoff=$(( $(date +%s) - days * 86400 ))
    local -a files=()
    local path
    while IFS= read -r path; do
        [[ "$path" == "$dir/"* && "${path#"$dir/"}" != */* && -f "$path" ]] || continue
        [[ -z "$cutoff" ]] || (( $(stat -c %Y -- "$path") < cutoff )) || continue
        files+=("$path")
    done < <(sqlite3 -readonly "$library" "SELECT path FROM captures WHERE missing = 0 ORDER BY path")
    detail "folder" "$dir"
    if [[ -n "$days" ]]; then detail "filter" "older than $days days"; else detail "filter" "all captures"; fi
    if (( ${#files[@]} == 0 )); then
        ok "Nothing to delete"
    else
        local size
        size="$(du -ch -- "${files[@]}" | tail -n1 | cut -f1)"
        detail "matches" "${#files[@]} files, $size"
        if (( dry )); then
            printf '%s\n' "${files[@]}" | sed "s|^$dir/|    ${DIM}|;s/\$/${RESET}/"
            ok "Dry run, nothing deleted"
        elif confirm "Delete ${#files[@]} captures ($size)?"; then
            rm -f -- "${files[@]}"
            ok "Deleted ${#files[@]} captures"
        else
            warn "Cancelled"
        fi
    fi
    if (( thumbs && ! dry )); then
        rm -rf -- "$CACHE_DIR/thumbs"
        ok "Cleared the thumbnail cache"
    fi
    printf '\n'
}

function cmd_backup() {
    local output="" library=0
    while (( $# )); do
        case "$1" in
            -o|--output)
                [[ -n "${2:-}" ]] || die "$1 needs a file path."
                output="$2"
                shift
                ;;
            --with-library) library=1 ;;
            *) unknown_option backup "$1" ;;
        esac
        shift
    done
    banner "backup captures"
    local dir
    dir="$(captures_dir)"
    [[ -d "$dir" ]] || die "No captures folder at $dir."
    [[ -n "$output" ]] || output="$HOME/kiekje-captures-$(date +%Y%m%d-%H%M%S).zip"
    [[ "$output" == *.zip ]] || output="$output.zip"
    [[ "$output" == /* ]] || output="$PWD/$output"
    [[ -e "$output" ]] && die "$output already exists."
    local count
    count="$(find "$dir" -type f | wc -l)"
    (( count > 0 )) || die "The captures folder is empty."
    detail "source" "$dir ($count files)"
    step "Writing $output"
    local -a extra=()
    (( library )) && [[ -f "$DATA_DIR/library.db" ]] && extra+=("$DATA_DIR/library.db")
    if command -v zip >/dev/null 2>&1; then
        (cd "$(dirname "$dir")" && zip -qr9 "$output" "$(basename "$dir")")
        (( ${#extra[@]} )) && zip -qj "$output" "${extra[@]}"
    elif command -v bsdtar >/dev/null 2>&1; then
        local -a library_args=()
        (( ${#extra[@]} )) && library_args=(-C "$DATA_DIR" library.db)
        bsdtar -a -cf "$output" -C "$(dirname "$dir")" "$(basename "$dir")" ${library_args[@]+"${library_args[@]}"}
    else
        die "Install zip or bsdtar to make backups."
    fi
    ok "Backed up $count files, $(du -h "$output" | cut -f1)"
    printf '\n'
}

function cmd_github() {
    local page=repo print=0
    while (( $# )); do
        case "$1" in
            repo|issues|pulls|actions|releases) page="$1" ;;
            --print) print=1 ;;
            *) unknown_option github "$1" ;;
        esac
        shift
    done
    local url="$GITHUB_URL"
    [[ "$page" == repo ]] || url="$url/$page"
    if (( print )); then
        printf '%s\n' "$url"
        return
    fi
    require xdg-open
    xdg-open "$url" >/dev/null 2>&1 &
    ok "Opened $url"
}

function cmd_help() {
    local topic="${1:-}"
    case "$topic" in
        "") help_main ;;
        install|uninstall|build|update|view|clean|backup|github|interactive|help) "help_$topic" ;;
        *) die "No help for '$topic'. Commands: install uninstall build update view clean backup github interactive." ;;
    esac
}

function main() {
    local -a rest=()
    local arg
    for arg in "$@"; do
        case "$arg" in
            --no-color) NO_COLOR=1 ;;
            -y|--yes) ASSUME_YES=1 ;;
            *) rest+=("$arg") ;;
        esac
    done
    setup_colors
    set -- "${rest[@]+"${rest[@]}"}"

    if (( $# == 0 )); then
        if [[ -t 0 && -t 1 ]]; then cmd_interactive; else cmd_help; fi
        return
    fi

    local command="$1"
    shift
    case "$command" in
        -h|--help|help) cmd_help "${1:-}" ;;
        install|uninstall|build|update|view|clean|backup|github)
            if wants_help "$@"; then "help_$command"; else "cmd_$command" "$@"; fi
            ;;
        interactive)
            if wants_help "$@"; then help_interactive; else cmd_interactive "$@"; fi
            ;;
        *) die "Unknown command '$command'. Run $SCRIPT --help." ;;
    esac
}

main "$@"
