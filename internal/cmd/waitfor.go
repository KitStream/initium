package cmd

import (
	"context"
	"crypto/tls"
	"fmt"
	"net"
	"net/http"
	"time"

	"github.com/kitstream/initium/internal/logging"
	"github.com/kitstream/initium/internal/retry"
	"github.com/spf13/cobra"
)

func NewWaitForCmd(log *logging.Logger) *cobra.Command {
	var (
		targets        []string
		timeout        time.Duration
		maxAttempts    int
		initialDelay   time.Duration
		maxDelay       time.Duration
		backoffFactor  float64
		jitterFraction float64
		httpStatus     int
		insecureTLS    bool
	)

	cmd := &cobra.Command{
		Use:   "wait-for",
		Short: "Wait for TCP or HTTP(S) endpoints to become available",
		Long: `Wait for one or more endpoints to become reachable before proceeding.
Supports TCP connectivity checks and HTTP(S) health checks with configurable
retries, exponential backoff, and jitter.

Targets use the format: tcp://host:port or http(s)://host:port/path`,
		Example: `  # Wait for Postgres
  initium wait-for --target tcp://postgres:5432

  # Wait for multiple services
  initium wait-for --target tcp://postgres:5432 --target http://api:8080/healthz

  # Wait for HTTPS endpoint allowing self-signed certs
  initium wait-for --target https://vault:8200/v1/sys/health --insecure-tls`,
		RunE: func(cmd *cobra.Command, args []string) error {
			if len(targets) == 0 {
				return fmt.Errorf("at least one --target is required")
			}

			cfg := retry.Config{
				MaxAttempts:    maxAttempts,
				InitialDelay:   initialDelay,
				MaxDelay:       maxDelay,
				BackoffFactor:  backoffFactor,
				JitterFraction: jitterFraction,
			}
			if err := cfg.Validate(); err != nil {
				return fmt.Errorf("invalid retry config: %w", err)
			}

			ctx, cancel := context.WithTimeout(cmd.Context(), timeout)
			defer cancel()

			for _, target := range targets {
				log.Info("waiting for target", "target", target)
				checker, err := newChecker(target, httpStatus, insecureTLS, timeout)
				if err != nil {
					return err
				}

				result := retry.Do(ctx, cfg, func(ctx context.Context, attempt int) error {
					log.Debug("attempt", "target", target, "attempt", fmt.Sprintf("%d", attempt+1))
					return checker(ctx)
				})

				if result.Err != nil {
					log.Error("target not reachable", "target", target, "error", result.Err.Error())
					return fmt.Errorf("target %s not reachable: %w", target, result.Err)
				}

				log.Info("target is reachable", "target", target, "attempts", fmt.Sprintf("%d", result.Attempt+1))
			}

			log.Info("all targets reachable")
			return nil
		},
	}

	cmd.Flags().StringArrayVar(&targets, "target", nil, "Target endpoint (tcp://host:port or http(s)://...)")
	cmd.Flags().DurationVar(&timeout, "timeout", 5*time.Minute, "Overall timeout for all targets")
	cmd.Flags().IntVar(&maxAttempts, "max-attempts", 60, "Maximum number of retry attempts per target")
	cmd.Flags().DurationVar(&initialDelay, "initial-delay", time.Second, "Initial delay between retries")
	cmd.Flags().DurationVar(&maxDelay, "max-delay", 30*time.Second, "Maximum delay between retries")
	cmd.Flags().Float64Var(&backoffFactor, "backoff-factor", 2.0, "Backoff multiplier")
	cmd.Flags().Float64Var(&jitterFraction, "jitter", 0.1, "Jitter fraction (0.0-1.0)")
	cmd.Flags().IntVar(&httpStatus, "http-status", 200, "Expected HTTP status code for HTTP(S) targets")
	cmd.Flags().BoolVar(&insecureTLS, "insecure-tls", false, "Allow insecure TLS connections (skip certificate verification)")

	return cmd
}

type checkerFunc func(ctx context.Context) error

func newChecker(target string, expectedStatus int, insecureTLS bool, timeout time.Duration) (checkerFunc, error) {
	switch {
	case len(target) >= 6 && target[:6] == "tcp://":
		addr := target[6:]
		return newTCPChecker(addr), nil
	case len(target) >= 7 && target[:7] == "http://":
		return newHTTPChecker(target, expectedStatus, false, timeout), nil
	case len(target) >= 8 && target[:8] == "https://":
		return newHTTPChecker(target, expectedStatus, insecureTLS, timeout), nil
	default:
		return nil, fmt.Errorf("unsupported target scheme in %q; use tcp://, http://, or https://", target)
	}
}

func newTCPChecker(addr string) checkerFunc {
	return func(ctx context.Context) error {
		var d net.Dialer
		conn, err := d.DialContext(ctx, "tcp", addr)
		if err != nil {
			return fmt.Errorf("tcp dial %s: %w", addr, err)
		}
		conn.Close()
		return nil
	}
}

func newHTTPChecker(url string, expectedStatus int, insecure bool, timeout time.Duration) checkerFunc {
	perRequestTimeout := 5 * time.Second
	if timeout < perRequestTimeout {
		perRequestTimeout = timeout
	}

	transport := &http.Transport{}
	if insecure {
		transport.TLSClientConfig = &tls.Config{InsecureSkipVerify: true} //nolint:gosec // user-opt-in via --insecure-tls
	}
	client := &http.Client{
		Timeout:   perRequestTimeout,
		Transport: transport,
	}

	return func(ctx context.Context) error {
		req, err := http.NewRequestWithContext(ctx, http.MethodGet, url, nil)
		if err != nil {
			return fmt.Errorf("creating request for %s: %w", url, err)
		}

		resp, err := client.Do(req)
		if err != nil {
			return fmt.Errorf("http request to %s: %w", url, err)
		}
		resp.Body.Close()

		if resp.StatusCode != expectedStatus {
			return fmt.Errorf("http %s returned status %d, expected %d", url, resp.StatusCode, expectedStatus)
		}
		return nil
	}
}
