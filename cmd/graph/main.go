// Copyright 2026 Fluid Assets Limited. All rights reserved. Use of this
// source code is governed by a BUSL license that can be found in the
// LICENSE file.

//go:generate go tool gqlgen generate

package main

import (
	"context"
	"database/sql"
	"encoding/hex"
	"encoding/json"
	"log"
	"log/slog"
	"math/big"
	"math/rand"
	"net"
	"net/http"
	"os"
	"runtime/debug"
	"strings"

	"github.com/fluidity-money/rfqhub.9lives.so/cmd/graph/gen"

	_ "github.com/lib/pq"

	"github.com/99designs/gqlgen/graphql/handler"
	"github.com/99designs/gqlgen/graphql/handler/extension"
	"github.com/99designs/gqlgen/graphql/handler/lru"
	"github.com/99designs/gqlgen/graphql/handler/transport"
	"github.com/99designs/gqlgen/graphql/playground"

	"github.com/vektah/gqlparser/v2/ast"

	ethCommon "github.com/ethereum/go-ethereum/common"
	"github.com/ethereum/go-ethereum/ethclient"
)

// ChainId to correspond to Arbitrum.
var ChainId = new(big.Int).SetInt64(42161)

// HttpUnixSocket that we listen on using a socket.
const HttpUnixSocket = "/run/rfqhub.9lives.so/http.sock"

const (
	// EnvDebug to print logs.
	EnvDebug = "SPN_DEBUG"

	// EnvTimescaleUri to use as the database for private key loading
	// and authentication key loading.
	EnvTimescaleUri = "SPN_TIMESCALE"

	// EnvGethAddr to connect to make requests to Arbitrum One.
	EnvGethAddr = "SPN_SUPERPOSITION_URL"

	// EnvAccPrivateKey to execute transactions on the behalf of users with.
	EnvAccPrivateKey = "SPN_ACCOUNTS_PRIVATE_KEY"

	// EnvAccPublicKey, set since we feed the Rust code the private key, and
	// it expects a differently sized key, so we can't derive the same key reliably here.
	EnvAccPublicKey = "SPN_ACCOUNTS_PUBLIC_KEY"

	// EnvAccountsFactoryAddr to derive the addresses to send
	// transactions to when users ask to solve with mint, or to
	// create accounts with.
	EnvAccountsFactoryAddr = "SPN_ACCOUNTS_ADDR"

	// EnvRfqhubAddr to use as the location to send Rfqhub payloads
	// that we generate here in tandem with the accounts service.
	EnvRfqhubAddr = "SPN_RFQHUB_ADDR"
)

type authMiddleware struct {
	db  *sql.DB
	srv http.Handler
}

// ServeHTTP with a copy and paste from the accounts system
func (a authMiddleware) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	origin := r.Header.Get("Origin")
	if origin != "" {
		w.Header().Set("Access-Control-Allow-Origin", origin)
		w.Header().Set("Access-Control-Allow-Headers", "*")
		w.Header().Set("Access-Control-Allow-Methods", "*")
		w.Header().Set("Access-Control-Allow-Credentials", "true")
		w.Header().Set("Access-Control-Max-Age", "86400")
		w.Header().Set("Access-Control-Expose-Headers", "*")
	}
	if r.Method == "OPTIONS" {
		w.WriteHeader(204)
		return
	}
	snowflake := rand.Int()
	ctx := context.WithValue(r.Context(), "snowflake", snowflake)
	switch bearer := r.Header.Get("Authorization"); bearer {
	case "":
		slog.Debug("Serving a request without any Authorization",
			"snowflake", snowflake,
		)
		a.srv.ServeHTTP(w, r.WithContext(ctx))
	default:
		slog.Debug("Serving a request with a bearer",
			"snowflake", snowflake,
			"bearer", bearer,
		)
		bearerS := strings.Split(bearer, ":")
		eoaPreferred_ := bearerS[0]
		if !ethCommon.IsHexAddress(eoaPreferred_) {
			slog.Error("not eoa address",
				"eoa", eoaPreferred_,
				"snowflake", snowflake,
			)
			w.WriteHeader(http.StatusBadRequest)
			return
		}
		eoaPreferred := strings.ToLower(ethCommon.HexToAddress(eoaPreferred_).String())
		_, err := hex.DecodeString(bearerS[1])
		if err != nil {
			slog.Error("error decoding bearer",
				"err", err,
				"bearer", bearerS,
				"snowflake", snowflake,
			)
			w.WriteHeader(http.StatusBadRequest)
			return
		}
		secretX := strings.ToLower(bearerS[1])
		row := a.db.QueryRow(`
SELECT COUNT(1)
FROM accounts_secrets_2
WHERE eoa_addr = $1 AND secret = $2`,
			eoaPreferred,
			secretX,
		)
		var count int
		if err := row.Scan(&count); err != nil {
			w.WriteHeader(http.StatusUnauthorized)
			log.Fatalf("error scanning secrets, snowflake: %v: %v", "snowflake", err)
			writeUnauthorised(w)
			return
		}
		if count == 0 {
			w.WriteHeader(http.StatusUnauthorized)
			slog.Error("no rows found", "snowflake", snowflake)
			writeUnauthorised(w)
			return
		}
		eoa := ethCommon.HexToAddress(eoaPreferred)
		ctx = context.WithValue(context.WithValue(ctx, "authed", true), "eoa", eoa)
		a.srv.ServeHTTP(w, r.WithContext(ctx))
	}
}

func main() {
	var (
		accountsFactoryAddr = getAddr(EnvAccountsFactoryAddr)
		rfqhubAddr          = getAddr(EnvRfqhubAddr)
	)
	if _, err := hex.DecodeString(os.Getenv(EnvAccPrivateKey)); err != nil {
		log.Fatalf("accounts private key needs to be set: %v", err)
	}
	var publicKey [32]byte
	publicKey_, err := hex.DecodeString(os.Getenv(EnvAccPublicKey))
	if err != nil {
		log.Fatalf("accounts public key: %v", err)
	}
	if l := len(publicKey); l != 32 {
		log.Fatalf("public key size: %v", l)
	}
	copy(publicKey[:], publicKey_)
	client, err := ethclient.Dial(os.Getenv(EnvGethAddr))
	if err != nil {
		log.Fatalf("ethclient dial: %v", err)
	}
	defer client.Close()
	db, err := sql.Open("postgres", os.Getenv(EnvTimescaleUri))
	if err != nil {
		log.Fatalf("connect postgres: %v", err)
	}
	defer db.Close()
	srv := handler.New(gen.NewExecutableSchema(gen.Config{
		Resolvers: &gen.Resolver{
			Db:                  db,
			AccountsFactoryAddr: accountsFactoryAddr,
			RfqhubAddr:          rfqhubAddr,
			ChainId:             ChainId,
			MinimumAmount:       0,
			AccPubKey:           publicKey,
			Client:              client,
			UrlAlarm:            "",
		},
	}))
	srv.AddTransport(transport.Options{})
	srv.AddTransport(transport.GET{})
	srv.AddTransport(transport.POST{})
	srv.SetQueryCache(lru.New[*ast.QueryDocument](1000))
	srv.Use(extension.Introspection{})
	srv.Use(extension.AutomaticPersistedQuery{
		Cache: lru.New[string](100),
	})
	http.Handle("/playground", playground.Handler("GraphQL playground", "/query"))
	http.Handle("/", authMiddleware{db, srv})
	_ = os.Remove(HttpUnixSocket)
	l, err := net.Listen("unix", HttpUnixSocket)
	if err != nil {
		log.Fatal(err)
	}
	defer l.Close()
	panic(http.Serve(l, nil))
}

func writeUnauthorised(w http.ResponseWriter) {
	_ = json.NewEncoder(w).Encode(struct {
		Error string `json:"error"`
	}{"unauthorised"})
}

func getAddr(x string) ethCommon.Address {
	if v := os.Getenv(x); ethCommon.IsHexAddress(v) {
		return ethCommon.HexToAddress(v)
	} else {
		log.Fatalf("%v unset: %v", x, v)
		panic("unset")
	}
}

func init() {
	logLevel := slog.LevelInfo
	if os.Getenv(EnvDebug) != "" {
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
