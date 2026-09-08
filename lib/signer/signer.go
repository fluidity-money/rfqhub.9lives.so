package signer

import (
	"bytes"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"os/exec"

	"github.com/fluidity-money/rfqhub.9lives.so/lib/types"
)

const (
	// ProgClientSigningPath to invoke to get client signing performed
	ProgClientSigningPath = "/bin/rfqhub-cli-client-signing"

	// ProgServerSigningPath to invoke to get server signing performed
	ProgServerSigningPath = "/bin/rfqhub-cli-client-signing"
)

func ServerSignAggregate(args types.Args) (sig [64]byte, err error) {
	cmd := exec.Command(ProgServerSigningPath)
	var bufIn, bufOut bytes.Buffer
	if err := json.NewEncoder(&bufIn).Encode(args); err != nil {
		return sig, fmt.Errorf("encoding: %v", args)
	}
	cmd.Stdin = &bufIn
	cmd.Stdout = &bufOut
	if err := cmd.Run(); err != nil {
		return sig, fmt.Errorf("running cmd: %v", err)
	}
	if _, err := hex.Decode(sig[:], bufOut.Bytes()); err != nil {
		return sig, fmt.Errorf("decode sig: %v", err)
	}
	return
}

func ClientSignClientArgs(args types.ClientSigningArgs) (sig [64]byte, err error) {
	cmd := exec.Command(ProgClientSigningPath)
	var bufIn, bufOut bytes.Buffer
	if err := json.NewEncoder(&bufIn).Encode(args); err != nil {
		return sig, fmt.Errorf("encoding: %v", args)
	}
	cmd.Stdin = &bufIn
	cmd.Stdout = &bufOut
	if err := cmd.Run(); err != nil {
		return sig, fmt.Errorf("running cmd: %v", err)
	}
	if _, err := hex.Decode(sig[:], bufOut.Bytes()); err != nil {
		return sig, fmt.Errorf("decode sig: %v", err)
	}
	return
}
