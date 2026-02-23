package fetch

import (
	"context"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

func TestDoSuccess(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
		w.Write([]byte(`{"key":"value"}`))
	}))
	defer srv.Close()

	workdir := t.TempDir()
	cfg := Config{
		URL:        srv.URL,
		OutputPath: "out.json",
		Workdir:    workdir,
		Timeout:    5 * time.Second,
	}

	err := Do(context.Background(), cfg)
	if err != nil {
		t.Fatalf("expected success, got: %v", err)
	}

	content, err := os.ReadFile(filepath.Join(workdir, "out.json"))
	if err != nil {
		t.Fatalf("failed to read output: %v", err)
	}
	if string(content) != `{"key":"value"}` {
		t.Fatalf("expected JSON body, got %q", string(content))
	}
}

func TestDoAuthHeader(t *testing.T) {
	var gotAuth string
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		gotAuth = r.Header.Get("Authorization")
		w.WriteHeader(http.StatusOK)
		w.Write([]byte("ok"))
	}))
	defer srv.Close()

	t.Setenv("TEST_FETCH_AUTH", "Bearer my-token")

	workdir := t.TempDir()
	cfg := Config{
		URL:        srv.URL,
		OutputPath: "out.txt",
		Workdir:    workdir,
		AuthEnv:    "TEST_FETCH_AUTH",
		Timeout:    5 * time.Second,
	}

	err := Do(context.Background(), cfg)
	if err != nil {
		t.Fatalf("expected success, got: %v", err)
	}

	if gotAuth != "Bearer my-token" {
		t.Fatalf("expected auth header 'Bearer my-token', got %q", gotAuth)
	}
}

func TestDoAuthEnvEmpty(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
	}))
	defer srv.Close()

	os.Unsetenv("TEST_FETCH_AUTH_EMPTY")

	workdir := t.TempDir()
	cfg := Config{
		URL:        srv.URL,
		OutputPath: "out.txt",
		Workdir:    workdir,
		AuthEnv:    "TEST_FETCH_AUTH_EMPTY",
		Timeout:    5 * time.Second,
	}

	err := Do(context.Background(), cfg)
	if err == nil {
		t.Fatal("expected error for empty auth env var")
	}
	if !strings.Contains(err.Error(), "empty or not set") {
		t.Fatalf("unexpected error: %v", err)
	}
}

func TestDoMissingURL(t *testing.T) {
	cfg := Config{
		OutputPath: "out.txt",
		Workdir:    "/tmp",
		Timeout:    5 * time.Second,
	}

	err := Do(context.Background(), cfg)
	if err == nil {
		t.Fatal("expected error for missing URL")
	}
	if !strings.Contains(err.Error(), "url is required") {
		t.Fatalf("unexpected error: %v", err)
	}
}

func TestDoMissingOutput(t *testing.T) {
	cfg := Config{
		URL:     "http://example.com",
		Workdir: "/tmp",
		Timeout: 5 * time.Second,
	}

	err := Do(context.Background(), cfg)
	if err == nil {
		t.Fatal("expected error for missing output")
	}
	if !strings.Contains(err.Error(), "output is required") {
		t.Fatalf("unexpected error: %v", err)
	}
}

func TestDoPathTraversal(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
	}))
	defer srv.Close()

	workdir := t.TempDir()
	cfg := Config{
		URL:        srv.URL,
		OutputPath: "../../../etc/passwd",
		Workdir:    workdir,
		Timeout:    5 * time.Second,
	}

	err := Do(context.Background(), cfg)
	if err == nil {
		t.Fatal("expected error for path traversal")
	}
	if !strings.Contains(err.Error(), "path traversal") {
		t.Fatalf("expected path traversal error, got: %v", err)
	}
}

func TestDoHTTPError(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusInternalServerError)
	}))
	defer srv.Close()

	workdir := t.TempDir()
	cfg := Config{
		URL:        srv.URL,
		OutputPath: "out.txt",
		Workdir:    workdir,
		Timeout:    5 * time.Second,
	}

	err := Do(context.Background(), cfg)
	if err == nil {
		t.Fatal("expected error for 500 status")
	}
	if !strings.Contains(err.Error(), "status 500") {
		t.Fatalf("expected status 500 error, got: %v", err)
	}
}

func TestDoInsecureTLS(t *testing.T) {
	srv := httptest.NewTLSServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
		w.Write([]byte("tls-ok"))
	}))
	defer srv.Close()

	workdir := t.TempDir()

	// Without insecure TLS: should fail
	cfg := Config{
		URL:         srv.URL,
		OutputPath:  "out.txt",
		Workdir:     workdir,
		InsecureTLS: false,
		Timeout:     5 * time.Second,
	}
	err := Do(context.Background(), cfg)
	if err == nil {
		t.Fatal("expected error for self-signed cert without insecure-tls")
	}

	// With insecure TLS: should succeed
	cfg.InsecureTLS = true
	err = Do(context.Background(), cfg)
	if err != nil {
		t.Fatalf("expected success with insecure-tls, got: %v", err)
	}

	content, err := os.ReadFile(filepath.Join(workdir, "out.txt"))
	if err != nil {
		t.Fatalf("failed to read output: %v", err)
	}
	if string(content) != "tls-ok" {
		t.Fatalf("expected 'tls-ok', got %q", string(content))
	}
}

func TestDoNoFollowRedirects(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path == "/" {
			http.Redirect(w, r, "/target", http.StatusFound)
			return
		}
		w.WriteHeader(http.StatusOK)
		w.Write([]byte("redirected"))
	}))
	defer srv.Close()

	workdir := t.TempDir()
	cfg := Config{
		URL:             srv.URL,
		OutputPath:      "out.txt",
		Workdir:         workdir,
		FollowRedirects: false,
		Timeout:         5 * time.Second,
	}

	// Without follow-redirects, a 302 is a non-2xx status → error
	err := Do(context.Background(), cfg)
	if err == nil {
		t.Fatal("expected error for redirect without follow-redirects")
	}
	if !strings.Contains(err.Error(), "status 302") {
		t.Fatalf("expected status 302 error, got: %v", err)
	}
}

func TestDoFollowRedirectsSameSite(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path == "/" {
			http.Redirect(w, r, "/target", http.StatusFound)
			return
		}
		w.WriteHeader(http.StatusOK)
		w.Write([]byte("redirected"))
	}))
	defer srv.Close()

	workdir := t.TempDir()
	cfg := Config{
		URL:             srv.URL,
		OutputPath:      "out.txt",
		Workdir:         workdir,
		FollowRedirects: true,
		Timeout:         5 * time.Second,
	}

	err := Do(context.Background(), cfg)
	if err != nil {
		t.Fatalf("expected success for same-site redirect, got: %v", err)
	}

	content, err := os.ReadFile(filepath.Join(workdir, "out.txt"))
	if err != nil {
		t.Fatalf("failed to read output: %v", err)
	}
	if string(content) != "redirected" {
		t.Fatalf("expected 'redirected', got %q", string(content))
	}
}

func TestDoNestedOutputDir(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
		w.Write([]byte("nested"))
	}))
	defer srv.Close()

	workdir := t.TempDir()
	cfg := Config{
		URL:        srv.URL,
		OutputPath: "sub/dir/out.json",
		Workdir:    workdir,
		Timeout:    5 * time.Second,
	}

	err := Do(context.Background(), cfg)
	if err != nil {
		t.Fatalf("expected success, got: %v", err)
	}

	content, err := os.ReadFile(filepath.Join(workdir, "sub", "dir", "out.json"))
	if err != nil {
		t.Fatalf("failed to read output: %v", err)
	}
	if string(content) != "nested" {
		t.Fatalf("expected 'nested', got %q", string(content))
	}
}

func TestDoContextCancelled(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		time.Sleep(2 * time.Second)
		w.WriteHeader(http.StatusOK)
	}))
	defer srv.Close()

	ctx, cancel := context.WithTimeout(context.Background(), 100*time.Millisecond)
	defer cancel()

	workdir := t.TempDir()
	cfg := Config{
		URL:        srv.URL,
		OutputPath: "out.txt",
		Workdir:    workdir,
		Timeout:    10 * time.Second,
	}

	err := Do(ctx, cfg)
	if err == nil {
		t.Fatal("expected error for cancelled context")
	}
}

func TestValidateAllowCrossSiteWithoutFollowRedirects(t *testing.T) {
	cfg := Config{
		URL:                    "http://example.com",
		OutputPath:             "out.txt",
		Workdir:                "/tmp",
		AllowCrossSiteRedirect: true,
		FollowRedirects:        false,
	}

	err := cfg.Validate()
	if err == nil {
		t.Fatal("expected error when allow-cross-site-redirects is set without follow-redirects")
	}
	if !strings.Contains(err.Error(), "requires --follow-redirects") {
		t.Fatalf("unexpected error: %v", err)
	}
}
