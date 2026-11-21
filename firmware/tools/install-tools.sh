#!/usr/bin/env bash
# Install the pinned firmware tools (Arm GNU Toolchain and picotool) from
# firmware/tools/toolchain.env, verifying each SHA256 before unpacking
# (AD-1, Story 2.1 Decision D1). Used by CI and by local builds alike.
#
# Usage: firmware/tools/install-tools.sh
# Env:   OCB_TOOLS_DIR  install root (default: firmware/.tools, git-ignored)
#
# Supported hosts: Linux x86_64, macOS arm64. Idempotent: an install whose
# .ok stamp matches the pinned URL and checksum is skipped.
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
fw_dir="$(cd "${script_dir}/.." && pwd)"

set -a
# shellcheck source=SCRIPTDIR/toolchain.env disable=SC1091
. "${script_dir}/toolchain.env"
set +a

tools_dir="${OCB_TOOLS_DIR:-${fw_dir}/.tools}"

die() {
    echo "install-tools: error: $*" >&2
    exit 1
}

os="$(uname -s)"
arch="$(uname -m)"
case "${os}/${arch}" in
    Linux/x86_64)
        arm_url="${ARM_TOOLCHAIN_LINUX_X86_64_URL}"
        arm_sha="${ARM_TOOLCHAIN_LINUX_X86_64_SHA256}"
        pt_url="${PICOTOOL_LINUX_X86_64_URL}"
        pt_sha="${PICOTOOL_LINUX_X86_64_SHA256}"
        ;;
    Darwin/arm64)
        arm_url="${ARM_TOOLCHAIN_DARWIN_ARM64_URL}"
        arm_sha="${ARM_TOOLCHAIN_DARWIN_ARM64_SHA256}"
        pt_url="${PICOTOOL_DARWIN_ARM64_URL}"
        pt_sha="${PICOTOOL_DARWIN_ARM64_SHA256}"
        ;;
    *)
        die "unsupported host ${os}/${arch} (supported: Linux/x86_64, Darwin/arm64)"
        ;;
esac

sha256_of() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | cut -d ' ' -f 1
    elif command -v shasum >/dev/null 2>&1; then
        shasum -a 256 "$1" | cut -d ' ' -f 1
    else
        die "neither sha256sum nor shasum is available"
    fi
}

# download URL SHA256 DEST: fetch and verify; delete and fail on mismatch.
download() {
    local url="$1" want="$2" dest="$3" got
    echo "install-tools: downloading ${url}"
    curl -fsSL --retry 3 -o "${dest}" "${url}" || die "download failed: ${url}"
    got="$(sha256_of "${dest}")"
    if [ "${got}" != "${want}" ]; then
        rm -f "${dest}"
        die "checksum mismatch for ${url}: expected ${want}, got ${got}"
    fi
    echo "install-tools: sha256 ok (${want})"
}

stamp_ok() {
    local dir="$1" want="$2"
    [ -d "${dir}" ] && [ -f "${dir}/.ok" ] && [ "$(cat "${dir}/.ok")" = "${want}" ]
}

work="$(mktemp -d)"
trap 'rm -rf "${work}"' EXIT
mkdir -p "${tools_dir}"

# --- Arm GNU Toolchain -------------------------------------------------------
arm_dir="${tools_dir}/arm-gnu-toolchain-${ARM_TOOLCHAIN_VERSION}"
arm_stamp="${arm_url} ${arm_sha}"
if stamp_ok "${arm_dir}" "${arm_stamp}"; then
    echo "install-tools: Arm GNU Toolchain ${ARM_TOOLCHAIN_VERSION} already installed"
else
    download "${arm_url}" "${arm_sha}" "${work}/arm.tar.xz"
    rm -rf "${arm_dir}" "${arm_dir}.partial"
    mkdir -p "${arm_dir}.partial"
    tar -xJf "${work}/arm.tar.xz" -C "${arm_dir}.partial" --strip-components=1
    [ -x "${arm_dir}.partial/bin/arm-none-eabi-gcc" ] || die "arm-none-eabi-gcc not found in the archive"
    printf '%s\n' "${arm_stamp}" >"${arm_dir}.partial/.ok"
    mv "${arm_dir}.partial" "${arm_dir}"
fi

# --- picotool ----------------------------------------------------------------
pt_dir="${tools_dir}/picotool-${PICOTOOL_VERSION}"
pt_stamp="${pt_url} ${pt_sha}"
if stamp_ok "${pt_dir}" "${pt_stamp}"; then
    echo "install-tools: picotool ${PICOTOOL_VERSION} already installed"
else
    case "${pt_url}" in
        *.zip) pt_archive="${work}/picotool.zip" ;;
        *) pt_archive="${work}/picotool.tar.gz" ;;
    esac
    download "${pt_url}" "${pt_sha}" "${pt_archive}"
    rm -rf "${pt_dir}" "${pt_dir}.partial"
    mkdir -p "${pt_dir}.partial"
    case "${pt_archive}" in
        *.zip) unzip -q "${pt_archive}" -d "${pt_dir}.partial" ;;
        *) tar -xzf "${pt_archive}" -C "${pt_dir}.partial" ;;
    esac
    [ -f "${pt_dir}.partial/picotool/picotoolConfig.cmake" ] \
        || die "picotool/picotoolConfig.cmake not found in the archive"
    printf '%s\n' "${pt_stamp}" >"${pt_dir}.partial/.ok"
    mv "${pt_dir}.partial" "${pt_dir}"
fi

# Only sanity print of a downloaded binary (Task 2.3).
"${arm_dir}/bin/arm-none-eabi-gcc" --version | head -n 1

echo
echo "install-tools: done. Export these before configuring the fw-* presets:"
echo "export OCB_ARM_TOOLCHAIN=${arm_dir}"
echo "export OCB_PICOTOOL_DIR=${pt_dir}"
