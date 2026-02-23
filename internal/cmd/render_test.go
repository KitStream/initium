package cmd

import (
	"bytes"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/kitstream/initium/internal/logging"
)

func TestRenderCmdNoTemplate(t *testing.T) {
	log := logging.Default()
	cmd := NewRenderCmd(log)
	cmd.SetArgs([]string{"--output", "out.conf"})
	err := cmd.Execute()
	if err == nil {
		t.Fatal("expected error when --template not specified")
	}
	if !strings.Contains(err.Error(), "--template is required") {
		t.Fatalf("unexpected error: %v", err)
	}
}

func TestRenderCmdNoOutput(t *testing.T) {
	log := logging.Default()
	cmd := NewRenderCmd(log)
	cmd.SetArgs([]string{"--template", "/tmp/some.tmpl"})
	err := cmd.Execute()
	if err == nil {
		t.Fatal("expected error when --output not specified")
	}
	if !strings.Contains(err.Error(), "--output is required") {
		t.Fatalf("unexpected error: %v", err)
	}
}

func TestRenderCmdInvalidMode(t *testing.T) {
	log := logging.Default()
	cmd := NewRenderCmd(log)
	cmd.SetArgs([]string{"--template", "/tmp/t.tmpl", "--output", "o.conf", "--mode", "invalid"})
	err := cmd.Execute()
	if err == nil {
		t.Fatal("expected error for invalid mode")
	}
	if !strings.Contains(err.Error(), "must be envsubst or gotemplate") {
		t.Fatalf("unexpected error: %v", err)
	}
}

func TestRenderCmdEnvsubst(t *testing.T) {
	t.Setenv("RENDER_TEST_HOST", "db.local")
	t.Setenv("RENDER_TEST_PORT", "3306")

	tmplDir := t.TempDir()
	tmplFile := filepath.Join(tmplDir, "app.conf.tmpl")
	os.WriteFile(tmplFile, []byte("host=${RENDER_TEST_HOST}\nport=$RENDER_TEST_PORT\n"), 0o644)

	workdir := t.TempDir()

	var buf bytes.Buffer
	log := logging.New(&buf, false, logging.LevelInfo)
	cmd := NewRenderCmd(log)
	cmd.SetArgs([]string{
		"--template", tmplFile,
		"--output", "app.conf",
		"--workdir", workdir,
	})

	err := cmd.Execute()
	if err != nil {
		t.Fatalf("expected success, got: %v", err)
	}

	content, err := os.ReadFile(filepath.Join(workdir, "app.conf"))
	if err != nil {
		t.Fatalf("failed to read output: %v", err)
	}

	expected := "host=db.local\nport=3306\n"
	if string(content) != expected {
		t.Fatalf("expected %q, got %q", expected, string(content))
	}
}

func TestRenderCmdGoTemplate(t *testing.T) {
	t.Setenv("RENDER_TEST_NAME", "myapp")

	tmplDir := t.TempDir()
	tmplFile := filepath.Join(tmplDir, "app.conf.tmpl")
	os.WriteFile(tmplFile, []byte("name={{.RENDER_TEST_NAME}}\n"), 0o644)

	workdir := t.TempDir()

	var buf bytes.Buffer
	log := logging.New(&buf, false, logging.LevelInfo)
	cmd := NewRenderCmd(log)
	cmd.SetArgs([]string{
		"--template", tmplFile,
		"--output", "app.conf",
		"--workdir", workdir,
		"--mode", "gotemplate",
	})

	err := cmd.Execute()
	if err != nil {
		t.Fatalf("expected success, got: %v", err)
	}

	content, err := os.ReadFile(filepath.Join(workdir, "app.conf"))
	if err != nil {
		t.Fatalf("failed to read output: %v", err)
	}

	if string(content) != "name=myapp\n" {
		t.Fatalf("expected %q, got %q", "name=myapp\n", string(content))
	}
}

func TestRenderCmdPathTraversal(t *testing.T) {
	tmplDir := t.TempDir()
	tmplFile := filepath.Join(tmplDir, "t.tmpl")
	os.WriteFile(tmplFile, []byte("hello"), 0o644)

	workdir := t.TempDir()

	log := logging.Default()
	cmd := NewRenderCmd(log)
	cmd.SetArgs([]string{
		"--template", tmplFile,
		"--output", "../../../etc/passwd",
		"--workdir", workdir,
	})

	err := cmd.Execute()
	if err == nil {
		t.Fatal("expected error for path traversal")
	}
	if !strings.Contains(err.Error(), "path traversal") {
		t.Fatalf("expected path traversal error, got: %v", err)
	}
}

func TestRenderCmdMissingTemplate(t *testing.T) {
	workdir := t.TempDir()

	log := logging.Default()
	cmd := NewRenderCmd(log)
	cmd.SetArgs([]string{
		"--template", "/nonexistent/template.tmpl",
		"--output", "out.conf",
		"--workdir", workdir,
	})

	err := cmd.Execute()
	if err == nil {
		t.Fatal("expected error for missing template file")
	}
}

func TestRenderCmdNestedOutputDir(t *testing.T) {
	t.Setenv("RENDER_TEST_VAL", "nested")

	tmplDir := t.TempDir()
	tmplFile := filepath.Join(tmplDir, "t.tmpl")
	os.WriteFile(tmplFile, []byte("val=${RENDER_TEST_VAL}"), 0o644)

	workdir := t.TempDir()

	var buf bytes.Buffer
	log := logging.New(&buf, false, logging.LevelInfo)
	cmd := NewRenderCmd(log)
	cmd.SetArgs([]string{
		"--template", tmplFile,
		"--output", "sub/dir/out.conf",
		"--workdir", workdir,
	})

	err := cmd.Execute()
	if err != nil {
		t.Fatalf("expected success, got: %v", err)
	}

	content, err := os.ReadFile(filepath.Join(workdir, "sub", "dir", "out.conf"))
	if err != nil {
		t.Fatalf("failed to read nested output: %v", err)
	}
	if string(content) != "val=nested" {
		t.Fatalf("expected %q, got %q", "val=nested", string(content))
	}
}

func TestRenderCmdGoTemplateInvalidSyntax(t *testing.T) {
	tmplDir := t.TempDir()
	tmplFile := filepath.Join(tmplDir, "bad.tmpl")
	os.WriteFile(tmplFile, []byte("{{.broken"), 0o644)

	workdir := t.TempDir()

	log := logging.Default()
	cmd := NewRenderCmd(log)
	cmd.SetArgs([]string{
		"--template", tmplFile,
		"--output", "out.conf",
		"--workdir", workdir,
		"--mode", "gotemplate",
	})

	err := cmd.Execute()
	if err == nil {
		t.Fatal("expected error for invalid go template syntax")
	}
}

func TestRenderCmdJSONOutput(t *testing.T) {
	t.Setenv("RENDER_TEST_J", "jval")

	tmplDir := t.TempDir()
	tmplFile := filepath.Join(tmplDir, "t.tmpl")
	os.WriteFile(tmplFile, []byte("v=$RENDER_TEST_J"), 0o644)

	workdir := t.TempDir()

	var buf bytes.Buffer
	log := logging.New(&buf, false, logging.LevelInfo)
	cmd := NewRenderCmd(log)
	cmd.SetArgs([]string{
		"--json",
		"--template", tmplFile,
		"--output", "out.conf",
		"--workdir", workdir,
	})

	err := cmd.Execute()
	if err != nil {
		t.Fatalf("expected success, got: %v", err)
	}

	output := buf.String()
	if !strings.Contains(output, `"msg"`) {
		t.Fatalf("expected JSON output, got: %s", output)
	}
}

func TestRenderCmdEmptyTemplate(t *testing.T) {
	tmplDir := t.TempDir()
	tmplFile := filepath.Join(tmplDir, "empty.tmpl")
	os.WriteFile(tmplFile, []byte(""), 0o644)

	workdir := t.TempDir()

	log := logging.Default()
	cmd := NewRenderCmd(log)
	cmd.SetArgs([]string{
		"--template", tmplFile,
		"--output", "empty.conf",
		"--workdir", workdir,
	})

	err := cmd.Execute()
	if err != nil {
		t.Fatalf("expected success for empty template, got: %v", err)
	}

	content, err := os.ReadFile(filepath.Join(workdir, "empty.conf"))
	if err != nil {
		t.Fatalf("failed to read output: %v", err)
	}
	if string(content) != "" {
		t.Fatalf("expected empty output, got %q", string(content))
	}
}
