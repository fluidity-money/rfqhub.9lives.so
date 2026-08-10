//go:generate go tool gqlgen generate

package main

import (
	"database/sql"
	"net/http"
	"os"

	_ "github.com/lib/pq"

	"github.com/99designs/gqlgen/graphql/handler"
	"github.com/99designs/gqlgen/graphql/handler/extension"
	"github.com/99designs/gqlgen/graphql/handler/lru"
	"github.com/99designs/gqlgen/graphql/handler/transport"
	"github.com/99designs/gqlgen/graphql/playground"

	"github.com/vektah/gqlparser/v2/ast"

	"github.com/fluidity-money/rfqhub.9lives.so/cmd/graph/gen"
)

func main() {
	db, err := sql.Open("postgres", os.Getenv("SPN_TIMESCALE_URL"))
	if err != nil {
		panic(err)
	}
	defer db.Close()
	srv := handler.New(gen.NewExecutableSchema(gen.Config{
		Resolvers: &gen.Resolver{db},
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
