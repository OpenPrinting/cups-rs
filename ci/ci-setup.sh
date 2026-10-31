#!/bin/sh
# ci/ci-setup.sh
#
# CI helper for building cups-rs against several CUPS releases on both native
# and QEMU-emulated runners. build.rs already selects the right bindings via
# pkg-config (`cups3` when the force-cups3 feature is on, `cups` otherwise),
# so all this script does is make one of three CUPS releases available and
# then build and test against it.
#
# Subcommands:
#   deps              install build dependencies
#   cups <kind>       provide libcups; <kind> is one of:
#                       system-2x    distro libcups2-dev    (CUPS 2.4.x)
#                       source-2.5.x OpenPrinting/cups@master    (CUPS 2.5.x)
#                       source-3.x   OpenPrinting/libcups@master (libcups3)
#   build-test <kind> cargo build + test with the feature <kind> needs
#
# The script runs as root inside emulation containers and via sudo on native
# runners; it detects which automatically.
set -eu

# Classify a failure so CI (and humans) can tell an upstream-dependency
# breakage apart from a real cups-rs failure. Each invocation handles exactly
# one subcommand, so $1 identifies which side failed: providing the CUPS
# stack (built from @master on the source-* legs, an unpinned moving target)
# vs building/testing cups-rs itself. The source-* matrix legs are
# non-blocking precisely because of the former class of transient upstream
# breakage; this label makes the distinction explicit on both native and
# emulated legs (they run the same script).
_subcmd="${1:-}"
_classify_failure() {
	rc=$?
	[ "$rc" -eq 0 ] && exit 0
	case "$_subcmd" in
		cups)
			echo "::error::UPSTREAM-DEP-FAILED: 'cups' failed while building CUPS (from @master on the source-* legs). This is an upstream-dependency breakage, not a cups-rs bug." ;;
		build-test)
			echo "::error::CUPS-RS-FAILED: cups-rs build/test failed." ;;
	esac
	exit "$rc"
}
trap _classify_failure EXIT

SUDO=""
[ "$(id -u)" -eq 0 ] || SUDO="sudo"

# Make apt completely non-interactive. Native GitHub runners ship needrestart,
# whose service-restart prompt otherwise hangs the job forever; the emulated
# containers do not have it, which is why only the native legs stalled.
export DEBIAN_FRONTEND=noninteractive
export NEEDRESTART_MODE=a
export NEEDRESTART_SUSPEND=1

# A source-built CUPS installs its .pc files under /usr/lib/pkgconfig or
# /usr/lib/<multiarch>/pkgconfig; make sure pkg-config (and therefore
# build.rs) can find them.
ma=$(gcc -dumpmachine 2>/dev/null || echo "")
PKG_CONFIG_PATH="/usr/lib/pkgconfig${ma:+:/usr/lib/$ma/pkgconfig}:/usr/local/lib/pkgconfig${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}"
export PKG_CONFIG_PATH

apt_install() {
	$SUDO apt-get update --fix-missing -y
	$SUDO apt-get install -y "$@"
}

cmd_deps() {
	# Union of what bindgen needs, what CUPS 2's own pkg-config pulls in
	# (gnutls, avahi), and what building a source CUPS needs. The source
	# builds ship their own ./configure, so no autoconf/automake/libtool.
	apt_install \
		pkg-config clang libclang-dev build-essential \
		libavahi-client-dev libgnutls28-dev libnss-mdns \
		libpng-dev libssl-dev zlib1g-dev
}

# build_autoconf <url> <ref> <submodule-flag> [configure-args...]
build_autoconf() {
	url="$1"; ref="$2"; sub="$3"; shift 3
	echo "ci-setup: building $url @ $ref"
	src="$(mktemp -d)"
	git clone --depth 1 --branch "$ref" $sub "$url" "$src"
	( cd "$src"
	  ./configure --prefix=/usr "$@"
	  make -j"$(nproc)"
	  $SUDO make install )
	$SUDO ldconfig || true
}

cmd_cups() {
	kind="$1"
	case "$kind" in
		system-2x)
			apt_install libcups2-dev
			;;
		source-2.5.x)
			# CUPS 2.5 (OpenPrinting/cups master) ships cups.pc, which is
			# what build.rs's pkg-config probe looks for.
			build_autoconf https://github.com/OpenPrinting/cups.git master "" \
				--disable-systemd
			;;
		source-3.x)
			build_autoconf https://github.com/OpenPrinting/libcups.git master \
				"--recurse-submodules"
			;;
		*)
			echo "ci-setup: unknown cups kind: $kind" >&2; exit 2 ;;
	esac
}

cmd_build_test() {
	kind="$1"
	case "$kind" in
		system-2x|source-2.5.x) feature=force-cups2 ;;
		source-3.x) feature=force-cups3 ;;
		*)
			echo "ci-setup: unknown cups kind: $kind" >&2; exit 2 ;;
	esac

	echo "ci-setup: building against:"
	pkg-config --modversion cups3 2>/dev/null || pkg-config --modversion cups

	cargo build --locked --features "$feature"
	cargo test --locked --features "$feature"
}

case "${1:-}" in
	deps)       cmd_deps ;;
	cups)       shift; cmd_cups "$@" ;;
	build-test) shift; cmd_build_test "$@" ;;
	*)
		echo "usage: ci-setup.sh {deps | cups <kind> | build-test <kind>}" >&2
		exit 2 ;;
esac
