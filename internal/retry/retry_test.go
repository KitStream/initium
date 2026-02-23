package retry

import (
	"context"
	"errors"
	"testing"
	"time"
)

func TestDefaultConfigValid(t *testing.T) {
	cfg := DefaultConfig()
	if err := cfg.Validate(); err != nil {
		t.Fatalf("default config should be valid: %v", err)
	}
}

func TestConfigValidation(t *testing.T) {
	tests := []struct {
		name    string
		cfg     Config
		wantErr bool
	}{
		{"valid", DefaultConfig(), false},
		{"zero attempts", Config{MaxAttempts: 0, InitialDelay: time.Second, MaxDelay: time.Second, BackoffFactor: 1.0}, true},
		{"negative delay", Config{MaxAttempts: 1, InitialDelay: -1, MaxDelay: time.Second, BackoffFactor: 1.0}, true},
		{"max < initial", Config{MaxAttempts: 1, InitialDelay: 2 * time.Second, MaxDelay: time.Second, BackoffFactor: 1.0}, true},
		{"backoff < 1", Config{MaxAttempts: 1, InitialDelay: time.Second, MaxDelay: time.Second, BackoffFactor: 0.5}, true},
		{"jitter negative", Config{MaxAttempts: 1, InitialDelay: time.Second, MaxDelay: time.Second, BackoffFactor: 1.0, JitterFraction: -0.1}, true},
		{"jitter > 1", Config{MaxAttempts: 1, InitialDelay: time.Second, MaxDelay: time.Second, BackoffFactor: 1.0, JitterFraction: 1.5}, true},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			err := tt.cfg.Validate()
			if tt.wantErr && err == nil {
				t.Fatal("expected error")
			}
			if !tt.wantErr && err != nil {
				t.Fatalf("unexpected error: %v", err)
			}
		})
	}
}

func TestDelayExponentialBackoff(t *testing.T) {
	cfg := Config{
		MaxAttempts:    10,
		InitialDelay:   100 * time.Millisecond,
		MaxDelay:       10 * time.Second,
		BackoffFactor:  2.0,
		JitterFraction: 0,
	}

	d0 := Delay(cfg, 0)
	d1 := Delay(cfg, 1)
	d2 := Delay(cfg, 2)

	if d0 != 100*time.Millisecond {
		t.Fatalf("expected 100ms, got %s", d0)
	}
	if d1 != 200*time.Millisecond {
		t.Fatalf("expected 200ms, got %s", d1)
	}
	if d2 != 400*time.Millisecond {
		t.Fatalf("expected 400ms, got %s", d2)
	}
}

func TestDelayCapped(t *testing.T) {
	cfg := Config{
		MaxAttempts:    10,
		InitialDelay:   time.Second,
		MaxDelay:       5 * time.Second,
		BackoffFactor:  10.0,
		JitterFraction: 0,
	}

	d := Delay(cfg, 5)
	if d > 5*time.Second {
		t.Fatalf("delay %s exceeds max %s", d, 5*time.Second)
	}
}

func TestDelayWithJitter(t *testing.T) {
	cfg := Config{
		MaxAttempts:    10,
		InitialDelay:   time.Second,
		MaxDelay:       30 * time.Second,
		BackoffFactor:  2.0,
		JitterFraction: 0.5,
	}

	d := Delay(cfg, 0)
	if d < time.Second {
		t.Fatalf("delay with jitter should be >= base: got %s", d)
	}
	if d > time.Second+500*time.Millisecond {
		t.Fatalf("delay with 0.5 jitter should be <= 1.5s: got %s", d)
	}
}

func TestDoSuccess(t *testing.T) {
	cfg := Config{
		MaxAttempts:    5,
		InitialDelay:   time.Millisecond,
		MaxDelay:       10 * time.Millisecond,
		BackoffFactor:  1.0,
		JitterFraction: 0,
	}

	calls := 0
	result := Do(context.Background(), cfg, func(_ context.Context, _ int) error {
		calls++
		return nil
	})

	if result.Err != nil {
		t.Fatalf("expected success, got: %v", result.Err)
	}
	if calls != 1 {
		t.Fatalf("expected 1 call, got %d", calls)
	}
}

func TestDoRetryThenSuccess(t *testing.T) {
	cfg := Config{
		MaxAttempts:    5,
		InitialDelay:   time.Millisecond,
		MaxDelay:       10 * time.Millisecond,
		BackoffFactor:  1.0,
		JitterFraction: 0,
	}

	calls := 0
	result := Do(context.Background(), cfg, func(_ context.Context, _ int) error {
		calls++
		if calls < 3 {
			return errors.New("not ready")
		}
		return nil
	})

	if result.Err != nil {
		t.Fatalf("expected success, got: %v", result.Err)
	}
	if calls != 3 {
		t.Fatalf("expected 3 calls, got %d", calls)
	}
	if result.Attempt != 2 {
		t.Fatalf("expected attempt 2, got %d", result.Attempt)
	}
}

func TestDoAllFail(t *testing.T) {
	cfg := Config{
		MaxAttempts:    3,
		InitialDelay:   time.Millisecond,
		MaxDelay:       10 * time.Millisecond,
		BackoffFactor:  1.0,
		JitterFraction: 0,
	}

	calls := 0
	result := Do(context.Background(), cfg, func(_ context.Context, _ int) error {
		calls++
		return errors.New("fail")
	})

	if result.Err == nil {
		t.Fatal("expected error")
	}
	if calls != 3 {
		t.Fatalf("expected 3 calls, got %d", calls)
	}
}

func TestDoContextCancelled(t *testing.T) {
	cfg := Config{
		MaxAttempts:    100,
		InitialDelay:   time.Second,
		MaxDelay:       time.Second,
		BackoffFactor:  1.0,
		JitterFraction: 0,
	}

	ctx, cancel := context.WithCancel(context.Background())
	cancel()

	result := Do(ctx, cfg, func(_ context.Context, _ int) error {
		return errors.New("fail")
	})

	if result.Err == nil {
		t.Fatal("expected context error")
	}
}
