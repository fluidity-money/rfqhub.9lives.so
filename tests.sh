#!/bin/sh -e

# This is left here for you to comment out if you have functions you
# want to test in Rust on the native host:
#cargo test --features std

make

arbos-forge test $@
