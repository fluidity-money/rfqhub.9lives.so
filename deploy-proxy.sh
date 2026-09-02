#!/bin/sh -eu

cat >/dev/null <<EOF
$SPN_ADMIN_ADDR
$SPN_SUPERPOSITION_KEY
$SPN_SUPERPOSITION_URL
EOF

set +u

if [ -z "$SPN_RFQHUB_IMPL" ]; then
	export SPN_RFQHUB_IMPL="$(./deploy-impl.sh)"
fi

echo SPN_RFQHUB_IMPL=$SPN_RFQHUB_IMPL 2>&1

forge create \
	--json \
	--rpc-url "$SPN_SUPERPOSITION_URL" \
	--broadcast \
	--private-key "$SPN_SUPERPOSITION_KEY" \
	solidity/RfqhubProxy.sol:RfqhubProxy \
	--constructor-args "$SPN_RFQHUB_IMPL" "$SPN_ADMIN_ADDR" \
		| jq -r .deployedTo
