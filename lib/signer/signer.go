// Copyright 2026 Fluid Assets Limited. All rights reserved. Use of this
// source code is governed by a BUSL license that can be found in the
// LICENSE file.

package signer

import (
	"bytes"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"log/slog"
	"os/exec"

	"github.com/fluidity-money/rfqhub.9lives.so/lib/types"
)

const (
	// ProgClientSigningPath to invoke to get client signing performed
	ProgClientSigningPath = "/bin/rfqhub-cli-client-signing"

	// ProgServerSigningPath to invoke to get server signing performed
	ProgServerSigningPath = "/bin/rfqhub-cli-server-signing"
)

func ServerSignAggregate(args types.ServerSigningArgs) (sig [64]byte, err error) {
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
	_, err = hex.Decode(sig[:], bytes.TrimSuffix(bufOut.Bytes(), []byte{'\n'}))
	if err != nil {
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
	input := bufIn.Bytes()
	cmd.Stdin = &bufIn
	cmd.Stdout = &bufOut
	if err := cmd.Run(); err != nil {
		slog.Debug("Error running signing client arguments stdin",
			"stdin", input,
		)
		return sig, fmt.Errorf("running cmd: %v", err)
	}
	_, err = hex.Decode(sig[:], bytes.TrimSuffix(bufOut.Bytes(), []byte{'\n'}))
	if err != nil {
		return sig, fmt.Errorf("decode sig: %v", err)
	}
	return
}
