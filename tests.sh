#!/bin/sh -e

cargo test --features testing,proptest,arbitrary

export SPN_DEPLOY_TARGET=arbos

make rfqhub-cli-borsh

make -B rfqhub.9lives.so.wasm

arbos-forge test --stylus-debug --ffi -vvv
