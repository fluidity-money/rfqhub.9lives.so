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
}
