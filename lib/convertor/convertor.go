package convertor

import (
	"crypto/sha512"
	"encoding/binary"
	"encoding/hex"
	"fmt"
	"math"
	"os/exec"
	"strings"

	"github.com/fluidity-money/rfqhub.9lives.so/cmd/graph/gen/model"
	"github.com/fluidity-money/rfqhub.9lives.so/lib/types"

	acc_types "github.com/fluidity-money/accounts.superposition.so/lib/types"

	ethCommon "github.com/ethereum/go-ethereum/common"

	"github.com/near/borsh-go"
)

var MaxBytes32 [32]byte

func strToAddr(s string) ([20]byte, error) {
	var b [20]byte
	if s == "" {
		return b, nil
	}
	i, err := hex.Decode(b[:], []byte(strings.TrimPrefix(s, "0x")))
	if err != nil {
		return b, fmt.Errorf("decode str: %v", err)
	}
	if i != 20 {
		return b, fmt.Errorf("decode str len: %v", i)
	}
	return b, nil
}

func strToBytes32(s string) ([32]byte, error) {
	var b [32]byte
	i, err := hex.Decode(b[:], []byte(strings.TrimPrefix(s, "0x")))
	if err != nil {
		return b, fmt.Errorf("decode str: %v", err)
	}
	if i != 32 {
		return b, fmt.Errorf("decode str len: %v", i)
	}
	return b, nil
}

func CreateFreshBackwards(
	pubKey [32]byte,
	createAccount model.CreateAccount,
) (*acc_types.FreshBackwards, error) {
	eoa, err := strToAddr(createAccount.EoaAddr)
	if err != nil {
		return nil, fmt.Errorf("eoa: %v", err)
	}
	var authority *acc_types.ArgsAuthorityAddr
	if a := createAccount.Authority; a != nil {
		x, err := strToAddr(*a)
		if err != nil {
			return nil, fmt.Errorf("authority: %v", err)
		}
		v := acc_types.ArgsAuthorityAddr(x)
		authority = &v
	}
	if createAccount.SigV < 0 || createAccount.SigV > math.MaxUint8 {
		return nil, fmt.Errorf("v exceeds")
	}
	v := uint8(createAccount.SigV)
	r, err := strToBytes32(createAccount.SigR)
	if err != nil {
		return nil, fmt.Errorf("r: %v", err)
	}
	s, err := strToBytes32(createAccount.SigS)
	if err != nil {
		return nil, fmt.Errorf("s: %v", err)
	}
	return &acc_types.FreshBackwards{
		Key:       pubKey,
		EoaAddr:   eoa,
		V:         v,
		R:         r,
		S:         s,
		Authority: authority,
	}, nil
}

// CreateSolveArgsSigArgs for minting by also creating 9lives calldata.
func CreateSolveArgsSigArgs(
	prog string,
	rfqhubAddr ethCommon.Address,
	asset types.Asset,
	amt uint64,
	permit *model.Permit,
	msTs [6]byte,
) (*acc_types.SolveArgsSigArgs, error) {
	cd, err := borsh.Serialize(types.Args{
		Enum: types.ArgsOnrampingFromBal,
		OnrampingFromBal: types.OnrampingFromBalArgs{
			Asset: asset,
			Amt:   amt,
		},
	})
	if err != nil {
		return nil, fmt.Errorf("serialise borsh: %v", err)
	}
	var toTake [32]byte
	if _, err := binary.Encode(toTake[32-8:], binary.BigEndian, amt); err != nil {
		return nil, fmt.Errorf("encoding toTake: %v", err)
	}
	var msTsI [16]byte
	copy(msTsI[:16-len(msTs)], msTs[:])
	solveArgs := acc_types.SolveArgs{
		From: []acc_types.FromArgs{{
			Asset:  asset,
			ToTake: toTake,
			//MaxUnspent: [32]byte{},
		}},
		Target: rfqhubAddr,
		Cd:     cd,
		MsTs:   msTsI,
	}
	if permit != nil {
		if permit.Deadline < 0 {
			return nil, fmt.Errorf("negative deadline")
		}
		d := uint64(permit.Deadline)
		p, err := NewPermit(
			asset,
			d,
			permit.PermitV,
			permit.PermitR,
			permit.PermitS,
		)
		if err != nil {
			return nil, fmt.Errorf("permit: %v", err)
		}
		solveArgs.Permit = append(solveArgs.Permit, *p)
	}
	solveArgsDigest, err := borsh.Serialize(solveArgs)
	if err != nil {
		return nil, fmt.Errorf("making digest: %v", err)
	}
	var sigArr [64]byte
	d := sha512.Sum512(solveArgsDigest)
	sig, err := exec.Command(
		prog,
		hex.EncodeToString(d[:]),
	).
		Output()
	if err != nil {
		return nil, fmt.Errorf("invoking dalekph: %v, %v", string(sig), err)
	}
	copy(sigArr[:], sig)
	return &acc_types.SolveArgsSigArgs{
		Sig:  sigArr,
		Args: solveArgs,
	}, nil
}

func TagFreshBackwardsWithOnrampingFromBal(
	prog string,
	rfqhubAddr ethCommon.Address,
	f *acc_types.FreshBackwards,
	asset types.Asset,
	amt uint64,
	permit *model.Permit,
	msTs [6]byte,
) error {
	m, err := CreateSolveArgsSigArgs(
		prog,
		rfqhubAddr,
		asset,
		amt,
		permit,
		msTs,
	)
	if err != nil {
		return err
	}
	f.SolveArgs = append(f.SolveArgs, *m)
	return nil
}

func NewPermit(
	asset types.Asset,
	deadline uint64,
	permitV int,
	permitR, permitS string,
) (*acc_types.Permit, error) {
	if permitV < 0 || permitV > math.MaxUint8 {
		return nil, fmt.Errorf("v exceeds")
	}
	v := uint8(permitV)
	r, err := strToBytes32(permitR)
	if err != nil {
		return nil, fmt.Errorf("permit r: %v", err)
	}
	s, err := strToBytes32(permitS)
	if err != nil {
		return nil, fmt.Errorf("permit s: %v", err)
	}
	return &acc_types.Permit{
		Asset:    asset,
		Deadline: deadline,
		V:        v,
		R:        r,
		S:        s,
	}, nil
}

func init() {
	for i := 0; i < 32; i++ {
		MaxBytes32[i] = math.MaxUint8
	}
}
