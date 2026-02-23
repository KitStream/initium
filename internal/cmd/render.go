package cmd

import (
	"fmt"
	"os"
	"path/filepath"

	"github.com/kitstream/initium/internal/logging"
	"github.com/kitstream/initium/internal/render"
	"github.com/kitstream/initium/internal/safety"
	"github.com/spf13/cobra"
)

func NewRenderCmd(log *logging.Logger) *cobra.Command {
	var (
		templatePath string
		outputPath   string
		workdir      string
		mode         string
		jsonLogs     bool
	)

	cmd := &cobra.Command{
		Use:   "render",
		Short: "Render templates into config files",
		Long: `Render a template file into a config file using environment variable
substitution. Supports two modes:

  envsubst    — replaces ${VAR} and $VAR patterns (default)
  gotemplate  — full Go text/template with env vars as .VarName

Output files are written relative to --workdir with path traversal prevention.
Intermediate directories are created automatically.`,
		Example: `  # envsubst mode (default)
  initium render --template /templates/app.conf.tmpl --output app.conf

  # Go template mode
  initium render --mode gotemplate --template /templates/app.conf.tmpl --output app.conf

  # Custom workdir
  initium render --template /tpl/nginx.conf.tmpl --output nginx.conf --workdir /etc/nginx`,
		SilenceUsage:  true,
		SilenceErrors: true,
		RunE: func(cmd *cobra.Command, args []string) error {
			if jsonLogs {
				log.SetJSON(true)
			}

			if templatePath == "" {
				return fmt.Errorf("--template is required")
			}
			if outputPath == "" {
				return fmt.Errorf("--output is required")
			}
			if mode != "envsubst" && mode != "gotemplate" {
				return fmt.Errorf("--mode must be envsubst or gotemplate, got %q", mode)
			}

			outPath, err := safety.ValidateFilePath(workdir, outputPath)
			if err != nil {
				return fmt.Errorf("invalid output path: %w", err)
			}

			data, err := os.ReadFile(templatePath)
			if err != nil {
				return fmt.Errorf("reading template %s: %w", templatePath, err)
			}

			log.Info("rendering template", "template", templatePath, "output", outPath, "mode", mode)

			var result string
			switch mode {
			case "envsubst":
				result = render.Envsubst(string(data))
			case "gotemplate":
				result, err = render.GoTemplate(string(data))
				if err != nil {
					return fmt.Errorf("rendering template: %w", err)
				}
			}

			if err := os.MkdirAll(filepath.Dir(outPath), 0o755); err != nil {
				return fmt.Errorf("creating output directory: %w", err)
			}

			if err := os.WriteFile(outPath, []byte(result), 0o644); err != nil {
				return fmt.Errorf("writing output %s: %w", outPath, err)
			}

			log.Info("render completed", "output", outPath)
			return nil
		},
	}

	cmd.Flags().StringVar(&templatePath, "template", "", "Path to template file (required)")
	cmd.Flags().StringVar(&outputPath, "output", "", "Output file path relative to workdir (required)")
	cmd.Flags().StringVar(&workdir, "workdir", "/work", "Working directory for output files")
	cmd.Flags().StringVar(&mode, "mode", "envsubst", "Template mode: envsubst or gotemplate")
	cmd.Flags().BoolVar(&jsonLogs, "json", false, "Enable JSON log output")

	return cmd
}
