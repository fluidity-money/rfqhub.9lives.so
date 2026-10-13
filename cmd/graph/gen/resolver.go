// Copyright 2026 Fluid Assets Limited. All rights reserved. Use of this
// source code is governed by a BUSL license that can be found in the
// LICENSE file.

package gen

import (
	"database/sql"
	"math/big"

	ethCommon "github.com/ethereum/go-ethereum/common"
	"github.com/ethereum/go-ethereum/ethclient"
)

type Resolver struct {
	Db                              *sql.DB
	AccountsFactoryAddr, RfqhubAddr ethCommon.Address
	ChainId                         *big.Int
	MinimumAmount                   uint64
	AccPubKey                       [32]byte
	Client                          *ethclient.Client
	UrlAlarm                        string
}
