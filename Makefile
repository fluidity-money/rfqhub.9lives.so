
include config.mk

.DELETE_ON_ERROR:

.PHONY: build wasm cli

build: wasm cli

wasm: rfqhub.9lives.so.wasm

OUT_WASM := target/wasm32-unknown-unknown/release/contract.wasm

OUT_CLI_SERVER_SIGNING := target/release/cli-server-signing
OUT_CLI_CLIENT_SIGNING := target/release/cli-client-signing
OUT_CLI_BORSH := target/release/cli-borsh

${OUT_WASM}: $(shell find Cargo.* superposition_librfq contract -type f)
	@cargo build \
		--package contract \
		--release \
		--target wasm32-unknown-unknown \
		--features ${SPN_DEPLOY_TARGET}

rfqhub.9lives.so.wasm: ${OUT_WASM}
	@./wasm-post.sh ${OUT_WASM} rfqhub.9lives.so.wasm
	@./check-codesize.sh rfqhub.9lives.so.wasm

${OUT_CLI_SERVER_SIGNING}: $(shell find Cargo.* superposition_librfq cli-server-signing cli-borsh cli-client-signing -type f)
	@cargo build --release

cli: rfqhub-cli-server-signing rfqhub-cli-client-signing rfqhub-cli-borsh

rfqhub-cli-server-signing: ${OUT_CLI_SERVER_SIGNING}
	@cp ${OUT_CLI_SERVER_SIGNING} rfqhub-cli-server-signing

rfqhub-cli-client-signing: ${OUT_CLI_SERVER_SIGNING}
	@cp ${OUT_CLI_CLIENT_SIGNING} rfqhub-cli-client-signing

rfqhub-cli-borsh: ${OUT_CLI_SERVER_SIGNING}
	@cp ${OUT_CLI_BORSH} rfqhub-cli-borsh

rfqhub-graph: $(shell find cmd/graph go.mod go.sum -type f)
	@cd cmd/graph && go build -o ../../rfqhub-graph

clean:
	@cargo clean
	@rm -f ${OUT_CLI} ${OUT_WASM} rfqhub-graph
