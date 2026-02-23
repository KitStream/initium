package cmd

import (
	"context"
	"fmt"
	"time"

	"github.com/kitstream/initium/internal/fetch"
	"github.com/kitstream/initium/internal/logging"
	"github.com/kitstream/initium/internal/retry"
	"github.com/spf13/cobra"
)

func NewFetchCmd(log *logging.Logger) *cobra.Command {
	var (
		urlFlag                string
		output                 string
		workdir                string
		authEnv                string
		insecureTLS            bool
		followRedirects        bool
		allowCrossSiteRedirect bool
		timeout                time.Duration
		maxAttempts            int
		initialDelay           time.Duration
		maxDelay               time.Duration
		backoffFactor          float64
		jitterFraction         float64
		jsonLogs               bool
	)

	cmd := &cobra.Command{
		Use:   "fetch",
		Short: "Fetch secrets or config from HTTP(S) endpoints",
		Long: `Fetch a resource from an HTTP(S) endpoint and write the response body to a
file within the working directory.

Supports optional authentication via an environment variable (to avoid leaking
credentials in process argument lists), TLS verification skipping, redirect
control, and retries with exponential backoff.`,
		Example: `  # Fetch a config file
  initium fetch --url http://config-service:8080/app.json --output app.json

  # Fetch from Vault with auth token
  initium fetch --url https://vault:8200/v1/secret/data/app --output secrets.json \
    --auth-env VAULT_TOKEN --insecure-tls

  # Fetch with retries
  initium fetch --url http://api:8080/config --output config.json \
    --max-attempts 10 --initial-delay 2s

  # Follow redirects (same-site only by default)
  initium fetch --url http://cdn/config --output config.json --follow-redirects`,
		SilenceUsage:  true,
		SilenceErrors: true,
		RunE: func(cmd *cobra.Command, args []string) error {
			if jsonLogs {
				log.SetJSON(true)
			}

			if urlFlag == "" {
				return fmt.Errorf("--url is required")
			}
			if output == "" {
				return fmt.Errorf("--output is required")
			}

			retryCfg := retry.Config{
				MaxAttempts:    maxAttempts,
				InitialDelay:   initialDelay,
				MaxDelay:       maxDelay,
				BackoffFactor:  backoffFactor,
				JitterFraction: jitterFraction,
			}
			if err := retryCfg.Validate(); err != nil {
				return fmt.Errorf("invalid retry config: %w", err)
			}

			fetchCfg := fetch.Config{
				URL:                    urlFlag,
				OutputPath:             output,
				Workdir:                workdir,
				AuthEnv:                authEnv,
				InsecureTLS:            insecureTLS,
				FollowRedirects:        followRedirects,
				AllowCrossSiteRedirect: allowCrossSiteRedirect,
				Timeout:                timeout,
			}

			if err := fetchCfg.Validate(); err != nil {
				return err
			}

			ctx, cancel := context.WithTimeout(cmd.Context(), timeout)
			defer cancel()

			log.Info("fetching", "url", urlFlag, "output", output)

			result := retry.Do(ctx, retryCfg, func(ctx context.Context, attempt int) error {
				log.Debug("fetch attempt", "attempt", fmt.Sprintf("%d", attempt+1))
				return fetch.Do(ctx, fetchCfg)
			})

			if result.Err != nil {
				log.Error("fetch failed", "url", urlFlag, "error", result.Err.Error())
				return fmt.Errorf("fetch %s failed: %w", urlFlag, result.Err)
			}

			log.Info("fetch completed", "url", urlFlag, "output", output, "attempts", fmt.Sprintf("%d", result.Attempt+1))
			return nil
		},
	}

	cmd.Flags().StringVar(&urlFlag, "url", "", "Target URL to fetch (required)")
	cmd.Flags().StringVar(&output, "output", "", "Output file path relative to workdir (required)")
	cmd.Flags().StringVar(&workdir, "workdir", "/work", "Working directory for output files")
	cmd.Flags().StringVar(&authEnv, "auth-env", "", "Name of env var containing the Authorization header value")
	cmd.Flags().BoolVar(&insecureTLS, "insecure-tls", false, "Skip TLS certificate verification")
	cmd.Flags().BoolVar(&followRedirects, "follow-redirects", false, "Follow HTTP redirects")
	cmd.Flags().BoolVar(&allowCrossSiteRedirect, "allow-cross-site-redirects", false, "Allow cross-site redirects (requires --follow-redirects)")
	cmd.Flags().DurationVar(&timeout, "timeout", 5*time.Minute, "Overall timeout")
	cmd.Flags().IntVar(&maxAttempts, "max-attempts", 3, "Maximum retry attempts")
	cmd.Flags().DurationVar(&initialDelay, "initial-delay", time.Second, "Initial delay between retries")
	cmd.Flags().DurationVar(&maxDelay, "max-delay", 30*time.Second, "Maximum delay between retries")
	cmd.Flags().Float64Var(&backoffFactor, "backoff-factor", 2.0, "Backoff multiplier")
	cmd.Flags().Float64Var(&jitterFraction, "jitter", 0.1, "Jitter fraction (0.0-1.0)")
	cmd.Flags().BoolVar(&jsonLogs, "json", false, "Enable JSON log output")

	return cmd
}
