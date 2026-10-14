// Copyright 2026 Fluid Assets Limited. All rights reserved. Use of this
// source code is governed by a BUSL license that can be found in the
// LICENSE file.

package main

import (
	"context"
	"database/sql"
	"encoding/json"
	"fmt"
	"log"
	"log/slog"
	"net"
	"net/http"
	"net/url"
	"os"
	"strconv"
	"strings"
	"time"

	types "github.com/fluidity-money/rfqhub.9lives.so/lib/types"
	"github.com/fluidity-money/rfqhub.9lives.so/lib/websocket"

	cdc "github.com/Trendyol/go-pq-cdc"
	cdcConfig "github.com/Trendyol/go-pq-cdc/config"
	cdcFormat "github.com/Trendyol/go-pq-cdc/pq/message/format"
	cdcPublication "github.com/Trendyol/go-pq-cdc/pq/publication"
	cdcReplication "github.com/Trendyol/go-pq-cdc/pq/replication"
	cdcSlot "github.com/Trendyol/go-pq-cdc/pq/slot"
	_ "github.com/lib/pq"
)

const HttpUnixSocket = "/run/rfqhub.9lives.so/websocket.sock"

const (
	EnvTimescaleUri          = "SPN_TIMESCALE"
	EnvPublicationName       = "SPN_PUBLICATION_NAME"
	EnvPublicationSlot       = "SPN_PUBLICATION_SLOT"
	EnvPublicationMetricSlot = "SPN_PUBLICATION_PORT"
)

type Response struct {
	BundleTaker *BundleTaker `json:"BundleTaker,omitempty"`
	BundleMaker *BundleMaker `json:"BundleMaker,omitempty"`
}

type BundleMaker struct {
	AccAddr       string `json:"acc_addr"`
	Amt           int64  `json:"amt"`
	BundleTakerID int32  `json:"bundle_taker_id"`
}

func bundleMakerFromUpdate(row map[string]any) (*BundleMaker, bool, error) {
	if structure, set := row["structure"]; !set || structure == nil {
		return nil, false, nil
	}
	data, err := json.Marshal(map[string]any{
		"acc_addr":        row["acc_addr"],
		"amt":             row["amt"],
		"bundle_taker_id": row["bundle_taker_id"],
	})
	if err != nil {
		return nil, false, fmt.Errorf("encode bundle maker update: %w", err)
	}
	var bundle BundleMaker
	if err := json.Unmarshal(data, &bundle); err != nil {
		return nil, false, fmt.Errorf("decode bundle maker update: %w", err)
	}
	return &bundle, true, nil
}

type BundleTaker struct {
	ID          int32             `json:"id"`
	AccAddr     string            `json:"acc_addr"`
	Asset       string            `json:"asset"`
	Amt         int64             `json:"amt"`
	MinOffer    int64             `json:"min_offer"`
	IsUp        bool              `json:"is_up"`
	PriceTarget int64             `json:"price_target"`
	Outcome     string            `json:"outcome"`
	Deadline    time.Time         `json:"deadline"`
	Expiry      time.Time         `json:"expiry"`
	Structure   types.BundleTaker `json:"structure"`
}

func bundleTakerFromUpdate(row map[string]any) (*BundleTaker, bool, error) {
	rawStructure, set := row["structure"]
	if !set || rawStructure == nil {
		return nil, false, nil
	}
	var structure types.BundleTaker
	switch value := rawStructure.(type) {
	case []byte:
		if err := json.Unmarshal(value, &structure); err != nil {
			return nil, false, fmt.Errorf("decode structure: %w", err)
		}
	case string:
		if err := json.Unmarshal([]byte(value), &structure); err != nil {
			return nil, false, fmt.Errorf("decode structure: %w", err)
		}
	default:
		data, err := json.Marshal(value)
		if err != nil {
			return nil, false, fmt.Errorf("encode structure: %w", err)
		}
		if err := json.Unmarshal(data, &structure); err != nil {
			return nil, false, fmt.Errorf("decode structure: %w", err)
		}
	}
	normalised := make(map[string]any, len(row))
	for key, value := range row {
		normalised[key] = value
	}
	normalised["structure"] = structure
	data, err := json.Marshal(normalised)
	if err != nil {
		return nil, false, fmt.Errorf("encode bundle taker update: %w", err)
	}

	var bundle BundleTaker
	if err := json.Unmarshal(data, &bundle); err != nil {
		return nil, false, fmt.Errorf("decode bundle taker update: %w", err)
	}
	return &bundle, true, nil
}

// One statement gives both lists the same database snapshot. Select public
// fields explicitly, particularly for makers whose structure must stay private.
const activeBundlesQuery = `
WITH active_takers AS MATERIALIZED (
	SELECT * FROM rfqhub_bundles_taker_ongoing_1
)
SELECT json_build_object(
	'BundleTakers', COALESCE((
		SELECT json_agg(json_build_object(
			'id', t.id,
			'acc_addr', t.acc_addr,
			'asset', t.asset,
			'amt', t.amt,
			'min_offer', t.min_offer,
			'is_up', t.is_up,
			'price_target', t.price_target,
			'outcome', t.outcome,
			'deadline', to_char(t.deadline, 'YYYY-MM-DD"T"HH24:MI:SS.US"Z"'),
			'expiry', to_char(t.expiry, 'YYYY-MM-DD"T"HH24:MI:SS.US"Z"'),
			'structure', t.structure
		) ORDER BY t.id)
		FROM active_takers t
	), '[]'::json),
	'BundleMakers', COALESCE((
		SELECT json_agg(json_build_object(
			'acc_addr', m.acc_addr,
			'amt', m.amt,
			'bundle_taker_id', m.bundle_taker_id
		) ORDER BY m.bundle_taker_id, m.id)
		FROM rfqhub_bundles_maker_1 m
		JOIN active_takers t ON t.id = m.bundle_taker_id
		WHERE m.structure IS NOT NULL
	), '[]'::json)
)`

func main() {
	db, err := sql.Open("postgres", os.Getenv(EnvTimescaleUri))
	if err != nil {
		log.Fatalf("open snapshot database: %v", err)
	}
	defer db.Close()
	db.SetMaxOpenConns(10)
	db.SetMaxIdleConns(5)

	u, err := url.Parse(os.Getenv(EnvTimescaleUri))
	if err != nil {
		log.Fatalf("parse error: %v", err)
	}
	if u.User == nil {
		log.Fatal("timescale url has no username")
	}
	timescalePassword, _ := u.User.Password()
	var port int
	if p := u.Port(); p != "" {
		if port, err = strconv.Atoi(strings.TrimPrefix(u.Port(), ":")); err != nil {
			log.Fatalf("bad port: %v", u.Port())
		}
	} else {
		port = 5432
	}
	metricPort, err := strconv.Atoi(os.Getenv(EnvPublicationMetricSlot))
	if err != nil {
		log.Fatalf("metric slot: %v", err)
	}
	cfg := cdcConfig.Config{
		Host:     u.Hostname(),
		Username: u.User.Username(),
		Password: timescalePassword,
		Database: strings.TrimPrefix(u.Path, "/"),
		Port:     port,
		ExtensionSupport: cdcConfig.ExtensionSupport{
			EnableTimeScaleDB: true,
		},
		Publication: cdcPublication.Config{
			CreateIfNotExists: true,
			Name:              os.Getenv(EnvPublicationName),
			Operations: cdcPublication.Operations{
				cdcPublication.OperationUpdate,
			},
			Tables: []cdcPublication.Table{
				{
					Name:            "rfqhub_bundles_taker_1",
					ReplicaIdentity: cdcPublication.ReplicaIdentityFull,
				},
				{
					Name:            "rfqhub_bundles_maker_1",
					ReplicaIdentity: cdcPublication.ReplicaIdentityFull,
				},
			},
		},
		Metric: cdcConfig.MetricConfig{
			Port: metricPort,
		},
		Slot: cdcSlot.Config{
			Name:                        os.Getenv(EnvPublicationSlot),
			CreateIfNotExists:           true,
			SlotActivityCheckerInterval: 1000,
		},
	}
	masterCtx, masterCancel := context.WithCancel(context.Background())
	defer masterCancel()
	broadcast := websocket.NewBroadcast[Response]()
	cdcConn, err := cdc.NewConnector(masterCtx, cfg, func(ctx *cdcReplication.ListenerContext) {
		if err := ctx.Ack(); err != nil {
			log.Fatalf("ack: %v", err)
		}
		msg, isUpdate := ctx.Message.(*cdcFormat.Update)
		if !isUpdate {
			slog.Debug("non update received",
				"type", fmt.Sprintf("%T", ctx.Message),
			)
			return
		}
		switch msg.TableName {
		case "rfqhub_bundles_taker_1":
			bundle, structureSet, err := bundleTakerFromUpdate(msg.NewDecoded)
			if err != nil {
				slog.Error("failed to decode bundle taker update", "err", err)
				return
			}
			if structureSet {
				broadcast.Broadcast(Response{BundleTaker: bundle})
			}
		case "rfqhub_bundles_maker_1":
			bundle, structureSet, err := bundleMakerFromUpdate(msg.NewDecoded)
			if err != nil {
				slog.Error("failed to decode bundle maker update", "err", err)
				return
			}
			if structureSet {
				broadcast.Broadcast(Response{BundleMaker: bundle})
			}
		}
	})
	if err != nil {
		log.Fatalf("error opening cdc connector: %v", err)
	}
	go func() {
		websocket.Endpoint("/", func(
			connCtx context.Context,
			ipAddr string,
			query url.Values,
			header http.Header,
			replies <-chan []byte,
			outgoing chan<- []byte,
			shutdown chan<- error,
			requestShutdown <-chan bool,
		) {
			sink := make(chan Response, 5*60)
			cookie := broadcast.Subscribe(sink)
			defer broadcast.Unsubscribe(cookie)

			// Subscribe before querying so live updates can queue while the
			// snapshot loads. Clients may see a row again in the live feed.
			snapshotCtx, snapshotCancel := context.WithTimeout(connCtx, 10*time.Second)
			var snapshot []byte
			err := db.QueryRowContext(snapshotCtx, activeBundlesQuery).Scan(&snapshot)
			snapshotCancel()
			if err != nil {
				slog.Error("failed to load active bundles", "err", err, "ip", ipAddr)
				return
			}
			select {
			case outgoing <- snapshot:
			case <-connCtx.Done():
				return
			case <-masterCtx.Done():
				return
			}

		L:
			for {
				select {
				case <-connCtx.Done():
					slog.Debug("connection context cancelled",
						"ip", ipAddr,
					)
					break L
				case <-masterCtx.Done():
					slog.Debug("master context cancelled, tearing down connection",
						"ip", ipAddr,
					)
					break L
				case m, ok := <-sink:
					if !ok {
						break L
					}
					payload, err := json.Marshal(m)
					if err != nil {
						slog.Error("failed to encode websocket update", "err", err)
						continue
					}
					select {
					case outgoing <- payload:
					case <-connCtx.Done():
						break L
					case <-masterCtx.Done():
						break L
					}
				case _, ok := <-replies:
					if !ok {
						break L
					}
				case done, ok := <-requestShutdown:
					if !ok || done {
						slog.Debug("shutdown requested by framework",
							"ip", ipAddr,
						)
						break L
					}
				}
			}
			select {
			case shutdown <- nil:
			default:
			}
		})
		_ = os.Remove(HttpUnixSocket)
		l, err := net.Listen("unix", HttpUnixSocket)
		if err != nil {
			log.Fatal(err)
		}
		defer l.Close()
		log.Fatal(http.Serve(l, nil))
	}()
	defer cdcConn.Close()
	cdcConn.Start(masterCtx)
}
