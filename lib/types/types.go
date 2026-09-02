package types

import (
	acc_types "github.com/fluidity-money/accounts.superposition.so/lib/types"

	"github.com/near/borsh-go"
)

type Sig [64]byte

type Asset = acc_types.Asset

type Outcome uint8

const (
	OutcomeBitcoinPrice Outcome = 0
)

type FillerArgs struct {
	FusdcOnWin uint64   `borsh:"fusdc_on_win"`
	AccSender  [20]byte `borsh:"acc_sender"`
}

type OnchainPotArgs struct {
	AccAddr [20]byte `borsh:"acc_addr"`
	Amt     uint64   `borsh:"amt"`
	Asset   Asset    `borsh:"asset"`
}

const (
	OnrampingSourceOnchainPot borsh.Enum = iota
	OnrampingSourceAggregatedResult
)

type OnrampingSource struct {
	Enum             borsh.Enum `borsh_enum:"true"`
	OnchainPot       OnchainPotArgs
	AggregatedResult *AggregateEnum
}

type BundleTakerArgs struct {
	Src         OnrampingSource
	Expiry      uint32
	MsTs        [6]byte
	IsUp        bool
	PriceTarget uint64
	Outcome     Outcome
}

type BundleTaker struct {
	Args BundleTakerArgs
	Sig  Sig
}

type BundleMakerArgs struct {
	Amt   uint64
	Taker BundleTaker
	MsTs  [6]byte
	Acc   [20]byte
}

type BundleMaker struct {
	Args BundleMakerArgs
	Sig  Sig
}

type OfframpArgs struct {
	Asset   Asset
	Amt     uint64
	MsTs    [6]byte
	AccAddr [20]byte
}

type Timestamplet struct {
	TimestampletTime    uint32
	TimestampletOutcome Outcome
	TimestampletVal     uint64
}

type AggregateArgs struct {
	Maker        BundleMaker
	Timestamplet Timestamplet
}

type Aggregate struct {
	Args      AggregateArgs
	ServerSig Sig
}

const (
	AggregateEnumArgs borsh.Enum = iota
	AggregateEnumHash
)

type AggregateEnum struct {
	Enum borsh.Enum `borsh_enum:"true"`
	Args Aggregate
	Hash [32]byte
}

const (
	ArgsVersion borsh.Enum = iota
	ArgsSource
	ArgsBalance
	ArgsOnrampingFromBal
	ArgsOfframp
	ArgsAggregateToOfframp
)

type BalanceArgs struct {
	Asset [20]byte
	Addr  [20]byte
}

type OnrampingFromBalArgs struct {
	Asset Asset
	Amt   uint64
}

type OfframpVariantArgs struct {
	Args      OfframpArgs
	ServerSig Sig
}

type AggregateToOfframpArgs struct {
	Args    AggregateEnum
	IsTaker bool
}

type Args struct {
	Enum               borsh.Enum `borsh_enum:"true"`
	Version            struct{}
	Source             struct{}
	Balance            BalanceArgs
	OnrampingFromBal   OnrampingFromBalArgs
	Offramp            OfframpVariantArgs
	AggregateToOfframp AggregateToOfframpArgs
}

const (
	ClientSigningArgsBundleTaker borsh.Enum = iota
	ClientSigningArgsBundleMaker
)

type ClientSigningArgs struct {
	Enum        borsh.Enum `borsh_enum:"true"`
	BundleTaker BundleTakerArgs
	BundleMaker BundleMakerArgs
}
