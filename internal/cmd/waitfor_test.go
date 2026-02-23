package cmd

import (
	"context"
	"fmt"
	"net"
	"net/http"
	"net/http/httptest"
	"testing"
	"time"

	"github.com/kitstream/initium/internal/logging"
)

func TestNewCheckerTCP(t *testing.T) {
	listener, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		t.Fatalf("failed to create listener: %v", err)
	}
	defer listener.Close()

	checker, err := newChecker("tcp://"+listener.Addr().String(), 200, false, 5*time.Second)
	if err != nil {
		t.Fatalf("newChecker failed: %v", err)
	}

	if err := checker(context.Background()); err != nil {
		t.Fatalf("TCP check failed: %v", err)
	}
}

func TestNewCheckerTCPUnreachable(t *testing.T) {
	checker, err := newChecker("tcp://127.0.0.1:1", 200, false, 5*time.Second)
	if err != nil {
		t.Fatalf("newChecker failed: %v", err)
	}

	err = checker(context.Background())
	if err == nil {
		t.Fatal("expected error for unreachable TCP target")
	}
}

func TestNewCheckerHTTP(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
	}))
	defer srv.Close()

	checker, err := newChecker(srv.URL, 200, false, 5*time.Second)
	if err != nil {
		t.Fatalf("newChecker failed: %v", err)
	}

	if err := checker(context.Background()); err != nil {
		t.Fatalf("HTTP check failed: %v", err)
	}
}

func TestNewCheckerHTTPWrongStatus(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusServiceUnavailable)
	}))
	defer srv.Close()

	checker, err := newChecker(srv.URL, 200, false, 5*time.Second)
	if err != nil {
		t.Fatalf("newChecker failed: %v", err)
	}

	err = checker(context.Background())
	if err == nil {
		t.Fatal("expected error for wrong HTTP status")
	}
}

func TestNewCheckerHTTPS(t *testing.T) {
	srv := httptest.NewTLSServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
	}))
	defer srv.Close()

	checker, err := newChecker(srv.URL, 200, false, 5*time.Second)
	if err != nil {
		t.Fatalf("newChecker failed: %v", err)
	}
	if err := checker(context.Background()); err == nil {
		t.Fatal("expected error for self-signed cert without insecure-tls")
	}

	checker2, err := newChecker(srv.URL, 200, true, 5*time.Second)
	if err != nil {
		t.Fatalf("newChecker failed: %v", err)
	}
	if err := checker2(context.Background()); err != nil {
		t.Fatalf("HTTPS check with insecure-tls failed: %v", err)
	}
}

func TestNewCheckerInvalidScheme(t *testing.T) {
	_, err := newChecker("ftp://example.com", 200, false, 5*time.Second)
	if err == nil {
		t.Fatal("expected error for unsupported scheme")
	}
}

func TestWaitForCmdNoTargets(t *testing.T) {
	log := logging.Default()
	cmd := NewWaitForCmd(log)
	cmd.SetArgs([]string{})
	err := cmd.Execute()
	if err == nil {
		t.Fatal("expected error when no targets specified")
	}
}

func TestWaitForCmdTCPSuccess(t *testing.T) {
	listener, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		t.Fatalf("failed to create listener: %v", err)
	}
	defer listener.Close()

	log := logging.Default()
	cmd := NewWaitForCmd(log)
	cmd.SetArgs([]string{
		"--target", fmt.Sprintf("tcp://%s", listener.Addr().String()),
		"--max-attempts", "3",
		"--initial-delay", "10ms",
		"--max-delay", "50ms",
		"--timeout", "5s",
	})

	if err := cmd.Execute(); err != nil {
		t.Fatalf("wait-for failed: %v", err)
	}
}

func TestWaitForCmdHTTPSuccess(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
	}))
	defer srv.Close()

	log := logging.Default()
	cmd := NewWaitForCmd(log)
	cmd.SetArgs([]string{
		"--target", srv.URL,
		"--max-attempts", "3",
		"--initial-delay", "10ms",
		"--max-delay", "50ms",
		"--timeout", "5s",
	})

	if err := cmd.Execute(); err != nil {
		t.Fatalf("wait-for HTTP failed: %v", err)
	}
}

func TestWaitForCmdTCPFailure(t *testing.T) {
	log := logging.Default()
	cmd := NewWaitForCmd(log)
	cmd.SilenceUsage = true
	cmd.SilenceErrors = true
	cmd.SetArgs([]string{
		"--target", "tcp://127.0.0.1:1",
		"--max-attempts", "2",
		"--initial-delay", "10ms",
		"--max-delay", "50ms",
		"--timeout", "2s",
	})

	err := cmd.Execute()
	if err == nil {
		t.Fatal("expected error for unreachable target")
	}
}

func TestWaitForCmdInvalidRetryConfig(t *testing.T) {
	log := logging.Default()
	cmd := NewWaitForCmd(log)
	cmd.SilenceUsage = true
	cmd.SilenceErrors = true
	cmd.SetArgs([]string{
		"--target", "tcp://localhost:1234",
		"--max-attempts", "0",
	})

	err := cmd.Execute()
	if err == nil {
		t.Fatal("expected error for invalid retry config")
	}
}
