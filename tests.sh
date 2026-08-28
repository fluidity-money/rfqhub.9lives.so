#!/bin/sh -e

cargo test --features testing

export SPN_DEPLOY_TARGET=arbos

make rfqhub.9lives.so.wasm

arbos-forge test --ffi
