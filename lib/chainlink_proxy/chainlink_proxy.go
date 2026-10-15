// Copyright 2026 Fluid Assets Limited. All rights reserved. Use of this
// source code is governed by a BUSL license that can be found in the
// LICENSE file.

package chainlink_proxy

import (
	"bytes"
	"context"
	_ "embed"
	"encoding/binary"
	"fmt"
	"math/big"
	"time"

	"github.com/fluidity-money/rfqhub.9lives.so/lib/types"

	"github.com/ethereum/go-ethereum"
	ethAbi "github.com/ethereum/go-ethereum/accounts/abi"
	ethCommon "github.com/ethereum/go-ethereum/common"
	"github.com/ethereum/go-ethereum/ethclient"
)

//go:embed abi.json
var abiB []byte

var abi, abiErr = ethAbi.JSON(bytes.NewReader(abiB))

var AddrChainlinkProxy = ethCommon.HexToAddress("0xE69bCD13ac4C2dE1882B10339fe3e5075AE27C46")

type Round struct {
	RoundId           *big.Int `abi:"roundId"`
	Phase             uint16   `abi:"phase"`
	AggregatorRoundId uint64   `abi:"aggregatorRoundId"`
	Answer            *big.Int `abi:"answer"`
	StartedAt         *big.Int `abi:"startedAt"`
	UpdatedAt         *big.Int `abi:"updatedAt"`
}

// GetOutcomePrice closest to the time given, according to Chainlink.
func GetOutcomePrice(ctx context.Context, c *ethclient.Client, o types.Outcome, t time.Time) (*Round, error) {
	b := make([]byte, 4+32+32)
	copy(b[:4], []byte{0x69, 0x5a, 0x94, 0x36})
	chainlinkAddr := o.ChainlinkOracleAddrArb()
	copy(b[4+32-20:4+32], chainlinkAddr[:])
	binary.BigEndian.PutUint64(b[4+32*2-8:4+32*2], uint64(t.Unix()))
	rd, err := c.CallContract(ctx, ethereum.CallMsg{
		To:   &AddrChainlinkProxy,
		Data: b,
	}, nil)
	if err != nil {
		return nil, fmt.Errorf("get outcome price: %v", err)
	}
	var r struct {
		Round Round
	}
	if err := abi.UnpackIntoInterface(&r, "priceAt", rd); err != nil {
		return nil, fmt.Errorf("outcome price unpack: %v", err)
	}
	return &r.Round, nil
}
