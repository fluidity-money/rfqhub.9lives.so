#!/bin/sh -e

cargo test --features testing

make rfqhub.9lives.so.wasm

arbos-forge test --ffi
