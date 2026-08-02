
rfqhub-9lives.wasm: $(shell find Cargo.* src -type f)
	@rm -f rfqhub_9lives.wasm
	@cargo build --release --target wasm32-unknown-unknown
	@./wasm-post.sh \
		target/wasm32-unknown-unknown/release/rfqhub-9lives.wasm \
		rfqhub_9lives.wasm
	@./check-codesize.sh rfqhub-9lives.wasm
