#!/usr/bin/env bash

kiekje_detect_package_manager() {
    if command -v pacman >/dev/null 2>&1; then
        echo "pacman"
        return 0
    fi
    if command -v apt-get >/dev/null 2>&1; then
        echo "apt-get"
        return 0
    fi
    if command -v dnf >/dev/null 2>&1; then
        echo "dnf"
        return 0
    fi
    if command -v zypper >/dev/null 2>&1; then
        echo "zypper"
        return 0
    fi
    return 1
}

kiekje_core_dependency_packages() {
    local manager="$1"
    case "$manager" in
        pacman) echo "grim wl-clipboard" ;;
        apt-get) echo "grim wl-clipboard" ;;
        dnf) echo "grim wl-clipboard" ;;
        zypper) echo "grim wl-clipboard" ;;
        *) return 1 ;;
    esac
}

kiekje_hyprland_package_hint() {
    local manager="$1"
    case "$manager" in
        pacman) echo "sudo pacman -S hyprland" ;;
        apt-get) echo "Install Hyprland or make window capture optional on your system." ;;
        dnf) echo "Install Hyprland or make window capture optional on your system." ;;
        zypper) echo "Install Hyprland or make window capture optional on your system." ;;
        *) echo "Install Hyprland or make window capture optional on your system." ;;
    esac
}

kiekje_print_dependency_status() {
    local missing_core=()
    local missing_optional=()

    command -v grim >/dev/null 2>&1 || missing_core+=("grim")
    command -v wl-copy >/dev/null 2>&1 || missing_core+=("wl-copy (provided by wl-clipboard)")
    command -v hyprctl >/dev/null 2>&1 || missing_optional+=("hyprctl (optional, needed for window capture)")

    if [[ "${#missing_core[@]}" -eq 0 && "${#missing_optional[@]}" -eq 0 ]]; then
        echo "Dependencies look good: grim, wl-copy, and hyprctl are available."
        return 0
    fi

    echo "Dependency check:"
    if [[ "${#missing_core[@]}" -gt 0 ]]; then
        echo "  Missing core tools:"
        printf '    - %s\n' "${missing_core[@]}"
    fi
    if [[ "${#missing_optional[@]}" -gt 0 ]]; then
        echo "  Missing optional tools:"
        printf '    - %s\n' "${missing_optional[@]}"
    fi

    if manager="$(kiekje_detect_package_manager)"; then
        if [[ "${#missing_core[@]}" -gt 0 ]]; then
            echo "  Suggested install:"
            case "$manager" in
                pacman)
                    echo "    sudo pacman -S $(kiekje_core_dependency_packages "$manager")"
                    ;;
                apt-get)
                    echo "    sudo apt-get update && sudo apt-get install -y $(kiekje_core_dependency_packages "$manager")"
                    ;;
                dnf)
                    echo "    sudo dnf install -y $(kiekje_core_dependency_packages "$manager")"
                    ;;
                zypper)
                    echo "    sudo zypper install $(kiekje_core_dependency_packages "$manager")"
                    ;;
            esac
        fi
        if [[ "${#missing_optional[@]}" -gt 0 ]]; then
            echo "  Optional Hyprland tools:"
            echo "    $(kiekje_hyprland_package_hint "$manager")"
        fi
    else
        echo "  No supported package manager detected for automatic guidance."
    fi

    return 1
}

kiekje_install_core_dependencies() {
    local assume_yes="${1:-0}"
    local manager packages

    if command -v grim >/dev/null 2>&1 && command -v wl-copy >/dev/null 2>&1; then
        echo "Core dependencies already installed."
        return 0
    fi

    manager="$(kiekje_detect_package_manager)" || {
        echo "Could not detect a supported package manager for automatic dependency installation." >&2
        return 1
    }
    packages="$(kiekje_core_dependency_packages "$manager")" || {
        echo "No package mapping for package manager: $manager" >&2
        return 1
    }

    case "$manager" in
        pacman)
            if [[ "$assume_yes" -eq 1 ]]; then
                sudo pacman -Sy --needed --noconfirm $packages
            else
                sudo pacman -Sy --needed $packages
            fi
            ;;
        apt-get)
            sudo apt-get update
            if [[ "$assume_yes" -eq 1 ]]; then
                sudo apt-get install -y $packages
            else
                sudo apt-get install $packages
            fi
            ;;
        dnf)
            if [[ "$assume_yes" -eq 1 ]]; then
                sudo dnf install -y $packages
            else
                sudo dnf install $packages
            fi
            ;;
        zypper)
            if [[ "$assume_yes" -eq 1 ]]; then
                sudo zypper --non-interactive install $packages
            else
                sudo zypper install $packages
            fi
            ;;
    esac
}
