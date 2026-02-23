package main

import (
	"context"

	"github.com/kitstream/initium/internal/logging"
)

type loggerKey struct{}

// withLogger returns a new context.Context that carries a logger.
func withLogger(ctx context.Context, log *logging.Logger) context.Context {
	return context.WithValue(ctx, loggerKey{}, log)
}
