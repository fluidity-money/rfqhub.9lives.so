// Copyright 2026 Fluid Assets Limited. All rights reserved. Use of this
// source code is governed by a BUSL license that can be found in the
// LICENSE file.

package main

import (
	"log"
	"strconv"
	"net/http"
	"context"
	"os"
	"log/slog"
	"fmt"
	"strings"
	"net/url"

	"github.com/fluidity-money/rfqhub.9lives.so/lib/websocket"

	cdc "github.com/Trendyol/go-pq-cdc"
	cdcConfig "github.com/Trendyol/go-pq-cdc/config"
	cdcFormat "github.com/Trendyol/go-pq-cdc/pq/message/format"
	cdcPublication "github.com/Trendyol/go-pq-cdc/pq/publication"
	cdcReplication "github.com/Trendyol/go-pq-cdc/pq/replication"
	cdcSlot "github.com/Trendyol/go-pq-cdc/pq/slot"
)

const (
	EnvTimescaleUri = "SPN_TIMESCALE"
	EnvPublicationName       = "SPN_PUBLICATION_NAME"
	EnvPublicationSlot       = "SPN_PUBLICATION_SLOT"
	EnvPublicationMetricSlot = "SPN_PUBLICATION_PORT"
)

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
				cdcPublication.OperationInsert,
			},
			Tables: []cdcPublication.Table{{
				Name: "rfqhub_bundles_maker_1",
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
	broadcast := websocket.NewBroadcast[TableContent]()
	cdcConn, err := cdc.NewConnector(masterCtx, cfg, func(ctx *cdcReplication.ListenerContext) {
		msg, isInsert := ctx.Message.(*cdcFormat.Insert)
		if err := ctx.Ack(); err != nil {
			log.Fatalf("ack: %v", err)
		}
		if !isInsert {
			slog.Debug("non insert received",
				"msg", msg,
				"type", fmt.Sprintf("%T", msg),
			)
			return
		}
		broadcast.Broadcast(msg)
	})
	if err != nil {
		log.Fatalf("error opening cdc connector: %v", err)
	}
	go func() {
		websocket.Endpoint("/", func(
			connCtx context.Context,
			ipAddr string,
			query url.Values,
			replies <-chan []byte,
			outgoing chan<- []byte,
			shutdown chan<- error,
			requestShutdown <-chan bool,
		) {
			sink := make(chan TableContent, 5*60)
			cookie := broadcast.Subscribe(sink)
			defer broadcast.Unsubscribe(cookie)

			filterRules := make(map[string]map[string]*FilterConstraint)
			filterRulesSet := make(map[string]bool)

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
				case msg, ok := <-replies:
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
		log.Fatalf("bind: %v", http.ListenAndServe(
			os.Getenv(EnvListenAddr),
			nil,
		))
	}()
	defer connector.Close()
	connector.Start(masterCtx)
}
