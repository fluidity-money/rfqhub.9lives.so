#!/bin/sh -e

cargo test --features std,proptest,arbitrary

export SPN_DEPLOY_TARGET=arbos

make rfqhub-cli

make -B rfqhub.9lives.so.wasm

arbos-forge test --stylus-debug --ffi -vvv
