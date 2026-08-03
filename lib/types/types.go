package types

import "github.com/near/borsh-go"

type Version struct{}

type Source struct{}

type Balance struct {
	Addr [20]byte
}

type Onramping struct {
	OwnerPassport [20]byte
	Amt           uint64
}

const (
	ArgsVersion borsh.Enum = iota
	ArgsSource
	ArgsBalance
	ArgsOnramping
)

type Args struct {
	Enum      borsh.Enum `borsh_enum:"true" json:"enum"`
	Version   Version    `json:"version"`
	Source    Source     `json:"source"`
	Balance   Balance    `json:"balance"`
	Onramping Onramping  `json:"onramping"`
}
