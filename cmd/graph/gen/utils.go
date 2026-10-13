// Copyright 2026 Fluid Assets Limited. All rights reserved. Use of this
// source code is governed by a BUSL license that can be found in the
// LICENSE file.

package gen

import (
	"bytes"
	"database/sql"
	"encoding/binary"
	"encoding/json"
	"log/slog"
	"math/big"
	"net/http"
	"strings"
	"time"

	superposition_assets "github.com/fluidity-money/superposition-assets"

	"github.com/fluidity-money/rfqhub.9lives.so/cmd/graph/gen/model"
	"github.com/fluidity-money/rfqhub.9lives.so/lib/types"
)

const ProgDalek = "/bin/rfqhub-ed25519-dalek-ph"

func isDryrun(x *bool) bool {
	if x != nil && *x {
		return true
	} else {
		return false
	}
}

func activateSoftAlarm(url string, snowflake int, err error) {
	if url == "" {
		slog.Info("not alarming soft alarm (it's empty)")
		return
	}
	var buf bytes.Buffer
	_ = json.NewEncoder(&buf).Encode(struct {
		Snowflake int    `json:"snowflake"`
		Error     string `json:"error"`
	}{snowflake, err.Error()})
	r, _ := http.Post(url, "application/json", &buf)
	r.Body.Close()
}

func trackTx(db *sql.DB, eoaS, txHash string, gasLimit uint64, desc string) {
	_, err := db.Exec(`
INSERT INTO accounts_executed_transactions_2 (
	eoa_addr,
	transaction_hash,
	gas_limit,
	desc_
)
VALUES ($1, $2, $3, $4)`,
		eoaS,
		txHash,
		gasLimit,
		desc,
	)
	if err != nil {
		slog.Error("error tracking executed transactions", "err", err)
		// We'll ignore this and not propagate up to the user this error.
	}
}

func isIgnorable(err error) bool {
	es := err.Error()
	switch {
	case strings.Contains(es, "res: 99090e"),
		strings.Contains(es, "ERC20: transfer amount exceeds balance"),
		strings.Contains(es, "res: 990995"):
		return true
	default:
		return false
	}
}

func pickMsTs() (b [6]byte) {
	u := new(big.Int).SetInt64(time.Now().Unix())
	copy(b[:], u.Bytes())
	return
}

func msTsBytesToInt(b [6]byte) uint64 {
	var x [8]byte
	copy(x[2:], b[:])
	v, _ := binary.Uvarint(x[:])
	return v
}

func msTsIntToBytes(x uint64) (b [6]byte) {
	if x > 0xffffffffffff {
		panic("timestamp exceeds 48 bits")
	}
	var buf [8]byte
	binary.PutUvarint(buf[:], x)
	copy(b[:], buf[2:])
	return
}

func modelOutcomeToOutcome(x model.Outcome) types.Outcome {
	switch x {
	case "BitcoinPrice":
		return types.OutcomeBitcoinPrice
	}
	panic("bad outcome")
}

func usdcAddr() string {
	return strings.ToLower(superposition_assets.AssetUsdc.Address())
}
