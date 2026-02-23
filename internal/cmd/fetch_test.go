package cmd

import (
	"bytes"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/kitstream/initium/internal/logging"
)

func TestFetchCmdNoURL(t *testing.T) {
	log := logging.Default()
	c := NewFetchCmd(log)
	c.SetArgs([]string{"--output", "out.json"})
	err := c.Execute()
	if err == nil {
		t.Fatal("expected error when --url not specified")
	}
	if !strings.Contains(err.Error(), "--url is required") {
		t.Fatalf("unexpected error: %v", err)
	}
}

func TestFetchCmdNoOutput(t *testing.T) {
	log := logging.Default()
	c := NewFetchCmd(log)
	c.SetArgs([]string{"--url", "http://example.com"})
	err := c.Execute()
	if err == nil {
		t.Fatal("expected error when --output not specified")
	}
	if !strings.Contains(err.Error(), "--output is required") {
		t.Fatalf("unexpected error: %v", err)
	}
}

func TestFetchCmdSuccess(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
		_, _ = w.Write([]byte(`{"fetched":true}`))
	}))
	defer srv.Close()

	workdir := t.TempDir()

	var buf bytes.Buffer
	lg := logging.New(&buf, false, logging.LevelInfo)
	c := NewFetchCmd(lg)
	c.SetArgs([]string{
		"--url", srv.URL,
		"--output", "result.json",
		"--workdir", workdir,
		"--max-attempts", "1",
		"--timeout", "5s",
	})

	err := c.Execute()
	if err != nil {
		t.Fatalf("expected success, got: %v", err)
	}

	content, err := os.ReadFile(filepath.Join(workdir, "result.json"))
	if err != nil {
		t.Fatalf("failed to read output: %v", err)
	}
	if string(content) != `{"fetched":true}` {
		t.Fatalf("expected JSON body, got %q", string(content))
	}

	output := buf.String()
	if !strings.Contains(output, "fetch completed") {
		t.Fatalf("expected completion message, got: %s", output)
	}
}

func TestFetchCmdWithAuth(t *testing.T) {
	var gotAuth string
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		gotAuth = r.Header.Get("Authorization")
		w.WriteHeader(http.StatusOK)
		_, _ = w.Write([]byte("ok"))
	}))
	defer srv.Close()

	t.Setenv("TEST_FETCH_CMD_AUTH", "Bearer test-token-123")

	workdir := t.TempDir()
	lg := logging.Default()
	c := NewFetchCmd(lg)
	c.SetArgs([]string{
		"--url", srv.URL,
		"--output", "out.txt",
		"--workdir", workdir,
		"--auth-env", "TEST_FETCH_CMD_AUTH",
		"--max-attempts", "1",
		"--timeout", "5s",
	})

	err := c.Execute()
	if err != nil {
		t.Fatalf("expected success, got: %v", err)
	}

	if gotAuth != "Bearer test-token-123" {
		t.Fatalf("expected auth header, got %q", gotAuth)
	}
}

func TestFetchCmdPathTraversal(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
	}))
	defer srv.Close()

	workdir := t.TempDir()
	lg := logging.Default()
	c := NewFetchCmd(lg)
	c.SetArgs([]string{
		"--url", srv.URL,
		"--output", "../../../etc/passwd",
		"--workdir", workdir,
		"--max-attempts", "1",
		"--timeout", "5s",
	})

	err := c.Execute()
	if err == nil {
		t.Fatal("expected error for path traversal")
	}
}

func TestFetchCmdHTTPError(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusForbidden)
	}))
	defer srv.Close()

	workdir := t.TempDir()

	var buf bytes.Buffer
	lg := logging.New(&buf, false, logging.LevelInfo)
	c := NewFetchCmd(lg)
	c.SetArgs([]string{
		"--url", srv.URL,
		"--output", "out.txt",
		"--workdir", workdir,
		"--max-attempts", "1",
		"--initial-delay", "10ms",
		"--timeout", "5s",
	})

	err := c.Execute()
	if err == nil {
		t.Fatal("expected error for 403 status")
	}
}

func TestFetchCmdJSONOutput(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
		_, _ = w.Write([]byte("ok"))
	}))
	defer srv.Close()

	workdir := t.TempDir()

	var buf bytes.Buffer
	lg := logging.New(&buf, false, logging.LevelInfo)
	c := NewFetchCmd(lg)
	c.SetArgs([]string{
		"--json",
		"--url", srv.URL,
		"--output", "out.txt",
		"--workdir", workdir,
		"--max-attempts", "1",
		"--timeout", "5s",
	})

	err := c.Execute()
	if err != nil {
		t.Fatalf("expected success, got: %v", err)
	}

	output := buf.String()
	if !strings.Contains(output, `"msg"`) {
		t.Fatalf("expected JSON output, got: %s", output)
	}
}

func TestFetchCmdInvalidRetryConfig(t *testing.T) {
	lg := logging.Default()
	c := NewFetchCmd(lg)
	c.SetArgs([]string{
		"--url", "http://example.com",
		"--output", "out.txt",
		"--max-attempts", "0",
	})

	err := c.Execute()
	if err == nil {
		t.Fatal("expected error for invalid retry config")
	}
}

func TestFetchCmdCrossSiteWithoutFollowRedirects(t *testing.T) {
	lg := logging.Default()
	c := NewFetchCmd(lg)
	c.SetArgs([]string{
		"--url", "http://example.com",
		"--output", "out.txt",
		"--allow-cross-site-redirects",
	})

	err := c.Execute()
	if err == nil {
		t.Fatal("expected error for allow-cross-site-redirects without follow-redirects")
	}
	if !strings.Contains(err.Error(), "requires --follow-redirects") {
		t.Fatalf("unexpected error: %v", err)
	}
}
