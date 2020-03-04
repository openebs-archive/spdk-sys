#!/usr/bin/env bash

# This is a script which automates building of spdk fat library containing all
# spdk, dpdk and optionally the isa-l object files. It would have been nice if
# this was part of spdk makefiles so that anyone could run just configure && make
# to get the fat lib. But it's not a hot candidate for upstreaming since we do this only
# to work around limitations of rust build system, which is not a good reason
# for changing spdk makefiles.
#
# Usage: ./build.sh [extra-spdk-configure-args...]
#  (i.e. ./build.sh --enable-debug)

BASEDIR=$(dirname "$0")
cd "${BASEDIR}"

# checkout spdk sources
[[ -d spdk/.git ]] || git submodule update --init --recursive

# We need to disable some CPU specific optimization flags because we cannot
# know which flavour of x86-64 cpu the binary will run on.
# corei7 with certain cpu flags disabled seems to be a reasonable compromise.

cp defconfig_x86_64-nhm-linuxapp-gcc spdk/dpdk/config/defconfig_x86_64-nhm-linuxapp-gcc

# The current supported CPU extensions we use are:
#
# MODE64 (call)
# CMOV (cmovae)
# AVX (vmovdqa)
# NOVLX (vmovntdq)
# SSE1 (sfence)
# SSE2 (pause)
# SSSE3 (palignr)
# PCLMUL (pclmulqdq)
# SSE41 (pblendvb)
#
# To see the flags enabled per -march=$arg, you can run:
#
#  gcc -Q -march=corei7 --help=target
#
#  It will show the information in a human readable format, to show the
#  preprocessor output:
#
#  gcc -E -dM -march=corei7 - < /dev/null
#
# note that neither the CPU extensions nor the GCC defines, will map directly
# to a CPU instruction, for this one really needs to read the manual
DISABLED_FLAGS="-mno-movbe -mno-lzcnt -mno-bmi -mno-bmi2"

CONFIGURE_OPTS="--with-dpdk-machine=nhm --with-iscsi-initiator --with-rdma --with-uring"
CONFIGURE_OPTS+=" --with-internal-vhost-lib --disable-tests --without-isal --with-crypto"

(cd spdk; CFLAGS=${DISABLED_FLAGS} DPDK_EXTRA_FLAGS=${DISABLED_FLAGS} ./configure \
	${CONFIGURE_OPTS} "$@"
TARGET_ARCHITECTURE=corei7 make -j "$(nproc)"
)

ARCHIVES=
for f in spdk/build/lib/libspdk_*.a; do
	# avoid test mock lib with undefined symbols
	if [[ "$f" != spdk/build/lib/libspdk_ut_mock.a ]]; then
		ARCHIVES="$ARCHIVES $f"
	fi
done

for f in spdk/dpdk/build/lib/librte_*.a; do
	# avoid name clashes - spdk has its own vhost implementation
	if [[ "$f" != spdk/dpdk/build/lib/librte_vhost.a ]]; then
		ARCHIVES="$ARCHIVES $f"
	fi
done

# depending on CI system, this library might not be there which is fine for the crate itself
if [[ -f spdk/intel-ipsec-mb/libIPSec_MB.a ]]; then
	ARCHIVES="$ARCHIVES spdk/intel-ipsec-mb/libIPSec_MB.a"
fi

echo
echo "Constructing libspdk_fat.so from following object archives:"
for a in ${ARCHIVES}; do
	echo "    $a"
done

[[ -d build ]] || mkdir build
cc -shared -o build/libspdk_fat.so \
	-lc -lrdmacm -laio -libverbs -liscsi -lnuma -ldl -lrt -luuid -lcrypto -luring \
	-Wl,--whole-archive ${ARCHIVES} -Wl,--no-whole-archive

echo "If you are not using nix, you should either copy the library to your OS search path or"
echo "set the RUSTFLAGS environment variable manually, or whatever else you want."
