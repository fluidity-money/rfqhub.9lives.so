
# rfqhub.9lives.so

Rfqhub has its implementation deployed to `0x03b206334e7b402cfde8f48c58e5913d3d15b43e` on
Arbitrum One mainnet. It's proxy lives at `0x7193088e878F3b461e552b3D6c0c97F1051A7822`.

rfqhub is a calldata aggregating middleware bundler and off-chain matching engine. Orders
are signed with a blob for the price of the option expiry and the asset name. The order is
matched by a counterparty, who can submit the calldata, with the incentive for submitting
the calldata being on whoever wins.

Why a new contract instead of extending the contract in Superposition Passport? This is
supposed to have a simpler internal architecture and a field for the expiry deadline.

## Dependencies

1. (https://github.com/OffchainLabs/cargo-stylus)[`cargo-stylus-sdk`] -- Cargo Stylus
binary for deployment.

2. (https://github.com/iosiro/arbos-foundry)[`arbos-foundry`] -- Needed for testing.

3. Rust with wasm32-unknown-unknown.

## Building

	make

## Testing

	./tests.sh
