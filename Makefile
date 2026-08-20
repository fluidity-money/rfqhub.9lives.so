
include config.mk

.DELETE_ON_ERROR:

.PHONY: build wasm cli

build: wasm cli

wasm: rfqhub.9lives.so.wasm

OUT_WASM := target/wasm32-unknown-unknown/release/contract.wasm

OUT_CLI := target/release/cli

${OUT_WASM}: $(shell find Cargo.* superposition_librfq contract -type f)
	@cargo build --release --target wasm32-unknown-unknown

rfqhub.9lives.so.wasm: ${OUT_WASM}
	@./wasm-post.sh ${OUT_WASM} rfqhub.9lives.so.wasm
	@./check-codesize.sh rfqhub.9lives.so.wasm

${OUT_CLI}: $(shell find Cargo.* superposition_librfq cli -type f)
	@cargo build --release

cli: rfqhub-cli

rfqhub-cli: ${OUT_CLI}
	@cp ${OUT_CLI} rfqhub-cli

clean:
	@rm -rf target ${OUT_CLI} ${OUT_WASM}