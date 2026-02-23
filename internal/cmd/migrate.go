package cmd

import (
	"bufio"
	"fmt"
	"io"
	"os"
	"os/exec"
	"sync"
	"syscall"

	"github.com/kitstream/initium/internal/logging"
	"github.com/kitstream/initium/internal/safety"
	"github.com/spf13/cobra"
)

func NewMigrateCmd(log *logging.Logger) *cobra.Command {
	var (
		workdir  string
		lockFile string
		jsonLogs bool
	)

	cmd := &cobra.Command{
		Use:   "migrate -- COMMAND [ARGS...]",
		Short: "Run a database migration command with structured logging",
		Long: `Execute a database migration command with structured logging, exit code
forwarding, and optional idempotency via a lock file.

The command is executed directly via execve (no shell). Use "--" to separate
initium flags from the migration command and its arguments.

If --lock-file is set, the migration is skipped when the lock file already
exists inside --workdir. On successful completion the lock file is created
so subsequent runs become no-ops.`,
		Example: `  # Run a flyway migration
  initium migrate -- flyway migrate

  # Run with JSON logs
  initium migrate --json -- /app/migrate -path /migrations up

  # Idempotent: skip if already migrated
  initium migrate --lock-file .migrated --workdir /work -- /app/migrate up`,
		SilenceUsage:  true,
		SilenceErrors: true,
		RunE: func(cmd *cobra.Command, args []string) error {
			if jsonLogs {
				log.SetJSON(true)
			}

			if len(args) == 0 {
				return fmt.Errorf("migration command is required after \"--\"")
			}

			if lockFile != "" {
				lockPath, err := safety.ValidateFilePath(workdir, lockFile)
				if err != nil {
					return fmt.Errorf("invalid lock file path: %w", err)
				}

				if _, err := os.Stat(lockPath); err == nil {
					log.Info("lock file exists, skipping migration", "lock-file", lockPath)
					return nil
				}
			}

			log.Info("starting migration", "command", args[0])

			exitCode, err := runCommand(log, args)
			if err != nil {
				return fmt.Errorf("migration failed: %w", err)
			}

			if exitCode != 0 {
				return fmt.Errorf("migration exited with code %d", exitCode)
			}

			if lockFile != "" {
				lockPath, err := safety.ValidateFilePath(workdir, lockFile)
				if err != nil {
					return fmt.Errorf("invalid lock file path: %w", err)
				}

				if err := os.MkdirAll(workdir, 0o755); err != nil {
					return fmt.Errorf("creating workdir %s: %w", workdir, err)
				}

				if err := os.WriteFile(lockPath, []byte("migrated\n"), 0o644); err != nil {
					return fmt.Errorf("writing lock file %s: %w", lockPath, err)
				}
				log.Info("lock file created", "lock-file", lockPath)
			}

			log.Info("migration completed successfully")
			return nil
		},
	}

	cmd.Flags().StringVar(&workdir, "workdir", "/work", "Working directory for file operations")
	cmd.Flags().StringVar(&lockFile, "lock-file", "", "Skip migration if this file exists in workdir (idempotency)")
	cmd.Flags().BoolVar(&jsonLogs, "json", false, "Enable JSON log output")

	return cmd
}

func runCommand(log *logging.Logger, args []string) (int, error) {
	c := newExecCommand(args[0], args[1:]...)
	return executeAndStream(log, c)
}

func newExecCommand(name string, args ...string) *exec.Cmd {
	c := exec.Command(name, args...)
	c.Stdin = nil
	return c
}

func executeAndStream(log *logging.Logger, c *exec.Cmd) (int, error) {
	stdoutPipe, err := c.StdoutPipe()
	if err != nil {
		return -1, fmt.Errorf("creating stdout pipe: %w", err)
	}

	stderrPipe, err := c.StderrPipe()
	if err != nil {
		return -1, fmt.Errorf("creating stderr pipe: %w", err)
	}

	if err := c.Start(); err != nil {
		return -1, fmt.Errorf("starting command %q: %w", c.Path, err)
	}

	var wg sync.WaitGroup
	wg.Add(2)

	go func() {
		defer wg.Done()
		streamLines(log, stdoutPipe, "stdout")
	}()

	go func() {
		defer wg.Done()
		streamLines(log, stderrPipe, "stderr")
	}()

	wg.Wait()

	err = c.Wait()
	if err == nil {
		return 0, nil
	}

	var exitErr *exec.ExitError
	if ok := asExitError(err, &exitErr); ok {
		return exitErr.ExitCode(), nil
	}

	return -1, err
}

func asExitError(err error, target **exec.ExitError) bool {
	if e, ok := err.(*exec.ExitError); ok {
		*target = e
		return true
	}
	return false
}

func streamLines(log *logging.Logger, r io.Reader, stream string) {
	scanner := bufio.NewScanner(r)
	for scanner.Scan() {
		log.Info(scanner.Text(), "stream", stream)
	}
}

// ExitCodeFromError extracts the exit code from a command error.
// Used by callers that need to propagate exit codes (e.g., os.Exit).
func ExitCodeFromError(err error) int {
	if err == nil {
		return 0
	}

	// Check if the error message contains an exit code pattern
	var exitCode int
	if n, _ := fmt.Sscanf(err.Error(), "migration exited with code %d", &exitCode); n == 1 {
		return exitCode
	}

	// Check for underlying process exit status
	if exitErr, ok := err.(*exec.ExitError); ok {
		if status, ok := exitErr.Sys().(syscall.WaitStatus); ok {
			return status.ExitStatus()
		}
	}

	return 1
}
