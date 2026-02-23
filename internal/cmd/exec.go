package cmd

import (
	"fmt"

	"github.com/kitstream/initium/internal/logging"
	"github.com/spf13/cobra"
)

func NewExecCmd(log *logging.Logger) *cobra.Command {
	var (
		workdir  string
		jsonLogs bool
	)

	cmd := &cobra.Command{
		Use:   "exec -- COMMAND [ARGS...]",
		Short: "Run arbitrary commands with structured logging",
		Long: `Execute an arbitrary command with structured logging and exit code forwarding.

The command is executed directly via execve (no shell). Use "--" to separate
initium flags from the command and its arguments.

stdout and stderr are captured and logged with timestamps. The child process
exit code is forwarded. If --workdir is set, the child process working
directory is changed accordingly.`,
		Example: `  # Run a setup script
  initium exec -- /bin/setup.sh

  # Run with JSON logs
  initium exec --json -- python3 /scripts/init.py

  # Run in a specific directory
  initium exec --workdir /app -- ./prepare.sh

  # Generate a private key with openssl
  initium exec --workdir /certs -- openssl genrsa -out key.pem 4096`,
		SilenceUsage:  true,
		SilenceErrors: true,
		RunE: func(cmd *cobra.Command, args []string) error {
			if jsonLogs {
				log.SetJSON(true)
			}

			if len(args) == 0 {
				return fmt.Errorf("command is required after \"--\"")
			}

			log.Info("executing command", "command", args[0])

			exitCode, err := runCommandInDir(log, args, workdir)
			if err != nil {
				return fmt.Errorf("exec failed: %w", err)
			}

			if exitCode != 0 {
				return fmt.Errorf("command exited with code %d", exitCode)
			}

			log.Info("command completed successfully")
			return nil
		},
	}

	cmd.Flags().StringVar(&workdir, "workdir", "", "Working directory for the child process (default: inherit)")
	cmd.Flags().BoolVar(&jsonLogs, "json", false, "Enable JSON log output")

	return cmd
}

func runCommandInDir(log *logging.Logger, args []string, dir string) (int, error) {
	if dir == "" {
		return runCommand(log, args)
	}
	return runCommandWithDir(log, args, dir)
}

func runCommandWithDir(log *logging.Logger, args []string, dir string) (int, error) {
	c := newExecCommand(args[0], args[1:]...)
	c.Dir = dir
	return executeAndStream(log, c)
}
