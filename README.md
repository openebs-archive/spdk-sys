Rust bindings for spdk.

# Quick start

1.  Checkout spdk sources
    ```bash
    git submodule update --init --recursive
    ```
2.  Install spdk dependencies:
    ```bash
    sudo spdk/scripts/pkgdep.sh
    ```
3.  `nasm` is an optional dependency which must be installed if you want to make use of crypto and ISA-L:
    ```bash
    sudo apt-get install nasm
    ```
4.  Build monolithic spdk library:
    ```bash
    ./build.sh --with-isal --with-crypto
    ```
5.  Install spdk header files (libs are installed too but those are not needed):
    ```bash
    cd spdk && make install
    ```
5.  Copy the library to a place where it can be found by the linker:
    ```bash
    sudo cp build/libspdk_fat.so /usr/local/lib/
    ```
6.  Put spdk-sys crate to dependencies in Cargo.toml of your project and start using it.

# build.sh

`build.sh` is a helper script for building spdk fat library. It is quite
rough and may not fit all use cases. Configuring & building spdk without
using it is perfectly fine. The script just shows a possible way how to
do that. The only thing which matters is that at the end of build process
there must be a library named `libspdk_fat.so` somewhere in standard
library path. We will continue to evaluate this process and try to make
the spdk-sys crate as easy as possible to use.

# Design

## Problem statement

Creating a crate with bindings for spdk lib is challenging:


1. The public interface of spdk lib contains countless number of function, constant and structure definitions. That makes creating the bindings by hand unfeasible. rust’s bindgen must be used. The result is a huge file with all public member definitions in rust with tens thousands of lines, which makes the compilation slower. In fact we don’t generate bindings for all spdk header files, but just some and even then the file is big enough.
2. spdk is modular framework and contains a lot of optional modules which can but don’t have to be included. Depending on which modules are used, the right header files must be used for generating rust bindings and the right libraries must be used for linking. The question is how to master this complexity without having hard coded list of headers and libs in a build script.
3. spdk is very much optimised for the cpu it was compiled on. This goes much further than just different cpu architectures. Even various x86 64-bit processors are not compatible between each other. It is non-trivial to produce a binary which works on let’s say all x86 64-bit cpus manufactured in last 10 years.
4. Linking spdk libraries together with rust program is troublesome because some spdk libs are not included in the resulting binary. That’s because there is no explicit dependency on some of the libs. The libs are rather plugable modules which if loaded by the dynamic linker (shlib) or present in the binary (static) register themselves with spdk using “constructor” functions which are executed before the main starts. Linker cannot detect such dependencies and therefore ignores the libraries as it thinks they are not needed. However spdk cannot even start without some of such modules. So this is even more fundamental problem than the previous ones.

The purpose of this doc is to suggest a solution for the last problem mentioned above. As for the first three problems we assume that:


1. Compilation time does not concern us.
2. Size of the produced library/binary does not concern us. We expose all modules and functionality which spdk provides even if not used by the application in the end.
3. The only supported cpu architecture is x86_64 and the cpu must not be too old.

Though there are ways how to address first two problems in rust using “features”, it would require more work for the initial implementation and there is also maintenance burden as spdk libraries and headers get removed or added between spdk releases. That said it would be a good extension of spdk sys crate later when it gets more mature.

## Goals

We wish to have a spdk-sys crate which:

1. follows the best practises for sys crates in rust
2. is a general rust interface to spdk usable by any project making use of spdk
3. allow using modified spdk which differs from the upstream

## Best practices

Best practices follow from articles listed in the links section and exemplar rust sys crates (libgit2-sys).


- **static vs dynamic:** sys crates for libraries come in two basic flavours depending on how they link to the library: static or dynamic. In most cases static libraries are preferred because they minimise number of build and runtime dependencies. The program just works after being installed without having to do other prerequisite steps like using package manager to install the dependencies. This makes the program more robust and less error prone user experience.
- **building the C library**: A neat optional feature of many sys crates is that they can build the library without relying on it being installed on the system. It avoids problems and unexpected errors during the build stage. The sys crate is used as a dependency in another project and usually it will be a dependency of dependency of dependency … Errors about missing libs from deep of the build chain don’t contribute to a good user experience. sys crates bundle the source code of the library which they wrap, in form of a git submodule. And many of them don’t even use the build system of the library and have their own rules how to build it in the build.rs script. The important fact to note is that if the sys crate builds the library, then it is used exclusively for **static libs**. The built product which is an object archive is stored in a temporary build directory and apps depending on it take it from there during a link phase. This cannot be done for dynamic libraries as the build script would have to install the shlib to a system location requiring root privileges which is inappropriate action for a build script. Storing the shlib in a build directory is possible, but it would have to be distributed along with the application binary and the path of a temporary build directory would have to be hardcoded (using -rpath) in the app binary which is fragile and complicated.

Switching between different options is done by using rust “features” or environment variables. The default should be set to the most popular way of using the library which depends on the platform and the library itself. Detecting if the lib is installed on a system is done almost exclusively using pkg-config.

## Reality of SPDK

SPDK can be built static or dynamic. It is a set of following libraries:

SPDK:

    app_rpc bdev bdevio bdev_delay bdev_error bdev_gpt bdev_iscsi bdev_lvol bdev_malloc bdev_null bdev_nvme bdev_passthru bdev_id bdev_rpc bdev_split bdev_virtio blob blob_bdev blobfs conf copy copy_it env_dpdk event event_bdev event_copy event_iscsi event_nbd event_net event_nvmf event_scsi event_vhost event_vmd ftl ioat iscsi json jsonrpc log log_rpc lvol nbd net notify nvme nvmf rpc rte_vhost scsi sock sock_posix thread tce tce_rpc util vhost virtio vmd

DPDK:

    bus_pci bus_vdev cmdline compressdev cryptodev eal ethdev hash krgs mbuf mempool mempool_bucket mempool_ring meter net pci ring vhost

ISA-L:

    isal

There is no monolithic libspdk.so and libdpdk.so containing all libraries above. When building spdk with `--with-shared` configure option, DPDK libs are built as static. This is a bug in spdk build system and can be easily fixed.

## Applying the best practices to SPDK

The most preferred solution would be to build spdk object archives and link them statically to app. We bang a head against a wall when doing so. Object archives are linked with as-needed or no-whole-archive linker flag in rust and all libs which are not explicitly used by the app are omitted. Since there is no way how to change the default linker behaviour in rust, the only viable
workaround is to pretend that we use all of the libraries by referencing a symbol from each of them. The problem is that some of the libs have only private symbols and we can’t reference them anyhow without patching them. There is now way how to make that when using vanilla spdk (from upstream). Unless the rust build system is enhanced to support `-whole-archive` linker option (ticket https://github.com/rust-lang/rust/issues/56306), we have to use shared libraries.

Shared libraries suffer from the same problem as static libs as rust build system uses `as-needed` linker flag. But there is an elegant workaround for that. If we create one big shared library out of all smaller ones then it will be surely referenced because there will be surely at least one call to the library - if there was none then it would not make sense to require the lib by the app in the first place - and the dynamic lib loader must load it all to the memory in one piece . The only problem is that we must create this fat library (as I will be calling it) ourselves. Good news is that it is super simple. We just need to take all object archives and combine them into a single shlib:


    cc -shared -o libspdk_fat.so -Wl,--whole-archive *.a -Wl,--no-whole-archive

The result is a suboptimal solution because the shared library must be built and installed to the system prior to building the app and deployed to a target system along with the app. Detecting if libspdk fat lib is installed on the system is cumbersome as we can’t use pkg-config. spdk in general is missing support for pkg-config ( https://trello.com/c/uBM2PR4c/19-generate-pkg-config-pc-file-during-make-install ). In spite of all drawbacks it is kinda standard rust solution for sys crates and works with upstream spdk with no changes required.

## Using the spdk sys crate

High level steps of how to use spdk sys crate:

**Phase 1:**

1. Check out spdk-sys git repository including spdk sources as submodule.
2. Run a build script which automates steps needed to build spdk and creates the fat lib.
3. Install the fat lib to a system location (*as root)*.

**Phase 2:**

1. Use spdk-sys crate in dependencies in `Cargo.toml`.
2. When spdk-sys is built by rust, the build script merely checks that the fat shlib is installed on the system and generates bindings using bindgen and spdk header files for it (it does not build the library).

The way how the fat shlib is deployed along with the app to a target system is out of scope. In case of a docker image it can be copied from the system where the image is built to a docker image. On other systems it may be delivered in form of a package as it is usually done for other system libraries.

## spdk with patches

Some projects using spdk in rust will need to use their own version of spdk which differs from the upstream. There is probably no better way than to clone spdk-sys repository on github and override spdk git submodule in the repository so that it points to their own version of SPDK. In dependencies section of Cargo.toml file must be used a github URL of the cloned spdk-sys repo.

## Tests

The sys crate should come with tests. It is out of scope to test each function provided by SPDK. At minimum calling a single function from spdk would be sufficient. Later when support for opt-in modules is added, there should be a similar test for each module.

## Links
- Official doc for build.rs script: https://doc.rust-lang.org/cargo/reference/build-scripts.html
- Opinionated do’s and dont’s for creating sys crates: https://kornel.ski/rust-sys-crate
- libgit2-sys build script: https://github.com/rust-lang/git2-rs/blob/master/libgit2-sys/build.rs
