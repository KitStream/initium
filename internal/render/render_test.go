package render

import (
	"os"
	"strings"
	"testing"
)

func TestEnvsubstBasic(t *testing.T) {
	t.Setenv("RENDER_HOST", "localhost")
	t.Setenv("RENDER_PORT", "5432")

	input := "host=${RENDER_HOST} port=$RENDER_PORT"
	got := Envsubst(input)
	expected := "host=localhost port=5432"
	if got != expected {
		t.Fatalf("expected %q, got %q", expected, got)
	}
}

func TestEnvsubstMissingVar(t *testing.T) {
	os.Unsetenv("RENDER_MISSING_XYZ")

	input := "val=${RENDER_MISSING_XYZ}"
	got := Envsubst(input)
	if got != input {
		t.Fatalf("expected unchanged %q, got %q", input, got)
	}
}

func TestEnvsubstEmpty(t *testing.T) {
	got := Envsubst("")
	if got != "" {
		t.Fatalf("expected empty string, got %q", got)
	}
}

func TestEnvsubstNoVars(t *testing.T) {
	input := "no variables here"
	got := Envsubst(input)
	if got != input {
		t.Fatalf("expected %q, got %q", input, got)
	}
}

func TestEnvsubstEmptyValue(t *testing.T) {
	t.Setenv("RENDER_EMPTY", "")

	input := "val=${RENDER_EMPTY}end"
	got := Envsubst(input)
	if got != "val=end" {
		t.Fatalf("expected %q, got %q", "val=end", got)
	}
}

func TestEnvsubstSpecialChars(t *testing.T) {
	t.Setenv("RENDER_SPECIAL", "hello world & <tag> \"quotes\"")

	input := "val=${RENDER_SPECIAL}"
	got := Envsubst(input)
	expected := "val=hello world & <tag> \"quotes\""
	if got != expected {
		t.Fatalf("expected %q, got %q", expected, got)
	}
}

func TestEnvsubstMultiline(t *testing.T) {
	t.Setenv("RENDER_DB", "mydb")

	input := "line1=${RENDER_DB}\nline2=$RENDER_DB\n"
	got := Envsubst(input)
	expected := "line1=mydb\nline2=mydb\n"
	if got != expected {
		t.Fatalf("expected %q, got %q", expected, got)
	}
}

func TestEnvsubstAdjacentVars(t *testing.T) {
	t.Setenv("RENDER_A", "foo")
	t.Setenv("RENDER_B", "bar")

	input := "${RENDER_A}${RENDER_B}"
	got := Envsubst(input)
	if got != "foobar" {
		t.Fatalf("expected %q, got %q", "foobar", got)
	}
}

func TestEnvsubstUnderscoreInName(t *testing.T) {
	t.Setenv("RENDER_MY_VAR_123", "works")

	got := Envsubst("${RENDER_MY_VAR_123}")
	if got != "works" {
		t.Fatalf("expected %q, got %q", "works", got)
	}
}

func TestGoTemplateBasic(t *testing.T) {
	t.Setenv("RENDER_HOST", "localhost")
	t.Setenv("RENDER_PORT", "5432")

	input := "host={{.RENDER_HOST}} port={{.RENDER_PORT}}"
	got, err := GoTemplate(input)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	expected := "host=localhost port=5432"
	if got != expected {
		t.Fatalf("expected %q, got %q", expected, got)
	}
}

func TestGoTemplateMissingVar(t *testing.T) {
	os.Unsetenv("RENDER_NONEXISTENT_VAR_XYZ")

	input := "val={{.RENDER_NONEXISTENT_VAR_XYZ}}"
	got, err := GoTemplate(input)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if got != "val=" {
		t.Fatalf("expected %q, got %q", "val=", got)
	}
}

func TestGoTemplateEmpty(t *testing.T) {
	got, err := GoTemplate("")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if got != "" {
		t.Fatalf("expected empty string, got %q", got)
	}
}

func TestGoTemplateInvalidSyntax(t *testing.T) {
	_, err := GoTemplate("{{.broken")
	if err == nil {
		t.Fatal("expected error for invalid template syntax")
	}
}

func TestGoTemplateConditional(t *testing.T) {
	t.Setenv("RENDER_ENABLED", "true")

	input := `{{if .RENDER_ENABLED}}on{{else}}off{{end}}`
	got, err := GoTemplate(input)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if got != "on" {
		t.Fatalf("expected %q, got %q", "on", got)
	}
}

func TestGoTemplateSpecialChars(t *testing.T) {
	t.Setenv("RENDER_JSON", `{"key":"value"}`)

	input := "data={{.RENDER_JSON}}"
	got, err := GoTemplate(input)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !strings.Contains(got, `{"key":"value"}`) {
		t.Fatalf("expected JSON content, got %q", got)
	}
}

func TestEnvToMap(t *testing.T) {
	t.Setenv("RENDER_TEST_MAP", "mapval")

	m := envToMap()
	if m["RENDER_TEST_MAP"] != "mapval" {
		t.Fatalf("expected mapval, got %q", m["RENDER_TEST_MAP"])
	}
}

func TestEnvsubstDollarWithoutVar(t *testing.T) {
	input := "price is $5.00"
	got := Envsubst(input)
	if got != input {
		t.Fatalf("expected %q, got %q", input, got)
	}
}
