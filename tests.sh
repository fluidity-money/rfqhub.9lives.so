#!/bin/sh -e

cargo test --features testing

arbos-forge test --ffi
