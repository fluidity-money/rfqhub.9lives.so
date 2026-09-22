package types

import (
	"database/sql/driver"
	"encoding/json"
	"fmt"

	acc_types "github.com/fluidity-money/accounts.superposition.so/lib/types"

	"github.com/near/borsh-go"

	ethCommon "github.com/ethereum/go-ethereum/common"
)

type Sig [64]byte

type Asset acc_types.Asset

const (
	AssetUsdc Asset = Asset(acc_types.AssetUsdc)
	AssetArb  Asset = Asset(acc_types.AssetArb)
	AssetWeth Asset = Asset(acc_types.AssetWeth)
)

func (a Asset) MarshalJSON() ([]byte, error) {
	switch a {
	case AssetUsdc:
		return json.Marshal("USDC")
	case AssetArb:
		return json.Marshal("ARB")
	case AssetWeth:
		return json.Marshal("WETH")
	default:
		return nil, fmt.Errorf("unknown asset %d", a)
	}
}

func (a *Asset) UnmarshalJSON(data []byte) error {
	var s string
	if err := json.Unmarshal(data, &s); err != nil {
		return err
	}

	switch s {
	case "USDC":
		*a = AssetUsdc
	case "ARB":
		*a = AssetArb
	case "WETH":
		*a = AssetWeth
	default:
		return fmt.Errorf("unknown asset %q", s)
	}

	return nil
}

type Outcome uint8

const OutcomeBitcoinPrice Outcome = 0

func (o Outcome) MarshalJSON() ([]byte, error) {
	switch o {
	case OutcomeBitcoinPrice:
		return json.Marshal("BitcoinPrice")
	default:
		return nil, fmt.Errorf("unknown outcome %d", o)
	}
}

func (o *Outcome) UnmarshalJSON(data []byte) error {
	var s string
	if err := json.Unmarshal(data, &s); err != nil {
		return err
	}
	switch s {
	case "BitcoinPrice":
		*o = OutcomeBitcoinPrice
	default:
		return fmt.Errorf("unknown outcome %q", s)
	}
	return nil
}

// ChainlinkOracleAddrArb gets the Chainlink price feed for BTC USD (intended for Arbitrum One).
func (o Outcome) ChainlinkOracleAddrArb() ethCommon.Address {
	switch o {
	case OutcomeBitcoinPrice:
		return ethCommon.HexToAddress("0x6ce185860a4963106506C203335A2910413708e9")
	default:
		panic("bad outcome")
	}
}

type FillerArgs struct {
	FusdcOnWin uint64   `borsh:"fusdc_on_win"`
	AccSender  [20]byte `borsh:"acc_sender"`
}

type OnchainPotArgs struct {
	AccAddr [20]byte `json:"acc_addr"`
	Amt     uint64   `json:"amt"`
	Asset   Asset    `json:"asset"`
}

type AggregatedResultArgs struct {
	From      AggregateEnum `json:"from"`
	AmtToTake uint64        `json:"amt_to_take"`
}

const (
	OnrampingSourceOnchainPot borsh.Enum = iota
	OnrampingSourceAggregatedResult
)

type OnrampingSource struct {
	Enum             borsh.Enum            `borsh_enum:"true" json:"-"`
	OnchainPot       *OnchainPotArgs       `json:"OnchainPot,omitempty"`
	AggregatedResult *AggregatedResultArgs `json:"AggregatedResult,omitempty"`
}

type BundleTakerArgs struct {
	Src         OnrampingSource `json:"src"`
	Expiry      uint32          `json:"expiry"`
	MsTs        [6]byte         `json:"ms_ts"`
	IsUp        bool            `json:"is_up"`
	PriceTarget uint64          `json:"price_target"`
	Outcome     Outcome         `json:"outcome"`
}

type BundleTaker struct {
	Args BundleTakerArgs `json:"args"`
	Sig  Sig             `json:"sig"`
}

type BundleMakerArgs struct {
	Src   OnrampingSource `json:"src"`
	Taker BundleTaker     `json:"taker"`
	MsTs  [6]byte         `json:"ms_ts"`
	Acc   [20]byte        `json:"acc"`
}

type BundleMaker struct {
	Args BundleMakerArgs `json:"args"`
	Sig  Sig             `json:"sig"`
}

type OfframpArgs struct {
	Asset   Asset    `json:"asset"`
	Amt     uint64   `json:"amt"`
	MsTs    [6]byte  `json:"ms_ts"`
	AccAddr [20]byte `json:"acc_addr"`
}

type Timestamplet struct {
	TimestampletTime    uint32  `json:"timestamplet_time"`
	TimestampletOutcome Outcome `json:"timestamplet_outcome"`
	TimestampletVal     uint64  `json:"timestamplet_val"`
}

type AggregateArgs struct {
	Maker        BundleMaker  `json:"maker"`
	Timestamplet Timestamplet `json:"timestamplet"`
}

type Aggregate struct {
	Args      AggregateArgs `json:"args"`
	ServerSig Sig           `json:"server_sig"`
}

const (
	AggregateEnumArgs borsh.Enum = iota
	AggregateEnumHash
)

type AggregateEnum struct {
	Enum borsh.Enum `borsh_enum:"true" json:"-"`
	Args *Aggregate `json:"args,omitempty"`
	Hash *[32]byte  `json:"hash,omitempty"`
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
	Asset [20]byte `json:"asset"`
	Addr  [20]byte `json:"addr"`
}

type OnrampingFromBalArgs struct {
	Asset Asset  `json:"asset"`
	Amt   uint64 `json:"amt"`
}

type OfframpVariantArgs struct {
	Args      OfframpArgs `json:"args"`
	ServerSig Sig         `json:"server_sig"`
}

type AggregateToOfframpArgs struct {
	Args    AggregateEnum `json:"args"`
	IsTaker bool          `json:"is_taker"`
}

type Args struct {
	Enum               borsh.Enum              `borsh_enum:"true" json:"-"`
	Version            *struct{}               `json:"version,omitempty"`
	Source             *struct{}               `json:"source,omitempty"`
	Balance            *BalanceArgs            `json:"balance,omitempty"`
	OnrampingFromBal   *OnrampingFromBalArgs   `json:"onramping_from_bal,omitempty"`
	Offramp            *OfframpVariantArgs     `json:"offramp,omitempty"`
	AggregateToOfframp *AggregateToOfframpArgs `json:"aggregate_to_offramp,omitempty"`
}

const (
	ClientSigningArgsBundleTaker borsh.Enum = iota
	ClientSigningArgsBundleMaker
)

type ClientSigningArgs struct {
	Enum        borsh.Enum       `borsh_enum:"true" json:"-"`
	BundleTaker *BundleTakerArgs `json:"BundleTaker,omitempty"`
	BundleMaker *BundleMakerArgs `json:"BundleMaker,omitempty"`
}

const (
	ServerSigningArgsBundleOfframp borsh.Enum = iota
	ServerSigningArgsAggregateToOfframp
)

type ServerSigningArgs struct {
	Enum        borsh.Enum       `borsh_enum:"true" json:"-"`
	Offramp *OfframpArgs `json:"OfframpArgs,omitempty"`
	AggregateToOfframp *AggregateArgs `json:"AggregateToOfframp,omitempty"`
}

func (b BundleTaker) Value() (driver.Value, error) {
	return json.Marshal(b)
}

func (b *BundleTaker) Scan(src any) error {
	switch src := src.(type) {
	case []byte:
		return json.Unmarshal(src, b)
	case string:
		return json.Unmarshal([]byte(src), b)
	case nil:
		return nil
	default:
		return fmt.Errorf("cannot scan %T into BundleTaker", src)
	}
}

func (b BundleMaker) Value() (driver.Value, error) {
	return json.Marshal(b)
}

func (b *BundleMaker) Scan(src any) error {
	switch src := src.(type) {
	case []byte:
		return json.Unmarshal(src, b)
	case string:
		return json.Unmarshal([]byte(src), b)
	case nil:
		return nil
	default:
		return fmt.Errorf("cannot scan %T into BundleMaker", src)
	}
}
