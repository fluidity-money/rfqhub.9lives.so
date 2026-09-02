package signer

import (
	"os/exec"
	"bytes"
	"encoding/json"
	"fmt"
	"encoding/hex"

	"github.com/fluidity-money/rfqhub.9lives.so/lib/types"
)

const (
	// ProgClientSigningPath to invoke to get client signing performed
	ProgClientSigningPath = "/usr/local/bin/rfqhub-cli-client-signing"

	// ProgServerSigningPath to invoke to get server signing performed
	ProgServerSigningPath = "/usr/local/bin/rfqhub-cli-client-signing"
)

func ServerSignAggregate(key [32]byte, args types.Args) (sig [64]byte, err error) {
	cmd := exec.Command(ProgServerSigningPath)
	cmd.Env = append(cmd.Env, fmt.Sprintf("SPN_SUPERPOSITION_KEY=%x", key))
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

func ClientSignClientArgs(key [32]byte, args types.ClientSigningArgs) (sig [64]byte, err error) {
	cmd := exec.Command(ProgServerSigningPath)
	cmd.Env = append(cmd.Env, fmt.Sprintf("SPN_SUPERPOSITION_KEY=%x", key))
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
