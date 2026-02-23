package fetch

import (
	"context"
	"crypto/tls"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"os"
	"path/filepath"
	"time"

	"github.com/kitstream/initium/internal/safety"
)

type Config struct {
	URL                    string
	OutputPath             string
	Workdir                string
	AuthEnv                string
	InsecureTLS            bool
	FollowRedirects        bool
	AllowCrossSiteRedirect bool
	Timeout                time.Duration
}

func (c Config) Validate() error {
	if c.URL == "" {
		return fmt.Errorf("url is required")
	}
	if c.OutputPath == "" {
		return fmt.Errorf("output is required")
	}
	if c.AllowCrossSiteRedirect && !c.FollowRedirects {
		return fmt.Errorf("--allow-cross-site-redirects requires --follow-redirects")
	}
	return nil
}

func Do(ctx context.Context, cfg Config) error {
	if err := cfg.Validate(); err != nil {
		return err
	}

	outPath, err := safety.ValidateFilePath(cfg.Workdir, cfg.OutputPath)
	if err != nil {
		return fmt.Errorf("invalid output path: %w", err)
	}

	client := buildClient(cfg)

	req, err := http.NewRequestWithContext(ctx, http.MethodGet, cfg.URL, nil)
	if err != nil {
		return fmt.Errorf("creating request: %w", err)
	}

	if cfg.AuthEnv != "" {
		authVal := os.Getenv(cfg.AuthEnv)
		if authVal == "" {
			return fmt.Errorf("auth env var %q is empty or not set", cfg.AuthEnv)
		}
		req.Header.Set("Authorization", authVal)
	}

	resp, err := client.Do(req)
	if err != nil {
		return fmt.Errorf("HTTP request to %s: %w", cfg.URL, err)
	}
	defer resp.Body.Close()

	if resp.StatusCode < 200 || resp.StatusCode >= 300 {
		return fmt.Errorf("HTTP %s returned status %d", cfg.URL, resp.StatusCode)
	}

	body, err := io.ReadAll(resp.Body)
	if err != nil {
		return fmt.Errorf("reading response body: %w", err)
	}

	if err := os.MkdirAll(filepath.Dir(outPath), 0o755); err != nil {
		return fmt.Errorf("creating output directory: %w", err)
	}

	if err := os.WriteFile(outPath, body, 0o644); err != nil {
		return fmt.Errorf("writing output %s: %w", outPath, err)
	}

	return nil
}

func buildClient(cfg Config) *http.Client {
	transport := &http.Transport{}
	if cfg.InsecureTLS {
		transport.TLSClientConfig = &tls.Config{InsecureSkipVerify: true} //nolint:gosec // user-opt-in via --insecure-tls
	}

	client := &http.Client{
		Timeout:   cfg.Timeout,
		Transport: transport,
	}

	if !cfg.FollowRedirects {
		client.CheckRedirect = func(req *http.Request, via []*http.Request) error {
			return http.ErrUseLastResponse
		}
	} else if !cfg.AllowCrossSiteRedirect {
		client.CheckRedirect = sameSiteRedirectPolicy
	}

	return client
}

func sameSiteRedirectPolicy(req *http.Request, via []*http.Request) error {
	if len(via) >= 10 {
		return fmt.Errorf("too many redirects")
	}
	if len(via) == 0 {
		return nil
	}
	origHost := hostFromURL(via[0].URL)
	newHost := hostFromURL(req.URL)
	if origHost != newHost {
		return fmt.Errorf("cross-site redirect from %s to %s is not allowed; use --allow-cross-site-redirects to permit", origHost, newHost)
	}
	return nil
}

func hostFromURL(u *url.URL) string {
	h := u.Hostname()
	return h
}
