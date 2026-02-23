package main

import (
	"context"
	"github.com/kitstream/initium/internal/cmd"
	"github.com/kitstream/initium/internal/logging"
	"github.com/spf13/cobra"
	"os"
)

var version = "dev"

func main() {
	var jsonLogs bool
	root := &cobra.Command{
		Use:   "initium",
		Short: "Swiss-army toolbox for Kubernetes initContainers",
		Long: `Initium is a multi-tool CLI for Kubernetes initContainers.
It provides subcommands to wait for dependencies, run migrations,
seed databases, render config templates, fetch secrets, and execute
arbitrary commands — all with safe defaults, structured logging,
and security guardrails.`,
		Version:       version,
		SilenceErrors: true,
		PersistentPreRun: func(c *cobra.Command, args []string) {
			if l, ok := c.Context().Value(loggerKey{}).(*logging.Logger); ok {
				l.SetJSON(jsonLogs)
			}
		},
	}
	root.PersistentFlags().BoolVar(&jsonLogs, "json", false, "Enable JSON log output")
	log := logging.Default()
	ctx := withLogger(context.Background(), log)
	root.SetContext(ctx)
	root.AddCommand(cmd.NewWaitForCmd(log))
	root.AddCommand(cmd.NewMigrateCmd(log))
	root.AddCommand(cmd.NewSeedCmd(log))
	root.AddCommand(cmd.NewRenderCmd(log))
	root.AddCommand(cmd.NewFetchCmd(log))
	root.AddCommand(cmd.NewExecCmd(log))
	if err := root.Execute(); err != nil {
		log.Error(err.Error())
		os.Exit(1)
	}
}
