package safety
import (
"fmt"
"path/filepath"
"strings"
)
func ValidateFilePath(workdir, target string) (string, error) {
if workdir == "" {
return "", fmt.Errorf("workdir must not be empty")
}
if filepath.IsAbs(target) {
return "", fmt.Errorf("absolute target path not allowed: %q", target)
}
absWorkdir, err := filepath.Abs(workdir)
if err != nil {
return "", fmt.Errorf("resolving workdir: %w", err)
}
joined := filepath.Join(absWorkdir, target)
cleaned := filepath.Clean(joined)
if !strings.HasPrefix(cleaned, absWorkdir+string(filepath.Separator)) && cleaned != absWorkdir {
return "", fmt.Errorf("path traversal detected: %q escapes workdir %q", target, absWorkdir)
}
return cleaned, nil
}
