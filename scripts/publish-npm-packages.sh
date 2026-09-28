#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  scripts/publish-npm-packages.sh --dry-run
  scripts/publish-npm-packages.sh --execute [--access public]
  scripts/publish-npm-packages.sh --dry-run --stage-local

Publishes npm platform packages first, then the main creditlint wrapper package.
Uses npm publish so GitHub Actions trusted publishing can use OIDC.

Options:
  --dry-run         Validate and run pnpm publish --dry-run for every package.
  --execute         Actually publish packages to npm.
  --stage-local     Build and stage the current host binary into the matching platform package.
  --dist-dir DIR    Stage all platform binaries from DIR. Default: dist/npm.
  --access public   Pass --access public to pnpm publish. Usually only needed for scoped packages.
  --registry URL    Override the project npm registry for this run.
  -h, --help        Show this help.

Expected --dist-dir layout:
  creditlint-darwin-arm64
  creditlint-darwin-x64
  creditlint-linux-arm64
  creditlint-linux-x64
  creditlint-windows-x64.exe
USAGE
}

mode=""
access_args=()
stage_local=0
dist_dir="dist/npm"
registry=""

while [ "$#" -gt 0 ]; do
  case "$1" in
    --dry-run)
      mode="dry-run"
      ;;
    --execute)
      mode="execute"
      ;;
    --stage-local)
      stage_local=1
      ;;
    --dist-dir)
      if [ -z "${2:-}" ]; then
        echo "error: --dist-dir requires a path" >&2
        exit 2
      fi
      dist_dir="$2"
      shift
      ;;
    --access)
      if [ "${2:-}" != "public" ]; then
        echo "error: only --access public is supported" >&2
        exit 2
      fi
      access_args=(--access public)
      shift
      ;;
    --registry)
      if [ -z "${2:-}" ]; then
        echo "error: --registry requires a URL" >&2
        exit 2
      fi
      registry="$2"
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "error: unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
  shift
done

if [ -z "$mode" ]; then
  echo "error: choose --dry-run or --execute" >&2
  usage >&2
  exit 2
fi

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

platforms=(
  "creditlint-darwin-arm64:creditlint-darwin-arm64:creditlint"
  "creditlint-darwin-x64:creditlint-darwin-x64:creditlint"
  "creditlint-linux-arm64:creditlint-linux-arm64:creditlint"
  "creditlint-linux-x64:creditlint-linux-x64:creditlint"
  "creditlint-windows-x64.exe:creditlint-windows-x64:creditlint.exe"
)

packages=(
  "creditlint-darwin-arm64"
  "creditlint-darwin-x64"
  "creditlint-linux-arm64"
  "creditlint-linux-x64"
  "creditlint-windows-x64"
  "creditlint"
)

stage_binary() {
  local source="$1"
  local package="$2"
  local binary_name="$3"
  local destination="packages/${package}/bin/${binary_name}"

  if [ ! -f "$source" ]; then
    echo "missing source binary: $source" >&2
    return 1
  fi

  cp "$source" "$destination"
  chmod 755 "$destination"
  echo "staged $source -> $destination"
}

if [ "$stage_local" -eq 1 ]; then
  case "$(uname -s)-$(uname -m)" in
    Darwin-arm64)
      local_platform="creditlint-darwin-arm64:creditlint-darwin-arm64:creditlint"
      ;;
    Darwin-x86_64)
      local_platform="creditlint-darwin-x64:creditlint-darwin-x64:creditlint"
      ;;
    Linux-aarch64|Linux-arm64)
      local_platform="creditlint-linux-arm64:creditlint-linux-arm64:creditlint"
      ;;
    Linux-x86_64)
      local_platform="creditlint-linux-x64:creditlint-linux-x64:creditlint"
      ;;
    MINGW*-x86_64|MSYS*-x86_64|CYGWIN*-x86_64)
      local_platform="creditlint-windows-x64.exe:creditlint-windows-x64:creditlint.exe"
      ;;
    *)
      echo "error: unsupported local platform for --stage-local: $(uname -s)-$(uname -m)" >&2
      exit 2
      ;;
  esac

  cargo build --release

  IFS=: read -r _ package binary_name <<<"$local_platform"
  local_source="target/release/creditlint"
  if [ "$binary_name" = "creditlint.exe" ]; then
    local_source="target/release/creditlint.exe"
  fi
  stage_binary "$local_source" "$package" "$binary_name"
fi

case "$dist_dir" in
  /*)
    dist_dir_abs="$dist_dir"
    ;;
  *)
    dist_dir_abs="$repo_root/$dist_dir"
    ;;
esac
if [ -d "$dist_dir_abs" ]; then
  for platform in "${platforms[@]}"; do
    IFS=: read -r artifact package binary_name <<<"$platform"
    stage_binary "$dist_dir_abs/$artifact" "$package" "$binary_name"
  done
fi

missing=0
for platform in "${platforms[@]}"; do
  IFS=: read -r _ package binary_name <<<"$platform"
  destination="packages/${package}/bin/${binary_name}"
  if [ ! -f "$destination" ]; then
    echo "missing native binary: $destination" >&2
    missing=1
  fi
done

if [ "$missing" -ne 0 ]; then
  echo "error: stage all native binaries before publishing npm packages" >&2
  echo "hint: place release binaries in $dist_dir/ or use --stage-local for current-platform dry runs" >&2
  exit 2
fi

pnpm install --frozen-lockfile
pnpm --filter creditlint test

publish_args=()
if [ -n "$registry" ]; then
  publish_args+=(--registry "$registry")
fi
if [ "$mode" = "dry-run" ]; then
  publish_args+=(--dry-run)
fi
if [ "${#access_args[@]}" -gt 0 ]; then
  publish_args+=("${access_args[@]}")
fi

# npm publish does not replace pnpm's workspace: protocol versions the way
# pnpm publish does. Strip them so the published package.json has concrete
# version ranges that npm can resolve at install time.
strip_workspace_protocol() {
  node "$(dirname "${BASH_SOURCE[0]}")/strip-workspace-protocol.js" "$1"
}

backup_files=()
restore_package_json() {
  for backup in "${backup_files[@]}"; do
    if [ -f "$backup" ]; then
      mv "$backup" "${backup%.bak}"
    fi
  done
}
trap restore_package_json EXIT

for package in "${packages[@]}"; do
  pkg_json="packages/$package/package.json"
  if [ -f "$pkg_json" ]; then
    cp "$pkg_json" "$pkg_json.bak"
    backup_files+=("$pkg_json.bak")
    strip_workspace_protocol "$pkg_json"
  fi
done

for package in "${packages[@]}"; do
  echo "publishing $package (${mode})"
  (cd "packages/$package" && npm publish "${publish_args[@]}")
done
