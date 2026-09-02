//go:generate go tool gqlgen generate

package main

import (
	"encoding/json"
	"math/rand"
	"context"
	"strings"
	"log/slog"
	"encoding/hex"
	"log"
	"database/sql"
	"net/http"
	"os"

	"github.com/fluidity-money/rfqhub.9lives.so/cmd/graph/gen"

	_ "github.com/lib/pq"

	"github.com/99designs/gqlgen/graphql/handler"
	"github.com/99designs/gqlgen/graphql/handler/extension"
	"github.com/99designs/gqlgen/graphql/handler/lru"
	"github.com/99designs/gqlgen/graphql/handler/transport"
	"github.com/99designs/gqlgen/graphql/playground"

	"github.com/vektah/gqlparser/v2/ast"

	ethCommon "github.com/ethereum/go-ethereum/common"
)

const (
	// EnvTimescaleUri to use as the database for private key loading
	// and authentication key loading.
	EnvTimescaleUri = "SPN_TIMESCALE"

	// EnvAccPrivateKey to execute transactions on the behalf of users with.
	EnvAccPrivateKey = "SPN_ACCOUNTS_PRIVATE_KEY"
)

type authMiddleware struct {
	db          *sql.DB
	srv         http.Handler
	adminSecret string
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
		a.srv.ServeHTTP(w, r)
	case a.adminSecret:
		adminSecretEnabled := a.adminSecret != ""
		a.srv.ServeHTTP(w, r.WithContext(context.WithValue(
			r.Context(),
			"is admin",
			adminSecretEnabled,
		)))
	default:
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
	if _, err := hex.DecodeString(os.Getenv(EnvAccPrivateKey)); err != nil {
		log.Fatalf("accounts private key needs to be set: %v", err)
	}
	db, err := sql.Open("postgres", os.Getenv(EnvTimescaleUri))
	if err != nil {
		panic(err)
	}
	defer db.Close()
	srv := handler.New(gen.NewExecutableSchema(gen.Config{
		Resolvers: &gen.Resolver{
			Db: db,
			// TODO add remainder here
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
	http.Handle("/", playground.Handler("GraphQL playground", "/query"))
	http.Handle("/query", srv)
	panic(http.ListenAndServe(os.Getenv("SPN_LISTEN_ADDR"), nil))
}

func writeUnauthorised(w http.ResponseWriter) {
	_ = json.NewEncoder(w).Encode(struct {
		Error string `json:"error"`
	}{"unauthorised"})
}
