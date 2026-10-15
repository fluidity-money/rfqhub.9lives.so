package main

import (
	"context"
	"database/sql"
	"log"
	"log/slog"
	"math/big"
	"os"
	"time"

	"github.com/fluidity-money/rfqhub.9lives.so/lib/chainlink_proxy"
	"github.com/fluidity-money/rfqhub.9lives.so/lib/types"

	_ "github.com/lib/pq"

	"github.com/ethereum/go-ethereum/ethclient"
)

func main() {
	client, err := ethclient.Dial(os.Getenv("SPN_SUPERPOSITION_URL"))
	if err != nil {
		log.Fatalf("client open: %v", err)
	}
	defer client.Close()
	db, err := sql.Open("postgres", os.Getenv("SPN_TIMESCALE"))
	if err != nil {
		log.Fatalf("open database: %v", err)
	}
	// TODO: this assumes every outcome is the Bitcoin price!
	rows, err := db.Query(`
SELECT bundle_taker_id FROM rfqhub_takers_to_conclude_1`,
	)
	if err != nil {
		log.Fatalf("query conclude takers: %v", err)
	}
	defer rows.Close()
	for rows.Next() {
		var bundleTakerId int
		if err := rows.Scan(&bundleTakerId); err != nil {
			log.Fatalf("scan bundle taker id: %v", err)
		}
		go func(bundleTakerId int) {
			var expiry time.Time
			roundId, err := chainlink_proxy.GetOutcomePrice(
				context.Background(),
				client,
				types.OutcomeBitcoinPrice,
				expiry,
			)
			if err != nil {
				log.Fatalf("get outcome price: %v", err)
			}
			slog.Info("Resolving market for expiry",
				"round id", roundId,
				"outcome", types.OutcomeBitcoinPrice,
				"expiry", expiry,
			)
			// TODO: hardcoded for Bitcoin to get us to 1e6 as the price delta:
			exp := new(big.Int).SetInt64(10)
			exp.Exp(exp, new(big.Int).SetInt64(12), nil)
			answer := roundId.Answer
			answer.Div(answer, exp)
			if !answer.IsUint64() {
				log.Fatalf("answer too large from chainlink: %v", err)
			}
			timestampletVal := answer.Uint64()
			row := db.QueryRow(`
SELECT
	bundle_maker_id,
	taker_was_correct,
	taker_amt,
	maker_amt,
	structure
FROM rfqhub_conclude_aggregate_1($1, $2)`,
				bundleTakerId,
				timestampletVal,
			)
			var (
				bundleMakerId      int
				takerWon           bool
				takerAmt, makerAmt uint64
				makerStructure     types.BundleMaker
			)
			err = row.Scan(
				&bundleMakerId,
				&takerWon,
				&takerAmt,
				&makerAmt,
				&makerStructure,
			)
			if err != nil {
				log.Fatalf("conclude blob scan: %v", err)
			}
		}(bundleTakerId)
	}
}
