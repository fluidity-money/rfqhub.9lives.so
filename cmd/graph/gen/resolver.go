package gen

import (
	"database/sql"

	ethCommon "github.com/ethereum/go-ethereum/common"
)

type Resolver struct{
	Db *sql.DB
	AccountsFactoryAddr ethCommon.Address
}
