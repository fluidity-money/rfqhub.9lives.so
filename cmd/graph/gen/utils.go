package gen

import (
	"bytes"
	"database/sql"
	"encoding/json"
	"log/slog"
	"net/http"
	"strings"
)

const ProgDalek = "/usr/local/bin/ed25519-dalek-ph"

func isDryrun(x *bool) bool {
	if x != nil && *x {
		return true
	} else {
		return false
	}
}

func activateSoftAlarm(url string, snowflake int, err error) {
	if url == "" {
		slog.Info("not alarming soft alarm")
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
