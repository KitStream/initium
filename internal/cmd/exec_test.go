package cmd

import (
	"bytes"
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"testing"

	"github.com/kitstream/initium/internal/logging"
)

func TestExecCmdNoArgs(t *testing.T) {
	lg := logging.Default()
	c := NewExecCmd(lg)
	c.SetArgs([]string{})
	err := c.Execute()
	if err == nil {
		t.Fatal("expected error when no command specified")
	}
	if !strings.Contains(err.Error(), "command is required") {
		t.Fatalf("unexpected error: %v", err)
	}
}

func TestExecCmdSuccess(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("skipping on windows")
	}

	var buf bytes.Buffer
	lg := logging.New(&buf, false, logging.LevelInfo)
	c := NewExecCmd(lg)
	c.SetArgs([]string{"--", "echo", "hello exec"})

	err := c.Execute()
	if err != nil {
		t.Fatalf("expected success, got: %v", err)
	}

	output := buf.String()
	if !strings.Contains(output, "hello exec") {
		t.Fatalf("expected command output in logs, got: %s", output)
	}
	if !strings.Contains(output, "command completed successfully") {
		t.Fatalf("expected completion message, got: %s", output)
	}
}

func TestExecCmdExitCode(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("skipping on windows")
	}

	var buf bytes.Buffer
	lg := logging.New(&buf, false, logging.LevelInfo)
	c := NewExecCmd(lg)
	c.SetArgs([]string{"--", "sh", "-c", "exit 42"})

	err := c.Execute()
	if err == nil {
		t.Fatal("expected error for non-zero exit code")
	}
	if !strings.Contains(err.Error(), "exited with code 42") {
		t.Fatalf("expected exit code 42, got: %v", err)
	}
}

func TestExecCmdStdoutStderr(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("skipping on windows")
	}

	var buf bytes.Buffer
	lg := logging.New(&buf, false, logging.LevelInfo)
	c := NewExecCmd(lg)
	c.SetArgs([]string{"--", "sh", "-c", "echo out-line; echo err-line >&2"})

	err := c.Execute()
	if err != nil {
		t.Fatalf("expected success, got: %v", err)
	}

	output := buf.String()
	if !strings.Contains(output, "out-line") {
		t.Fatalf("expected stdout line, got: %s", output)
	}
	if !strings.Contains(output, "err-line") {
		t.Fatalf("expected stderr line, got: %s", output)
	}
}

func TestExecCmdJSONOutput(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("skipping on windows")
	}

	var buf bytes.Buffer
	lg := logging.New(&buf, false, logging.LevelInfo)
	c := NewExecCmd(lg)
	c.SetArgs([]string{"--json", "--", "echo", "json-test"})

	err := c.Execute()
	if err != nil {
		t.Fatalf("expected success, got: %v", err)
	}

	output := buf.String()
	if !strings.Contains(output, `"msg"`) {
		t.Fatalf("expected JSON output, got: %s", output)
	}
}

func TestExecCmdWorkdir(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("skipping on windows")
	}

	dir := t.TempDir()
	markerFile := filepath.Join(dir, "marker.txt")

	var buf bytes.Buffer
	lg := logging.New(&buf, false, logging.LevelInfo)
	c := NewExecCmd(lg)
	c.SetArgs([]string{"--workdir", dir, "--", "sh", "-c", "pwd > marker.txt"})

	err := c.Execute()
	if err != nil {
		t.Fatalf("expected success, got: %v", err)
	}

	content, err := os.ReadFile(markerFile)
	if err != nil {
		t.Fatalf("failed to read marker file: %v", err)
	}
	got := strings.TrimSpace(string(content))
	if got != dir {
		t.Fatalf("expected workdir %q, got %q", dir, got)
	}
}

func TestExecCmdCommandNotFound(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("skipping on windows")
	}

	var buf bytes.Buffer
	lg := logging.New(&buf, false, logging.LevelInfo)
	c := NewExecCmd(lg)
	c.SetArgs([]string{"--", "/nonexistent/command"})

	err := c.Execute()
	if err == nil {
		t.Fatal("expected error for command not found")
	}
}

func TestExecCmdMultipleArgs(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("skipping on windows")
	}

	var buf bytes.Buffer
	lg := logging.New(&buf, false, logging.LevelInfo)
	c := NewExecCmd(lg)
	c.SetArgs([]string{"--", "echo", "arg1", "arg2", "arg3"})

	err := c.Execute()
	if err != nil {
		t.Fatalf("expected success, got: %v", err)
	}

	output := buf.String()
	if !strings.Contains(output, "arg1 arg2 arg3") {
		t.Fatalf("expected all args in output, got: %s", output)
	}
}

func TestExecCmdExitCode1(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("skipping on windows")
	}

	var buf bytes.Buffer
	lg := logging.New(&buf, false, logging.LevelInfo)
	c := NewExecCmd(lg)
	c.SetArgs([]string{"--", "sh", "-c", "exit 1"})

	err := c.Execute()
	if err == nil {
		t.Fatal("expected error for exit code 1")
	}
	if !strings.Contains(err.Error(), "exited with code 1") {
		t.Fatalf("expected exit code 1, got: %v", err)
	}
}

func TestExecCmdStartMessage(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("skipping on windows")
	}

	var buf bytes.Buffer
	lg := logging.New(&buf, false, logging.LevelInfo)
	c := NewExecCmd(lg)
	c.SetArgs([]string{"--", "echo", "hi"})

	err := c.Execute()
	if err != nil {
		t.Fatalf("expected success, got: %v", err)
	}

	output := buf.String()
	if !strings.Contains(output, "executing command") {
		t.Fatalf("expected start message, got: %s", output)
	}
}

func TestExecCmdHelpOutput(t *testing.T) {
	lg := logging.Default()
	c := NewExecCmd(lg)

	if c.Use != "exec -- COMMAND [ARGS...]" {
		t.Fatalf("unexpected Use: %s", c.Use)
	}
	if !strings.Contains(c.Short, "arbitrary") {
		t.Fatalf("Short should mention arbitrary: %s", c.Short)
	}
}
