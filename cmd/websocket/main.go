// Copyright 2026 Fluid Assets Limited. All rights reserved. Use of this
// source code is governed by a BUSL license that can be found in the
// LICENSE file.

package main

import (
	"context"
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
)

const HttpUnixSocket = "/run/rfqhub.9lives.so/websocket.sock"

const (
	EnvTimescaleUri          = "SPN_TIMESCALE"
	EnvPublicationName       = "SPN_PUBLICATION_NAME"
	EnvPublicationSlot       = "SPN_PUBLICATION_SLOT"
	EnvPublicationMetricSlot = "SPN_PUBLICATION_PORT"
)

type Response struct {
	BundleTaker *BundleTaker
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

func main() {
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
			Tables: []cdcPublication.Table{{
				Name:            "rfqhub_bundles_taker_1",
				ReplicaIdentity: cdcPublication.ReplicaIdentityFull,
			}},
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
		bundle, structureSet, err := bundleTakerFromUpdate(msg.NewDecoded)
		if err != nil {
			slog.Error("failed to decode bundle taker update", "err", err)
			return
		}
		if !structureSet {
			return
		}
		broadcast.Broadcast(Response{BundleTaker: bundle})
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
