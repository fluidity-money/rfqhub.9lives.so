package main

import (
	"runtime/debug"
	"strings"
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
	chanConcluded := make(chan bool)
	for {
		time.Sleep(time.Second)
		// TODO: this assumes every outcome is the Bitcoin price!
		rows, err := db.Query(`
SELECT bundle_taker_id FROM rfqhub_takers_to_conclude_1`,
		)
		if err != nil {
			log.Fatalf("query conclude takers: %v", err)
		}
		defer rows.Close()
		var count int
		for count = 0; rows.Next(); count++ {
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
				_, err = db.Exec(`
SELECT rfqhub_conclude_aggregate_1($1, $2)`,
					bundleTakerId,
					timestampletVal,
				)
				if err != nil {
					log.Fatalf("conclude taker market execute: %v", err)
				}
				chanConcluded <- true
			}(bundleTakerId)
		}
		if count == 0 {
			slog.Debug("No takers that need resolution")
			continue
		}
		i := 0
		for range chanConcluded {
			i++
			if count == i {
				break
			}
		}
	}
}

func init() {
	logLevel := slog.LevelInfo
	if os.Getenv("SPN_DEBUG") != "" {
		logLevel = slog.LevelDebug
	}
	logger := slog.New(slog.NewJSONHandler(os.Stderr, &slog.HandlerOptions{
		Level: logLevel,
	}))
	var revision string
	if info, ok := debug.ReadBuildInfo(); ok {
		for _, setting := range info.Settings {
			if setting.Key == "vcs.revision" {
				revision = setting.Value
				break
			}
		}
	}
	logger.
		With("revision", revision).
		With("environment", "backend").
		With("command line", strings.Join(os.Args, ",")).
		With("is debug", logLevel == slog.LevelDebug)
	slog.SetDefault(logger)
}
