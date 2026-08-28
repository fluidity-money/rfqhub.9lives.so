#!/bin/sh -e

cargo test --features testing

export SPN_DEPLOY_TARGET=arbos

make -B rfqhub.9lives.so.wasm

arbos-forge test --stylus-debug --ffi -vvv
