
.DELETE_ON_ERROR:

rfqhub.9lives.so.wasm: $(shell find Cargo.* src -type f)
	@cargo build --release --target wasm32-unknown-unknown
	@./wasm-post.sh \
		target/wasm32-unknown-unknown/release/contract.wasm \
		rfqhub.9lives.so.wasm
	@./check-codesize.sh rfqhub.9lives.so.wasm
