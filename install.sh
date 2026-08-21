#!/usr/bin/env bash

# Adapted from https://github.com/railwayapp/cli/blob/master/install.sh
# (itself adapted from starship's installer).

help_text="Options

  -V, --verbose
  Enable verbose output for the installer

  -f, -y, --force, --yes
  Skip the confirmation prompt during installation

  -p, --platform
  Override the platform identified by the installer

  -b, --bin-dir
  Override the bin installation directory
  Precedence: --bin-dir > MOB_BIN_DIR > \$MOB_HOME/bin > ~/.mob/bin

  -a, --arch
  Override the architecture identified by the installer

  -B, --base-url
  Override the base URL used for downloading releases

  -r, --remove
  Uninstall mobs

  -h, --help
  Get some help

"

set -eu
printf '\n'

BOLD="$(tput bold 2>/dev/null || printf '')"
GREY="$(tput setaf 0 2>/dev/null || printf '')"
UNDERLINE="$(tput smul 2>/dev/null || printf '')"
RED="$(tput setaf 1 2>/dev/null || printf '')"
GREEN="$(tput setaf 2 2>/dev/null || printf '')"
YELLOW="$(tput setaf 3 2>/dev/null || printf '')"
NO_COLOR="$(tput sgr0 2>/dev/null || printf '')"

SUPPORTED_TARGETS="x86_64-unknown-linux-gnu x86_64-unknown-linux-musl \
  i686-unknown-linux-musl aarch64-unknown-linux-musl \
  arm-unknown-linux-musleabihf x86_64-apple-darwin \
  aarch64-apple-darwin x86_64-pc-windows-msvc \
  i686-pc-windows-msvc aarch64-pc-windows-msvc"

info() {
  printf '%s\n' "${BOLD}${GREY}>${NO_COLOR} $*"
}

debug() {
  if [ -n "${VERBOSE-}" ]; then
    printf '%s\n' "${BOLD}${GREY}>${NO_COLOR} $*"
  fi
}

warn() {
  printf '%s\n' "${YELLOW}! $*${NO_COLOR}"
}

error() {
  printf '%s\n' "${RED}x $*${NO_COLOR}" >&2
}

completed() {
  printf '%s\n' "${GREEN}✓${NO_COLOR} $*"
}

has() {
  command -v "$1" 1>/dev/null 2>&1
}

RANDOM_FOR_SH=$(od -vAn -N4 -tu4 < /dev/urandom | sed 's/\t*$//g')
# $RANDOM is unset under dash and busybox ash; fall back to $$.
RANDOM_FOR_SH=$(echo ${RANDOM_FOR_SH:-$$})

get_tmpfile() {
  suffix="$1"
  if has mktemp; then
    printf "%s%s.%s.%s" "$(mktemp)" "-mobs" "${RANDOM_FOR_SH}" "${suffix}"
  else
    printf "/tmp/mobs.%s" "${suffix}"
  fi
}

# Test if a location is writeable by trying to write to it.
test_writeable() {
  path="${1:-}/test.txt"
  if touch "${path}" 2>/dev/null; then
    rm "${path}"
    return 0
  else
    return 1
  fi
}

default_mob_home() {
  if [ -n "${MOB_HOME-}" ]; then
    printf '%s' "${MOB_HOME}"
    return 0
  fi

  if [ -n "${HOME-}" ]; then
    printf '%s' "${HOME}/.mob"
    return 0
  fi

  return 1
}

tildify() {
  if [ -n "${HOME-}" ]; then
    case "$1" in
      "$HOME"/*) printf '~/%s' "${1#"$HOME"/}" ;;
      "$HOME") printf '~' ;;
      *) printf '%s' "$1" ;;
    esac
  else
    printf '%s' "$1"
  fi
}

# POSIX stand-in for bash's ${var//needle/repl}; this script runs under
# whatever /bin/sh the host provides.
replace_all() {
  str="$1"
  needle="$2"
  repl="$3"
  out=""

  if [ -z "$needle" ]; then
    printf '%s' "$str"
    return 0
  fi

  while :; do
    case "$str" in
      *"$needle"*)
        head=${str%%"$needle"*}
        out=$out$head$repl
        str=${str#*"$needle"}
        ;;
      *) break ;;
    esac
  done

  printf '%s%s' "$out" "$str"
}

shell_quote() {
  printf "'%s'" "$(replace_all "$1" "'" "'\\''")"
}

fish_quote() {
  value="$(replace_all "$1" '\' '\\')"
  value="$(replace_all "$value" "'" "\\'")"
  printf "'%s'" "$value"
}

source_path() {
  path="$1"

  if [ -n "${HOME-}" ]; then
    case "$path" in
      "$HOME"/*) printf '"$HOME/%s"' "${path#"$HOME"/}"; return ;;
    esac
  fi

  shell_quote "$path"
}

fish_source_path() {
  path="$1"

  if [ -n "${HOME-}" ]; then
    case "$path" in
      "$HOME"/*) printf '"$HOME/%s"' "${path#"$HOME"/}"; return ;;
    esac
  fi

  fish_quote "$path"
}

bin_dir_uses_mob_home() {
  [ -n "${MOB_HOME_DIR-}" ] && [ "${BIN_DIR%/}" = "${MOB_HOME_DIR%/}/bin" ]
}

download() {
  file="$1"
  url="$2"
  touch "$file"

  if has curl; then
    cmd="curl --fail --silent --location --output $file $url"
  elif has wget; then
    cmd="wget --quiet --output-document=$file $url"
  elif has fetch; then
    cmd="fetch --quiet --output=$file $url"
  else
    error "No HTTP download program (curl, wget, fetch) found, exiting…"
    return 1
  fi

  $cmd && return 0 || rc=$?

  error "Command failed (exit code $rc): ${BOLD}${cmd}${NO_COLOR}"
  printf "\n" >&2
  info "This is likely due to mobs not yet supporting your configuration."
  info "If you would like to see a build for your configuration,"
  info "please create an issue requesting a build for ${BOLD}${TARGET}${NO_COLOR}:"
  info "${BOLD}${UNDERLINE}https://github.com/mobdotso/cli/issues/new/${NO_COLOR}"
  return $rc
}

unpack() {
  archive=$1
  bin_dir=$2
  sudo=${3-}

  case "$archive" in
    *.tar.gz)
      # VERBOSE is normalized to "v" or "" during option parsing; folding it
      # into the flag bundle keeps busybox tar from reading an empty argument
      # as a member name.
      ${sudo} tar "-${VERBOSE}xzf" "${archive}" -C "${bin_dir}"
      return 0
      ;;
    *.zip)
      UNZIP="" ${sudo} unzip "${archive}" -d "${bin_dir}"
      return 0
      ;;
  esac

  error "Unknown package extension."
  printf "\n"
  info "This almost certainly results from a bug in this script; please file a"
  info "bug report at https://github.com/mobdotso/cli/issues"
  return 1
}

elevate_priv() {
  if ! has sudo; then
    error 'Could not find the command "sudo", needed to get permissions for install.'
    info "If you are on Windows, please run your shell as an administrator, then"
    info "rerun this script. Otherwise, please run this script as root, or install"
    info "sudo."
    exit 1
  fi
  if ! sudo -v; then
    error "Superuser not granted, aborting installation"
    exit 1
  fi
}

install() {
  ext="$1"

  if test_writeable "${BIN_DIR}"; then
    sudo=""
    msg="Installing mobs, please wait…"
  else
    warn "Escalated permissions are required to install to ${BIN_DIR}"
    elevate_priv
    sudo="sudo"
    msg="Installing mobs as root, please wait…"
  fi
  info "$msg"

  archive=$(get_tmpfile "$ext")

  # download to the temp file
  download "${archive}" "${URL}"

  # unpack the temp file to the bin dir, using sudo if required
  unpack "${archive}" "${BIN_DIR}" "${sudo}"
}

# Currently supporting:
#   - win (Git Bash)
#   - darwin
#   - linux
#   - linux_musl (Alpine)
detect_platform() {
  platform="$(uname -s | tr '[:upper:]' '[:lower:]')"

  case "${platform}" in
    msys_nt*) platform="pc-windows-msvc" ;;
    cygwin_nt*) platform="pc-windows-msvc" ;;
    # mingw is Git-Bash
    mingw*) platform="pc-windows-msvc" ;;
    # use the statically compiled musl bins on linux to avoid linking issues.
    linux) platform="unknown-linux-musl" ;;
    darwin) platform="apple-darwin" ;;
  esac

  printf '%s' "${platform}"
}

detect_arch() {
  arch="$(uname -m | tr '[:upper:]' '[:lower:]')"

  case "${arch}" in
    amd64) arch="x86_64" ;;
    armv*) arch="arm" ;;
    arm64) arch="aarch64" ;;
  esac

  # `uname -m` in some cases mis-reports 32-bit OS as 64-bit, so double check
  if [ "${arch}" = "x86_64" ] && [ "$(getconf LONG_BIT)" -eq 32 ]; then
    arch=i686
  elif [ "${arch}" = "aarch64" ] && [ "$(getconf LONG_BIT)" -eq 32 ]; then
    arch=arm
  fi

  printf '%s' "${arch}"
}

detect_target() {
  arch="$1"
  platform="$2"
  target="$arch-$platform"

  if [ "${target}" = "arm-unknown-linux-musl" ]; then
    target="${target}eabihf"
  fi

  printf '%s' "${target}"
}

confirm() {
  if [ -t 0 ] && [ -z "${FORCE-}" ]; then
    printf "%s " "${BOLD}?${NO_COLOR} $* ${BOLD}[y/N]${NO_COLOR}"
    set +e
    read -r yn
    rc=$?
    set -e
    if [ $rc -ne 0 ]; then
      error "Error reading from prompt (please re-run with the '--yes' option)"
      exit 1
    fi
    if [ "$yn" != "y" ] && [ "$yn" != "yes" ]; then
      error "Aborting (please answer \"yes\" to continue)"
      exit 1
    fi
  fi
}

check_bin_dir() {
  bin_dir="${1%/}"

  if [ ! -d "$BIN_DIR" ]; then
    if ! mkdir -p "$BIN_DIR" 2>/dev/null; then
      warn "Escalated permissions are required to create ${BIN_DIR}"
      elevate_priv
      sudo mkdir -p "$BIN_DIR"
    fi
    info "Created directory ${BIN_DIR}"
  fi

  # https://stackoverflow.com/a/11655875
  good=$(
    IFS=:
    for path in $PATH; do
      if [ "${path%/}" = "${bin_dir}" ]; then
        printf 1
        break
      fi
    done
  )

  if [ "${good}" != "1" ]; then
    return 1
  fi
  return 0
}

is_build_available() {
  arch="$1"
  platform="$2"
  target="$3"

  good=$(
    IFS=" "
    for t in $SUPPORTED_TARGETS; do
      if [ "${t}" = "${target}" ]; then
        printf 1
        break
      fi
    done
  )

  if [ "${good}" != "1" ]; then
    error "${arch} builds for ${platform} are not yet available for mobs"
    printf "\n" >&2
    info "If you would like to see a build for your configuration,"
    info "please create an issue requesting a build for ${BOLD}${target}${NO_COLOR}:"
    info "${BOLD}${UNDERLINE}https://github.com/mobdotso/cli/issues/new/${NO_COLOR}"
    printf "\n"
    exit 1
  fi
}

UNINSTALL=0
HELP=0
MOB_HOME_DIR=""
MOB_ENV_FILE=""
MOB_FISH_ENV_FILE=""
PATH_ACTIVATION_PRINTED=0
MOB_PATH_MARKER_BEGIN="# >>> mobs initialize >>>"
MOB_PATH_MARKER_END="# <<< mobs initialize <<<"
SHELL_STARTUP_FILE=""
SHELL_STARTUP_ACTION=""

# Resolve the version to install. Deliberately lazy: called only after --help
# and --remove have had their chance to exit, and skipped outright when the
# caller pinned MOB_VERSION, so an unauthenticated api.github.com rate limit
# cannot break --help, --remove, or a pinned install.
resolve_mob_version() {
  if [ -n "${MOB_VERSION-}" ]; then
    return 0
  fi

  MOB_VERSION=$(curl -s --max-time 15 https://api.github.com/repos/mobdotso/cli/releases/latest \
    | grep -o '"tag_name":[[:space:]]*"v[^"]*"' | cut -d'"' -f4 | cut -c2-) || true

  if [ -z "$MOB_VERSION" ]; then
    error "Could not determine the latest mobs CLI version from GitHub."
    info "This is usually a transient network failure, or GitHub's unauthenticated"
    info "API rate limit (60 requests/hour per IP) on a shared runner."
    info "Pin a version to skip this lookup entirely:"
    info "  ${BOLD}MOB_VERSION=<x.y.z>${NO_COLOR}"
    exit 1
  fi
}

# defaults
if [ -z "${MOB_PLATFORM-}" ]; then
  PLATFORM="$(detect_platform)"
else
  PLATFORM="${MOB_PLATFORM}"
fi

if MOB_HOME_DIR="$(default_mob_home)"; then
  if [ -n "${MOB_BIN_DIR-}" ]; then
    BIN_DIR="${MOB_BIN_DIR}"
  else
    BIN_DIR="${MOB_HOME_DIR%/}/bin"
  fi
else
  if [ -n "${MOB_BIN_DIR-}" ]; then
    BIN_DIR="${MOB_BIN_DIR}"
  else
    BIN_DIR=""
  fi
fi

if [ -z "${MOB_ARCH-}" ]; then
  ARCH="$(detect_arch)"
else
  ARCH="${MOB_ARCH}"
fi

if [ -z "${MOB_BASE_URL-}" ]; then
  BASE_URL="https://github.com/mobdotso/cli/releases"
else
  BASE_URL="${MOB_BASE_URL}"
fi

# parse argv variables
while [ "$#" -gt 0 ]; do
  case "$1" in
    -p | --platform)
      PLATFORM="$2"
      shift 2
      ;;
    -b | --bin-dir)
      BIN_DIR="$2"
      shift 2
      ;;
    -a | --arch)
      ARCH="$2"
      shift 2
      ;;
    -B | --base-url)
      BASE_URL="$2"
      shift 2
      ;;

    -V | --verbose)
      VERBOSE=1
      shift 1
      ;;
    -f | -y | --force | --yes)
      FORCE=1
      shift 1
      ;;
    -r | --remove | --uninstall)
      UNINSTALL=1
      shift 1
      ;;
    -h | --help)
      HELP=1
      shift 1
      ;;
    -p=* | --platform=*)
      PLATFORM="${1#*=}"
      shift 1
      ;;
    -b=* | --bin-dir=*)
      BIN_DIR="${1#*=}"
      shift 1
      ;;
    -a=* | --arch=*)
      ARCH="${1#*=}"
      shift 1
      ;;
    -B=* | --base-url=*)
      BASE_URL="${1#*=}"
      shift 1
      ;;
    -V=* | --verbose=*)
      VERBOSE="${1#*=}"
      shift 1
      ;;
    -f=* | -y=* | --force=* | --yes=*)
      FORCE="${1#*=}"
      shift 1
      ;;

    *)
      error "Unknown option: $1"
      exit 1
      ;;
  esac
done

# non-empty VERBOSE enables verbose untarring
if [ -n "${VERBOSE-}" ]; then
  VERBOSE=v
else
  VERBOSE=
fi

write_env_files() {
  if ! bin_dir_uses_mob_home; then
    return 0
  fi

  if ! mkdir -p "$MOB_HOME_DIR"; then
    warn "Could not create $(tildify "$MOB_HOME_DIR"); skipping activation file."
    return 0
  fi

  MOB_ENV_FILE="$MOB_HOME_DIR/env"
  MOB_FISH_ENV_FILE="$MOB_HOME_DIR/env.fish"

  if [ -e "$MOB_ENV_FILE" ] && { [ ! -f "$MOB_ENV_FILE" ] || [ ! -w "$MOB_ENV_FILE" ]; }; then
    warn "Could not write $(tildify "$MOB_ENV_FILE"); skipping activation file."
    MOB_ENV_FILE=""
    MOB_FISH_ENV_FILE=""
    return 0
  fi

  quoted_mob_home="$(shell_quote "$MOB_HOME_DIR")"
  fish_mob_home="$(fish_quote "$MOB_HOME_DIR")"

  if ! {
    printf 'export MOB_HOME=%s\n' "$quoted_mob_home"
    printf 'case ":$PATH:" in\n'
    printf '  *":$MOB_HOME/bin:"*) ;;\n'
    printf '  *) export PATH="$MOB_HOME/bin:$PATH" ;;\n'
    printf 'esac\n'
  } > "$MOB_ENV_FILE"; then
    warn "Could not write $(tildify "$MOB_ENV_FILE"); skipping activation file."
    MOB_ENV_FILE=""
    MOB_FISH_ENV_FILE=""
    return 0
  fi

  if [ -e "$MOB_FISH_ENV_FILE" ] && { [ ! -f "$MOB_FISH_ENV_FILE" ] || [ ! -w "$MOB_FISH_ENV_FILE" ]; }; then
    warn "Could not write $(tildify "$MOB_FISH_ENV_FILE"); fish users may need to add $(tildify "$BIN_DIR") to PATH manually."
    MOB_FISH_ENV_FILE=""
    return 0
  fi

  if ! {
    printf 'set -gx MOB_HOME %s\n' "$fish_mob_home"
    printf 'if not contains "$MOB_HOME/bin" $PATH\n'
    printf '  set -gx PATH "$MOB_HOME/bin" $PATH\n'
    printf 'end\n'
  } > "$MOB_FISH_ENV_FILE"; then
    warn "Could not write $(tildify "$MOB_FISH_ENV_FILE"); fish users may need to add $(tildify "$BIN_DIR") to PATH manually."
    MOB_FISH_ENV_FILE=""
  fi
}

# Each command goes on its own line, bolded in full, so a triple-click copies
# a runnable command and the leading `source` cannot be read as prose.
print_path_commands() {
  commands="$1"

  printf '%s\n' "$commands" | while IFS= read -r command; do
    printf '  %s\n' "${BOLD}${command}${NO_COLOR}"
  done
}

activation_command() {
  shell_name=""
  env_file="$MOB_ENV_FILE"

  if [ -n "${SHELL-}" ]; then
    shell_name="$(basename "$SHELL")"
  fi

  if [ "$shell_name" = "fish" ]; then
    if [ -n "$MOB_FISH_ENV_FILE" ]; then
      env_file="$MOB_FISH_ENV_FILE"
    else
      return 1
    fi

    printf 'source %s' "$(fish_source_path "$env_file")"
    return 0
  fi

  if [ -n "$env_file" ]; then
    printf 'source %s' "$(source_path "$env_file")"
    return 0
  fi

  return 1
}

configure_shell_startup() {
  contents="$1"
  shell_name=""
  rc_file=""

  SHELL_STARTUP_FILE=""
  SHELL_STARTUP_ACTION=""

  if [ -z "${HOME-}" ]; then
    return 1
  fi

  if [ -n "${SHELL-}" ]; then
    shell_name="$(basename "$SHELL")"
  fi

  case "$shell_name" in
    fish)
      rc_file="$HOME/.config/fish/config.fish"
      ;;
    zsh)
      rc_file="$HOME/.zshrc"
      ;;
    bash)
      if [ -f "$HOME/.bash_profile" ]; then
        rc_file="$HOME/.bash_profile"
      else
        rc_file="$HOME/.bashrc"
      fi
      ;;
    *)
      return 1
      ;;
  esac

  if [ -e "$rc_file" ] && { [ ! -f "$rc_file" ] || [ ! -w "$rc_file" ]; }; then
    warn "Could not update $(tildify "$rc_file"); add $(tildify "$BIN_DIR") to PATH manually."
    return 1
  fi

  rc_dir="$(dirname "$rc_file")"
  if ! mkdir -p "$rc_dir"; then
    warn "Could not create $(tildify "$rc_dir"); add $(tildify "$BIN_DIR") to PATH manually."
    return 1
  fi

  if [ -f "$rc_file" ]; then
    if grep -qF "$MOB_PATH_MARKER_BEGIN" "$rc_file"; then
      SHELL_STARTUP_ACTION="Updated"
    else
      SHELL_STARTUP_ACTION="Added"
    fi
  else
    SHELL_STARTUP_ACTION="Created"
  fi

  tmp_file="$(get_tmpfile shell)"
  if [ -f "$rc_file" ]; then
    if ! awk -v begin="$MOB_PATH_MARKER_BEGIN" -v end="$MOB_PATH_MARKER_END" '
      $0 == begin { skip = 1; next }
      $0 == end { skip = 0; next }
      !skip { print }
    ' "$rc_file" > "$tmp_file"; then
      rm -f "$tmp_file"
      warn "Could not update $(tildify "$rc_file"); add $(tildify "$BIN_DIR") to PATH manually."
      return 1
    fi
  else
    : > "$tmp_file"
  fi

  {
    printf '\n%s\n' "$MOB_PATH_MARKER_BEGIN"
    printf '%s\n' "$contents"
    printf '%s\n' "$MOB_PATH_MARKER_END"
  } >> "$tmp_file"

  if [ -f "$rc_file" ]; then
    if ! cat "$tmp_file" > "$rc_file"; then
      rm -f "$tmp_file"
      warn "Could not update $(tildify "$rc_file"); add $(tildify "$BIN_DIR") to PATH manually."
      return 1
    fi
    rm -f "$tmp_file"
  elif ! mv "$tmp_file" "$rc_file"; then
    rm -f "$tmp_file"
    warn "Could not create $(tildify "$rc_file"); add $(tildify "$BIN_DIR") to PATH manually."
    return 1
  fi

  SHELL_STARTUP_FILE="$rc_file"
  return 0
}

configure_shell_path() {
  if bin_dir_uses_mob_home; then
    quoted_mob_home="$(shell_quote "$MOB_HOME_DIR")"
    fish_mob_home="$(fish_quote "$MOB_HOME_DIR")"
    bash_line='export PATH="$MOB_HOME/bin:$PATH"'
    bash_contents="export MOB_HOME=$quoted_mob_home
$bash_line"
    fish_line='set -gx PATH "$MOB_HOME/bin" $PATH'
    fish_contents="set -gx MOB_HOME $fish_mob_home
$fish_line"
  else
    quoted_bin_dir="$(shell_quote "$BIN_DIR")"
    fish_bin_dir="$(fish_quote "$BIN_DIR")"
    bash_line="export PATH=$quoted_bin_dir:\"\$PATH\""
    bash_contents="$bash_line"
    fish_line="set -gx PATH $fish_bin_dir \$PATH"
    fish_contents="$fish_line"
  fi

  shell_name=""
  if [ -n "${SHELL-}" ]; then
    shell_name="$(basename "$SHELL")"
  fi

  path_commands="$bash_contents"
  startup_contents="$bash_contents"
  if [ "$shell_name" = "fish" ]; then
    path_commands="$fish_contents"
    startup_contents="$fish_contents"
  fi

  if activation="$(activation_command)"; then
    path_commands="$activation"
    startup_contents="$activation"
  fi

  warn "mobs was installed to $(tildify "$BIN_DIR"), but this shell does not resolve 'mobs' from there yet."
  if configure_shell_startup "$startup_contents"; then
    info "$SHELL_STARTUP_ACTION mobs PATH setup in $(tildify "$SHELL_STARTUP_FILE")"
    info "New terminals will have mobs available automatically."
  else
    info "To make mobs available in new terminals, add this command to your shell startup file:"
    print_path_commands "$startup_contents"
  fi
  info "To use mobs in this terminal, run:"
  print_path_commands "$path_commands"
  PATH_ACTIVATION_PRINTED=1
}

if [ "$UNINSTALL" = 1 ]; then
  confirm "Are you sure you want to uninstall mobs?"

  msg=""
  sudo=""
  mob_bin="$(command -v mobs 2>/dev/null || true)"

  if [ -z "$mob_bin" ] && [ -x "$BIN_DIR/mobs" ]; then
    mob_bin="$BIN_DIR/mobs"
  fi

  if [ -z "$mob_bin" ]; then
    error "Could not find mobs on PATH or at $BIN_DIR/mobs"
    exit 1
  fi

  info "REMOVING mobs"

  if test_writeable "$(dirname "$mob_bin")"; then
    sudo=""
    msg="Removing mobs, please wait…"
  else
    warn "Escalated permissions are required to remove ${mob_bin}"
    elevate_priv
    sudo="sudo"
    msg="Removing mobs as root, please wait…"
  fi

  info "$msg"
  ${sudo} rm "$mob_bin"

  info "Removed mobs"
  exit 0
fi

if [ "$HELP" = 1 ]; then
  echo "${help_text}"
  exit 0
fi

# Everything past this point needs a concrete version; --help and --remove
# are already done, so this is the first place the lookup can be required.
resolve_mob_version

printf "  %s\n" "${UNDERLINE}Configuration${NO_COLOR}"
info "${BOLD}Version${NO_COLOR}:  ${BOLD}${MOB_VERSION}${NO_COLOR}"
info "${BOLD}Bin directory${NO_COLOR}:  ${BOLD}${BIN_DIR}${NO_COLOR}"
info "${BOLD}Platform${NO_COLOR}:  ${BOLD}${PLATFORM}${NO_COLOR}"
info "${BOLD}Arch${NO_COLOR}:  ${BOLD}${ARCH}${NO_COLOR}"
printf '\n'

TARGET="$(detect_target "${ARCH}" "${PLATFORM}")"

is_build_available "${ARCH}" "${PLATFORM}" "${TARGET}"

EXT=tar.gz
case "${PLATFORM}" in
  pc-windows-msvc) EXT=zip ;;
esac

URL="${BASE_URL}/download/v${MOB_VERSION}/mobs-v${MOB_VERSION}-${TARGET}.${EXT}"
debug "Tarball URL: ${UNDERLINE}${BOLD}${URL}${NO_COLOR}"
confirm "Install mobs ${BOLD}latest${NO_COLOR} to ${BOLD}${BIN_DIR}${NO_COLOR}?"
check_bin_dir "${BIN_DIR}" || true

write_env_files
install "${EXT}"
completed "mobs v${MOB_VERSION} installed"

if ! check_bin_dir "${BIN_DIR}"; then
  configure_shell_path
fi

if [ "$PATH_ACTIVATION_PRINTED" = 0 ]; then
  printf '\n'
  info "Run ${BOLD}mobs login${NO_COLOR} to connect your mob.so account."
fi

printf '\n'
