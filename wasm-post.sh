#!/bin/sh -eu

# We make a pass similar to the way the node operates (it does a
# wasm2wat->wat2wasm pass). We also do this for arbos-foundry.

f="$(mktemp)"

wasm-opt \
	--dce \
	--rse \
	--signature-pruning \
	--strip-debug \
	--enable-bulk-memory \
	--strip \
	-Oz \
	"$1" \
	-o "$f.wasm1"

wasm2wat -o $f.wat $f.wasm1

wat2wasm -o $2 $f.wat
