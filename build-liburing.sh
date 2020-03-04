#!/bin/bash
pkg_name=liburing-0.5

curl -L https://git.kernel.dk/cgit/liburing/snapshot/"$pkg_name".tar.gz | tar xz
pushd "$pkg_name"
./configure
make -j4
sudo make install

popd
