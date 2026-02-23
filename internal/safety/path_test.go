package safety

import (
	"testing"
)

func TestValidateFilePath(t *testing.T) {
	tests := []struct {
		name    string
		workdir string
		target  string
		wantErr bool
	}{
		{"simple file", "/work", "config.yaml", false},
		{"nested file", "/work", "sub/dir/file.txt", false},
		{"path traversal", "/work", "../etc/passwd", true},
		{"absolute escape", "/work", "/etc/passwd", true},
		{"dot-dot in middle", "/work", "sub/../../etc/passwd", true},
		{"workdir itself", "/work", ".", false},
		{"empty workdir", "", "file.txt", true},
		{"empty target", "/work", "", false},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result, err := ValidateFilePath(tt.workdir, tt.target)
			if tt.wantErr {
				if err == nil {
					t.Fatalf("expected error for target=%q, got path=%q", tt.target, result)
				}
				return
			}
			if err != nil {
				t.Fatalf("unexpected error: %v", err)
			}
			if result == "" {
				t.Fatal("expected non-empty result")
			}
		})
	}
}
