package cmd

import (
	"bytes"
	"fmt"
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"testing"

	"github.com/kitstream/initium/internal/logging"
)

func TestMigrateCmdNoArgs(t *testing.T) {
	log := logging.Default()
	cmd := NewMigrateCmd(log)
	cmd.SetArgs([]string{})
	err := cmd.Execute()
	if err == nil {
		t.Fatal("expected error when no command specified")
	}
	if !strings.Contains(err.Error(), "migration command is required") {
		t.Fatalf("unexpected error: %v", err)
	}
}

func TestMigrateCmdSuccess(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("skipping on windows")
	}

	var buf bytes.Buffer
	log := logging.New(&buf, false, logging.LevelInfo)
	cmd := NewMigrateCmd(log)
	cmd.SetArgs([]string{"--", "echo", "hello migration"})

	err := cmd.Execute()
	if err != nil {
		t.Fatalf("expected success, got: %v", err)
	}

	output := buf.String()
	if !strings.Contains(output, "migration completed successfully") {
		t.Fatalf("expected completion message, got: %s", output)
	}
	if !strings.Contains(output, "hello migration") {
		t.Fatalf("expected command output in logs, got: %s", output)
	}
}

func TestMigrateCmdExitCode(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("skipping on windows")
	}

	var buf bytes.Buffer
	log := logging.New(&buf, false, logging.LevelInfo)
	cmd := NewMigrateCmd(log)
	cmd.SetArgs([]string{"--", "sh", "-c", "exit 42"})

	err := cmd.Execute()
	if err == nil {
		t.Fatal("expected error for non-zero exit code")
	}
	if !strings.Contains(err.Error(), "exited with code 42") {
		t.Fatalf("expected exit code 42, got: %v", err)
	}
}

func TestMigrateCmdStdoutStderr(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("skipping on windows")
	}

	var buf bytes.Buffer
	log := logging.New(&buf, false, logging.LevelInfo)
	cmd := NewMigrateCmd(log)
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

func TestMigrateCmdJSONOutput(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("skipping on windows")
	}

	var buf bytes.Buffer
	log := logging.New(&buf, false, logging.LevelInfo)
	cmd := NewMigrateCmd(log)
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

func TestMigrateCmdLockFileSkip(t *testing.T) {
	workdir := t.TempDir()
	lockPath := filepath.Join(workdir, ".migrated")
	if err := os.WriteFile(lockPath, []byte("done"), 0o644); err != nil {
		t.Fatalf("failed to create lock file: %v", err)
	}

	var buf bytes.Buffer
	log := logging.New(&buf, false, logging.LevelInfo)
	cmd := NewMigrateCmd(log)
	cmd.SetArgs([]string{
		"--workdir", workdir,
		"--lock-file", ".migrated",
		"--",
		"sh", "-c", "echo should-not-run",
	})

	err := cmd.Execute()
	if err != nil {
		t.Fatalf("expected success (skip), got: %v", err)
	}

	output := buf.String()
	if !strings.Contains(output, "lock file exists, skipping migration") {
		t.Fatalf("expected skip message, got: %s", output)
	}
	if strings.Contains(output, "should-not-run") {
		t.Fatal("command should not have been executed when lock file exists")
	}
}

func TestMigrateCmdLockFileCreated(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("skipping on windows")
	}

	workdir := t.TempDir()

	var buf bytes.Buffer
	log := logging.New(&buf, false, logging.LevelInfo)
	cmd := NewMigrateCmd(log)
	cmd.SetArgs([]string{
		"--workdir", workdir,
		"--lock-file", ".migrated",
		"--",
		"echo", "migrating",
	})

	err := cmd.Execute()
	if err != nil {
		t.Fatalf("expected success, got: %v", err)
	}

	lockPath := filepath.Join(workdir, ".migrated")
	if _, err := os.Stat(lockPath); os.IsNotExist(err) {
		t.Fatal("expected lock file to be created after successful migration")
	}

	output := buf.String()
	if !strings.Contains(output, "lock file created") {
		t.Fatalf("expected lock file created message, got: %s", output)
	}
}

func TestMigrateCmdLockFileNotCreatedOnFailure(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("skipping on windows")
	}

	workdir := t.TempDir()

	var buf bytes.Buffer
	log := logging.New(&buf, false, logging.LevelInfo)
	cmd := NewMigrateCmd(log)
	cmd.SetArgs([]string{
		"--workdir", workdir,
		"--lock-file", ".migrated",
		"--",
		"sh", "-c", "exit 1",
	})

	err := cmd.Execute()
	if err == nil {
		t.Fatal("expected error for failed migration")
	}

	lockPath := filepath.Join(workdir, ".migrated")
	if _, err := os.Stat(lockPath); !os.IsNotExist(err) {
		t.Fatal("lock file should not be created when migration fails")
	}
}

func TestMigrateCmdLockFilePathTraversal(t *testing.T) {
	workdir := t.TempDir()

	var buf bytes.Buffer
	log := logging.New(&buf, false, logging.LevelInfo)
	cmd := NewMigrateCmd(log)
	cmd.SetArgs([]string{
		"--workdir", workdir,
		"--lock-file", "../../../etc/passwd",
		"--",
		"echo", "hello",
	})

	err := cmd.Execute()
	if err == nil {
		t.Fatal("expected error for path traversal in lock file")
	}
	if !strings.Contains(err.Error(), "path traversal") {
		t.Fatalf("expected path traversal error, got: %v", err)
	}
}

func TestMigrateCmdCommandNotFound(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("skipping on windows")
	}

	var buf bytes.Buffer
	log := logging.New(&buf, false, logging.LevelInfo)
	cmd := NewMigrateCmd(log)
	cmd.SetArgs([]string{"--", "/nonexistent/command"})

	err := cmd.Execute()
	if err == nil {
		t.Fatal("expected error for command not found")
	}
}

func TestExitCodeFromError(t *testing.T) {
	tests := []struct {
		name     string
		err      error
		expected int
	}{
		{"nil error", nil, 0},
		{"generic error", fmt.Errorf("something went wrong"), 1},
		{"exit code error", fmt.Errorf("migration exited with code 42"), 42},
		{"exit code 0 error", fmt.Errorf("migration exited with code 0"), 0},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			code := ExitCodeFromError(tt.err)
			if code != tt.expected {
				t.Fatalf("expected exit code %d, got %d", tt.expected, code)
			}
		})
	}
}

func TestRunCommandSuccess(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("skipping on windows")
	}

	var buf bytes.Buffer
	log := logging.New(&buf, false, logging.LevelInfo)

	exitCode, err := runCommand(log, []string{"echo", "test"})
	if err != nil {
		t.Fatalf("expected no error, got: %v", err)
	}
	if exitCode != 0 {
		t.Fatalf("expected exit code 0, got: %d", exitCode)
	}
}

func TestRunCommandFailure(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("skipping on windows")
	}

	var buf bytes.Buffer
	log := logging.New(&buf, false, logging.LevelInfo)

	exitCode, err := runCommand(log, []string{"sh", "-c", "exit 7"})
	if err != nil {
		t.Fatalf("expected no error (exit code returned), got: %v", err)
	}
	if exitCode != 7 {
		t.Fatalf("expected exit code 7, got: %d", exitCode)
	}
}
