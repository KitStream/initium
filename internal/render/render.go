package render

import (
	"bytes"
	"fmt"
	"os"
	"regexp"
	"text/template"
)

var envsubstPattern = regexp.MustCompile(`\$\{([a-zA-Z_][a-zA-Z0-9_]*)\}|\$([a-zA-Z_][a-zA-Z0-9_]*)`)

func Envsubst(input string) string {
	return envsubstPattern.ReplaceAllStringFunc(input, func(match string) string {
		var name string
		if match[1] == '{' {
			name = match[2 : len(match)-1]
		} else {
			name = match[1:]
		}
		if val, ok := os.LookupEnv(name); ok {
			return val
		}
		return match
	})
}

func GoTemplate(input string) (string, error) {
	envMap := envToMap()

	tmpl, err := template.New("initium").Option("missingkey=zero").Parse(input)
	if err != nil {
		return "", fmt.Errorf("parsing template: %w", err)
	}

	var buf bytes.Buffer
	if err := tmpl.Execute(&buf, envMap); err != nil {
		return "", fmt.Errorf("executing template: %w", err)
	}

	return buf.String(), nil
}

func envToMap() map[string]string {
	m := make(map[string]string)
	for _, entry := range os.Environ() {
		for i := 0; i < len(entry); i++ {
			if entry[i] == '=' {
				m[entry[:i]] = entry[i+1:]
				break
			}
		}
	}
	return m
}
