package cmd

import (
	"fmt"

	"github.com/kitstream/initium/internal/logging"
	"github.com/spf13/cobra"
)

func NewSeedCmd(log *logging.Logger) *cobra.Command {
	var jsonLogs bool

	cmd := &cobra.Command{
		Use:   "seed -- COMMAND [ARGS...]",
		Short: "Run a database seed command with structured logging",
		Long: `Execute a database seed command with structured logging and exit code forwarding.

The command is executed directly via execve (no shell). Use "--" to separate
initium flags from the seed command and its arguments.

Unlike migrate, seed has no idempotency hints — it is the caller's responsibility
to ensure seed operations are safe to repeat or are only run once.`,
		Example: `  # Seed from a SQL file
  initium seed -- psql -f /seeds/data.sql

  # Seed with a custom script
  initium seed -- /app/seed --file /seeds/data.sql

  # Seed with JSON logs
  initium seed --json -- python3 /scripts/seed.py`,
		SilenceUsage:  true,
		SilenceErrors: true,
		RunE: func(cmd *cobra.Command, args []string) error {
			if jsonLogs {
				log.SetJSON(true)
			}

			if len(args) == 0 {
				return fmt.Errorf("seed command is required after \"--\"")
			}

			log.Info("starting seed", "command", args[0])

			exitCode, err := runCommand(log, args)
			if err != nil {
				return fmt.Errorf("seed failed: %w", err)
			}

			if exitCode != 0 {
				return fmt.Errorf("seed exited with code %d", exitCode)
			}

			log.Info("seed completed successfully")
			return nil
		},
	}

	cmd.Flags().BoolVar(&jsonLogs, "json", false, "Enable JSON log output")

	return cmd
}
