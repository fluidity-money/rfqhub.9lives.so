package graph

import "database/sql"

type Resolver struct{
	Db *sql.DB
}
