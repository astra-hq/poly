#!/usr/bin/env bash
set -euo pipefail

MODE=""
DRY_RUN=0
YES=0
SKIP_POLY_CHECK=0
ICLOUD_ROOT="$HOME/Library/Mobile Documents/com~apple~CloudDocs/poly"

CONFIG_LOCAL="$HOME/.poly"
APP_LOCAL="$HOME/Library/Application Support/com.poly.ai"

usage() {
  cat <<'EOF'
Usage:
  scripts/setup-icloud-sync.sh --move-to-icloud --dry-run
  scripts/setup-icloud-sync.sh --move-to-icloud --yes
  scripts/setup-icloud-sync.sh --link-from-icloud --dry-run
  scripts/setup-icloud-sync.sh --link-from-icloud --yes

Modes:
  --move-to-icloud
      Existing machine. Moves local Poly data into iCloud Drive:
        ~/.poly                                      -> iCloud poly/config
        ~/Library/Application Support/com.poly.ai   -> iCloud poly/application
      Then creates symlinks at the original local paths.

  --link-from-icloud
      Clean/new machine. Requires iCloud poly/config and poly/application to
      already exist, then creates local symlinks pointing to them.

Options:
  --dry-run
      Print what would happen without changing the filesystem.

  --yes
      Required for real filesystem changes.

  --icloud-dir PATH
      Override the default iCloud Poly root. Default:
      ~/Library/Mobile Documents/com~apple~CloudDocs/poly

  --skip-poly-check
      Skip the best-effort check that Poly is not running.

Warnings:
  Quit Poly before running this script.
  Do not run Poly on two Macs at the same time against iCloud-synced app data.
  The app data directory contains SQLite files; concurrent iCloud writes can
  corrupt or conflict.
EOF
}

fail() {
  echo "ERROR: $*" >&2
  exit 1
}

note() {
  echo "==> $*"
}

run() {
  if [[ "$DRY_RUN" -eq 1 ]]; then
    printf 'DRY-RUN:'
    printf ' %q' "$@"
    printf '\n'
  else
    "$@"
  fi
}

is_empty_dir() {
  local dir="$1"
  [[ -d "$dir" ]] || return 1
  [[ -z "$(find "$dir" -mindepth 1 -maxdepth 1 -print -quit)" ]]
}

has_data() {
  local path="$1"
  if [[ -L "$path" ]]; then
    return 0
  fi
  if [[ -d "$path" ]]; then
    ! is_empty_dir "$path"
    return $?
  fi
  [[ -e "$path" ]]
}

target_for_local() {
  local local_path="$1"
  case "$local_path" in
    "$CONFIG_LOCAL") printf '%s/config' "$ICLOUD_ROOT" ;;
    "$APP_LOCAL") printf '%s/application' "$ICLOUD_ROOT" ;;
    *) fail "unknown local path: $local_path" ;;
  esac
}

is_expected_link() {
  local local_path="$1"
  local target_path="$2"
  [[ -L "$local_path" ]] || return 1
  [[ "$(readlink "$local_path")" == "$target_path" ]]
}

check_platform() {
  [[ "$(uname -s)" == "Darwin" ]] || fail "this script is macOS-only"
}

check_poly_not_running() {
  [[ "$SKIP_POLY_CHECK" -eq 0 ]] || return 0
  if pgrep -x "Poly" >/dev/null 2>&1 || pgrep -f "com.poly.ai" >/dev/null 2>&1; then
    fail "Poly appears to be running. Quit Poly before changing data symlinks."
  fi
}

check_icloud_available() {
  local mobile_docs
  mobile_docs="$(dirname "$ICLOUD_ROOT")"
  [[ -d "$mobile_docs" ]] || fail "iCloud Drive directory not found: $mobile_docs"
}

require_real_run_confirmation() {
  if [[ "$DRY_RUN" -eq 0 && "$YES" -eq 0 ]]; then
    fail "real filesystem changes require --yes. Re-run with --dry-run first if unsure."
  fi
}

prepare_parent() {
  local path="$1"
  local parent
  parent="$(dirname "$path")"
  run mkdir -p "$parent"
}

move_one_to_icloud() {
  local local_path="$1"
  local target_path
  target_path="$(target_for_local "$local_path")"

  note "Mapping: $local_path -> $target_path"

  if is_expected_link "$local_path" "$target_path"; then
    note "Already linked: $local_path"
    return 0
  fi

  if [[ -L "$local_path" ]]; then
    fail "$local_path is a symlink, but not to $target_path"
  fi

  if has_data "$local_path" && has_data "$target_path"; then
    fail "both local and iCloud paths contain data; resolve manually:\n  local:  $local_path\n  iCloud: $target_path"
  fi

  prepare_parent "$target_path"

  if [[ -e "$local_path" ]]; then
    if has_data "$target_path"; then
      if [[ -d "$local_path" ]] && is_empty_dir "$local_path"; then
        run rmdir "$local_path"
      else
        fail "local path exists but cannot be safely replaced: $local_path"
      fi
    else
      if [[ -d "$target_path" ]] && is_empty_dir "$target_path"; then
        run rmdir "$target_path"
      fi
      run mv "$local_path" "$target_path"
    fi
  elif [[ ! -e "$target_path" ]]; then
    run mkdir -p "$target_path"
  fi

  prepare_parent "$local_path"
  if [[ ! -e "$local_path" && ! -L "$local_path" ]]; then
    run ln -s "$target_path" "$local_path"
  fi

  verify_link "$local_path" "$target_path"
}

link_one_from_icloud() {
  local local_path="$1"
  local target_path
  target_path="$(target_for_local "$local_path")"

  note "Mapping: $local_path -> $target_path"

  [[ -d "$target_path" ]] || fail "iCloud target does not exist: $target_path"
  has_data "$target_path" || fail "iCloud target is empty: $target_path"

  if is_expected_link "$local_path" "$target_path"; then
    note "Already linked: $local_path"
    return 0
  fi

  if [[ -L "$local_path" ]]; then
    if [[ ! -e "$local_path" ]]; then
      run unlink "$local_path"
    else
      fail "$local_path is a symlink, but not to $target_path"
    fi
  elif has_data "$local_path"; then
    fail "local path already contains data; resolve manually before linking: $local_path"
  elif [[ -d "$local_path" ]] && is_empty_dir "$local_path"; then
    run rmdir "$local_path"
  fi

  prepare_parent "$local_path"
  run ln -s "$target_path" "$local_path"
  verify_link "$local_path" "$target_path"
}

verify_link() {
  local local_path="$1"
  local target_path="$2"

  if [[ "$DRY_RUN" -eq 1 ]]; then
    note "Would verify link: $local_path -> $target_path"
    return 0
  fi

  is_expected_link "$local_path" "$target_path" || fail "link verification failed: $local_path"
  note "Linked: $local_path -> $target_path"
}

parse_args() {
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --move-to-icloud)
        [[ -z "$MODE" ]] || fail "choose only one mode"
        MODE="move"
        ;;
      --link-from-icloud)
        [[ -z "$MODE" ]] || fail "choose only one mode"
        MODE="link"
        ;;
      --dry-run)
        DRY_RUN=1
        ;;
      --yes)
        YES=1
        ;;
      --icloud-dir)
        shift
        [[ $# -gt 0 ]] || fail "--icloud-dir requires a path"
        ICLOUD_ROOT="$1"
        ;;
      --skip-poly-check)
        SKIP_POLY_CHECK=1
        ;;
      -h|--help)
        usage
        exit 0
        ;;
      *)
        fail "unknown argument: $1"
        ;;
    esac
    shift
  done

  [[ -n "$MODE" ]] || fail "choose --move-to-icloud or --link-from-icloud"
}

main() {
  parse_args "$@"
  check_platform
  check_icloud_available
  check_poly_not_running
  require_real_run_confirmation

  note "iCloud Poly root: $ICLOUD_ROOT"
  note "Do not run Poly concurrently on multiple Macs with this shared app data."

  case "$MODE" in
    move)
      run mkdir -p "$ICLOUD_ROOT"
      move_one_to_icloud "$CONFIG_LOCAL"
      move_one_to_icloud "$APP_LOCAL"
      ;;
    link)
      link_one_from_icloud "$CONFIG_LOCAL"
      link_one_from_icloud "$APP_LOCAL"
      ;;
    *)
      fail "internal mode error"
      ;;
  esac

  note "Done. Launch Poly only after iCloud Drive has finished syncing."
}

main "$@"
