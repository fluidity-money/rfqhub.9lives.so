#!/bin/sh

url=${ENDPOINT:-https://testnet-rpc.superposition.so}

if [ -z "$PRIVATE_KEY" ]; then
	>&2 echo "PRIVATE_KEY unset"
	exit 2
fi

cargo stylus deploy \
	--wasm-file "rfqhub_9lives.wasm" \
	--private-key "$PRIVATE_KEY" \
	--endpoint "$url" \
	--no-verify \
	        | sed -nr 's/.*deployed code at address: +.*(0x.{40}).*$/\1/p'
