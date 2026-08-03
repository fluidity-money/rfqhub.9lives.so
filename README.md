
# rfqhub.9lives.so

rfqhub is a calldata aggregating middleware bundler and off-chain matching engine. Orders
are signed with a blob for the price of the option expiry and the asset name. The order is
matched by a counterparty, who can submit the calldata, with the incentive for submitting
the calldata being on whoever wins.

It differs from Orderbookkit and Superposition Passport in that the bundles explicitly
have the time of the option's exercise embedded in the option. The onramping into the
contract is like Superposition Passport's, in that it functions like a validium, needing
to be onramped into using a special type that can only be offramped out of using a
centralised server's blessing.

The blobs to aggregate trades are actually signed blobs from Superposition Passport's
"Statement" feature, which can be verified using the passport engine itself.

The UTXO-like on-chain state looks like the following. The UTXOs cannot be manipulated
with subtraction or addition in the rollup like Passport:

```ocaml
type state_onramping
type state_bundled
type state_offramped

type address
type amt = uint64
type time_creation = int
type time_expiry = int

(* The address of the passport contract that's used to verify signatures. *)
type owner_passport = address

type _ contract =
  | Onramping : owner_passport * amt -> state_onramping contract
  | Bundled : time_creation * time_expiry -> state_bundled contract
  | Resolved : state_bundled contract * outcome -> state_offramped contract
```

Why a new contract instead of extending the contract in Superposition Passport? Signature
verification happens inside Passport instead of here using a custom method, and the
resolution of the contracts depend on an interaction with Chainlink. It's also a bit
simpler, Passport supports multiple different places where the calldata could be sent.
Rfqhub is a single submission of the calldata, with only offramping taking place after the
calldata has been aggregated in the aggregated stage.

## Dependencies

1. (https://github.com/OffchainLabs/cargo-stylus)[`cargo-stylus-sdk`] -- Cargo Stylus
binary for deployment.

2. (https://github.com/iosiro/arbos-foundry)[`arbos-foundry`] -- Needed for testing.

3. Rust with wasm32-unknown-unknown.

## Building

	make

## Testing

	./tests.sh
