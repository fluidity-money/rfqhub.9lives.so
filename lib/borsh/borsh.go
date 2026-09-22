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

func Encode(args types.Args) (buf []byte, err error) {
	cmd := exec.Command(ProgBorshPath)
	var bufIn, bufOut bytes.Buffer
	if err := json.NewEncoder(&bufIn).Encode(args); err != nil {
		return buf, fmt.Errorf("encoding: %v", args)
	}
	cmd.Stdin = &bufIn
	cmd.Stdout = &bufOut
	if err := cmd.Run(); err != nil {
		return buf, fmt.Errorf("running cmd: %v", err)
	}
	b := bytes.TrimPrefix(bytes.TrimSuffix(bufOut.Bytes(), []byte{'\n'}), []byte{'0', 'x'})
	_, err = hex.Decode(buf, b)
	if err != nil {
		return buf, fmt.Errorf("decode sig: %v", err)
	}
	return
}
