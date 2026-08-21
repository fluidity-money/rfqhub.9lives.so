package gen

import (
	"database/sql"
	"math/big"

	ethCommon "github.com/ethereum/go-ethereum/common"
)

type Resolver struct {
	Db                              *sql.DB
	AccountsFactoryAddr, RfqhubAddr ethCommon.Address
	MinimumAmount                   *big.Int
	AccPubKey                       [32]byte
}
