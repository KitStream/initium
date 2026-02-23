package logging

import (
	"bytes"
	"encoding/json"
	"strings"
	"testing"
)

func TestTextOutput(t *testing.T) {
	var buf bytes.Buffer
	l := New(&buf, false, LevelInfo)
	l.Info("hello", "key", "value")
	out := buf.String()
	if !strings.Contains(out, "[INFO] hello") {
		t.Fatalf("expected INFO log, got: %s", out)
	}
	if !strings.Contains(out, "key=value") {
		t.Fatalf("expected key=value, got: %s", out)
	}
}

func TestJSONOutput(t *testing.T) {
	var buf bytes.Buffer
	l := New(&buf, true, LevelInfo)
	l.Info("hello", "key", "value")

	var entry map[string]string
	if err := json.Unmarshal(buf.Bytes(), &entry); err != nil {
		t.Fatalf("invalid JSON: %v", err)
	}
	if entry["msg"] != "hello" {
		t.Fatalf("expected msg=hello, got %s", entry["msg"])
	}
	if entry["key"] != "value" {
		t.Fatalf("expected key=value, got %s", entry["key"])
	}
}

func TestRedaction(t *testing.T) {
	var buf bytes.Buffer
	l := New(&buf, false, LevelInfo)
	l.Info("auth", "token", "supersecret")
	out := buf.String()
	if strings.Contains(out, "supersecret") {
		t.Fatalf("secret was not redacted: %s", out)
	}
	if !strings.Contains(out, "REDACTED") {
		t.Fatalf("expected REDACTED in output: %s", out)
	}
}

func TestLevelFiltering(t *testing.T) {
	var buf bytes.Buffer
	l := New(&buf, false, LevelWarn)
	l.Info("should not appear")
	l.Warn("should appear")
	out := buf.String()
	if strings.Contains(out, "should not appear") {
		t.Fatalf("info should be filtered at warn level")
	}
	if !strings.Contains(out, "should appear") {
		t.Fatalf("warn should not be filtered at warn level")
	}
}

func TestRedactEmptyValue(t *testing.T) {
	result := RedactValue("token", "")
	if result != "" {
		t.Fatalf("expected empty string for empty secret, got %s", result)
	}
}

func TestRedactNonSensitive(t *testing.T) {
	result := RedactValue("host", "example.com")
	if result != "example.com" {
		t.Fatalf("expected example.com, got %s", result)
	}
}
