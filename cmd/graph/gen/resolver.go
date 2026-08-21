package gen

import (
	"database/sql"
	"math/big"

	"github.com/ethereum/go-ethereum/ethclient"
	ethCommon "github.com/ethereum/go-ethereum/common"
)

type Resolver struct {
	Db                              *sql.DB
	AccountsFactoryAddr, RfqhubAddr ethCommon.Address
	MinimumAmount, ChainId                   *big.Int
	AccPubKey                       [32]byte
	Client *ethclient.Client
}
