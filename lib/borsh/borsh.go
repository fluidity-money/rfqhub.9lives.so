// Copyright 2026 Fluid Assets Limited. All rights reserved. Use of this
// source code is governed by a BUSL license that can be found in the
// LICENSE file.

package borsh

import (
	"bytes"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"os/exec"

	"github.com/fluidity-money/rfqhub.9lives.so/lib/types"
)

const ProgBorshPath = "/bin/rfqhub-cli-borsh"

func Encode(args types.Args) ([]byte, error) {
	cmd := exec.Command(ProgBorshPath)
	var bufIn, bufOut bytes.Buffer
	if err := json.NewEncoder(&bufIn).Encode(args); err != nil {
		return nil, fmt.Errorf("encoding: %v", args)
	}
	cmd.Stdin = &bufIn
	cmd.Stdout = &bufOut
	if err := cmd.Run(); err != nil {
		return nil, fmt.Errorf("running cmd: %v", err)
	}
	b := bytes.TrimPrefix(bytes.TrimSuffix(bufOut.Bytes(), []byte{'\n'}), []byte{'0', 'x'})
	var buf bytes.Buffer
	if _, err := buf.ReadFrom(hex.NewDecoder(bytes.NewBuffer(b))); err != nil {
		return nil, fmt.Errorf("decode hex: %v", err)
	}
	return buf.Bytes(), nil
}
