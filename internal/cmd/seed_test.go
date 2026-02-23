package cmd

import (
	"bytes"
	"fmt"
	"runtime"
	"strings"
	"testing"

	"github.com/kitstream/initium/internal/logging"
)

func TestSeedCmdNoArgs(t *testing.T) {
	log := logging.Default()
	cmd := NewSeedCmd(log)
	cmd.SetArgs([]string{})
	err := cmd.Execute()
	if err == nil {
		t.Fatal("expected error when no command specified")
	}
	if !strings.Contains(err.Error(), "seed command is required") {
		t.Fatalf("unexpected error: %v", err)
	}
}

func TestSeedCmdSuccess(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("skipping on windows")
	}

	var buf bytes.Buffer
	log := logging.New(&buf, false, logging.LevelInfo)
	cmd := NewSeedCmd(log)
	cmd.SetArgs([]string{"--", "echo", "seeding data"})

	err := cmd.Execute()
	if err != nil {
		t.Fatalf("expected success, got: %v", err)
	}

	output := buf.String()
	if !strings.Contains(output, "seed completed successfully") {
		t.Fatalf("expected completion message, got: %s", output)
	}
	if !strings.Contains(output, "seeding data") {
		t.Fatalf("expected command output in logs, got: %s", output)
	}
}

func TestSeedCmdExitCode(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("skipping on windows")
	}

	var buf bytes.Buffer
	log := logging.New(&buf, false, logging.LevelInfo)
	cmd := NewSeedCmd(log)
	cmd.SetArgs([]string{"--", "sh", "-c", "exit 42"})

	err := cmd.Execute()
	if err == nil {
		t.Fatal("expected error for non-zero exit code")
	}
	if !strings.Contains(err.Error(), "exited with code 42") {
		t.Fatalf("expected exit code 42, got: %v", err)
	}
}

func TestSeedCmdStdoutStderr(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("skipping on windows")
	}

	var buf bytes.Buffer
	log := logging.New(&buf, false, logging.LevelInfo)
	cmd := NewSeedCmd(log)
	cmd.SetArgs([]string{"--", "sh", "-c", "echo out-line; echo err-line >&2"})

	err := cmd.Execute()
	if err != nil {
		t.Fatalf("expected success, got: %v", err)
	}

	output := buf.String()
	if !strings.Contains(output, "out-line") {
		t.Fatalf("expected stdout line in logs, got: %s", output)
	}
	if !strings.Contains(output, "err-line") {
		t.Fatalf("expected stderr line in logs, got: %s", output)
	}
}

func TestSeedCmdJSONOutput(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("skipping on windows")
	}

	var buf bytes.Buffer
	log := logging.New(&buf, false, logging.LevelInfo)
	cmd := NewSeedCmd(log)
	cmd.SetArgs([]string{"--json", "--", "echo", "json-test"})

	err := cmd.Execute()
	if err != nil {
		t.Fatalf("expected success, got: %v", err)
	}

	output := buf.String()
	if !strings.Contains(output, `"msg"`) {
		t.Fatalf("expected JSON output, got: %s", output)
	}
}

func TestSeedCmdCommandNotFound(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("skipping on windows")
	}

	var buf bytes.Buffer
	log := logging.New(&buf, false, logging.LevelInfo)
	cmd := NewSeedCmd(log)
	cmd.SetArgs([]string{"--", "/nonexistent/command"})

	err := cmd.Execute()
	if err == nil {
		t.Fatal("expected error for command not found")
	}
}

func TestSeedCmdMultipleArgs(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("skipping on windows")
	}

	var buf bytes.Buffer
	log := logging.New(&buf, false, logging.LevelInfo)
	cmd := NewSeedCmd(log)
	cmd.SetArgs([]string{"--", "echo", "arg1", "arg2", "arg3"})

	err := cmd.Execute()
	if err != nil {
		t.Fatalf("expected success, got: %v", err)
	}

	output := buf.String()
	if !strings.Contains(output, "arg1 arg2 arg3") {
		t.Fatalf("expected all args in output, got: %s", output)
	}
}

func TestSeedCmdStartMessage(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("skipping on windows")
	}

	var buf bytes.Buffer
	log := logging.New(&buf, false, logging.LevelInfo)
	cmd := NewSeedCmd(log)
	cmd.SetArgs([]string{"--", "echo", "hello"})

	err := cmd.Execute()
	if err != nil {
		t.Fatalf("expected success, got: %v", err)
	}

	output := buf.String()
	if !strings.Contains(output, "starting seed") {
		t.Fatalf("expected starting seed message, got: %s", output)
	}
}

func TestSeedCmdFailureExitCode1(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("skipping on windows")
	}

	var buf bytes.Buffer
	log := logging.New(&buf, false, logging.LevelInfo)
	cmd := NewSeedCmd(log)
	cmd.SetArgs([]string{"--", "sh", "-c", "exit 1"})

	err := cmd.Execute()
	if err == nil {
		t.Fatal("expected error for exit code 1")
	}
	if !strings.Contains(err.Error(), "exited with code 1") {
		t.Fatalf("expected exit code 1 in error, got: %v", err)
	}
}

func TestSeedCmdNoIdempotency(t *testing.T) {
	// Verify seed has no --lock-file flag (unlike migrate)
	log := logging.Default()
	cmd := NewSeedCmd(log)
	flag := cmd.Flags().Lookup("lock-file")
	if flag != nil {
		t.Fatal("seed should not have a --lock-file flag; idempotency is migrate-only")
	}
}

func TestSeedCmdHelpOutput(t *testing.T) {
	log := logging.Default()
	cmd := NewSeedCmd(log)

	if cmd.Use != "seed -- COMMAND [ARGS...]" {
		t.Fatalf("unexpected Use: %s", cmd.Use)
	}
	if !strings.Contains(cmd.Short, "seed") {
		t.Fatalf("Short should mention seed: %s", cmd.Short)
	}
}

// Remove unused import warning - fmt is used via TestSeedCmdNoArgs error check
var _ = fmt.Sprintf
