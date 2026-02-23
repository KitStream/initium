package retry

import (
	"context"
	"fmt"
	"math"
	"math/rand/v2"
	"time"
)

type Config struct {
	MaxAttempts    int
	InitialDelay   time.Duration
	MaxDelay       time.Duration
	BackoffFactor  float64
	JitterFraction float64 // 0.0–1.0: fraction of delay to add as random jitter
}

func DefaultConfig() Config {
	return Config{
		MaxAttempts:    60,
		InitialDelay:   time.Second,
		MaxDelay:       30 * time.Second,
		BackoffFactor:  2.0,
		JitterFraction: 0.1,
	}
}

func (c Config) Validate() error {
	if c.MaxAttempts < 1 {
		return fmt.Errorf("max-attempts must be >= 1, got %d", c.MaxAttempts)
	}
	if c.InitialDelay <= 0 {
		return fmt.Errorf("initial-delay must be > 0, got %s", c.InitialDelay)
	}
	if c.MaxDelay < c.InitialDelay {
		return fmt.Errorf("max-delay (%s) must be >= initial-delay (%s)", c.MaxDelay, c.InitialDelay)
	}
	if c.BackoffFactor < 1.0 {
		return fmt.Errorf("backoff-factor must be >= 1.0, got %f", c.BackoffFactor)
	}
	if c.JitterFraction < 0 || c.JitterFraction > 1 {
		return fmt.Errorf("jitter-fraction must be in [0, 1], got %f", c.JitterFraction)
	}
	return nil
}

func Delay(cfg Config, attempt int) time.Duration {
	delay := float64(cfg.InitialDelay) * math.Pow(cfg.BackoffFactor, float64(attempt))
	if delay > float64(cfg.MaxDelay) {
		delay = float64(cfg.MaxDelay)
	}

	if cfg.JitterFraction > 0 {
		jitter := delay * cfg.JitterFraction * rand.Float64()
		delay += jitter
	}

	return time.Duration(delay)
}

type Result struct {
	Attempt int
	Err     error
}

func Do(ctx context.Context, cfg Config, fn func(ctx context.Context, attempt int) error) Result {
	for attempt := range cfg.MaxAttempts {
		err := fn(ctx, attempt)
		if err == nil {
			return Result{Attempt: attempt, Err: nil}
		}

		if attempt == cfg.MaxAttempts-1 {
			return Result{Attempt: attempt, Err: fmt.Errorf("all %d attempts failed, last error: %w", cfg.MaxAttempts, err)}
		}

		delay := Delay(cfg, attempt)
		select {
		case <-ctx.Done():
			return Result{Attempt: attempt, Err: fmt.Errorf("context cancelled after attempt %d: %w", attempt+1, ctx.Err())}
		case <-time.After(delay):
		}
	}

	return Result{Err: fmt.Errorf("max attempts reached")}
}
