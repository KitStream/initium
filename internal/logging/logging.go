package logging

import (
	"encoding/json"
	"fmt"
	"io"
	"os"
	"strings"
	"sync"
	"time"
)

type Level int

const (
	LevelDebug Level = iota
	LevelInfo
	LevelWarn
	LevelError
)

func (l Level) String() string {
	switch l {
	case LevelDebug:
		return "DEBUG"
	case LevelInfo:
		return "INFO"
	case LevelWarn:
		return "WARN"
	case LevelError:
		return "ERROR"
	default:
		return "UNKNOWN"
	}
}

type Logger struct {
	mu       sync.Mutex
	out      io.Writer
	jsonMode bool
	level    Level
}

func New(out io.Writer, jsonMode bool, level Level) *Logger {
	return &Logger{
		out:      out,
		jsonMode: jsonMode,
		level:    level,
	}
}

func Default() *Logger {
	return New(os.Stderr, false, LevelInfo)
}

func (l *Logger) SetJSON(enabled bool) {
	l.mu.Lock()
	defer l.mu.Unlock()
	l.jsonMode = enabled
}

func (l *Logger) log(level Level, msg string, kvs ...string) {
	if level < l.level {
		return
	}

	l.mu.Lock()
	defer l.mu.Unlock()

	now := time.Now().UTC().Format(time.RFC3339)

	if l.jsonMode {
		entry := map[string]string{
			"time":  now,
			"level": level.String(),
			"msg":   msg,
		}
		for i := 0; i+1 < len(kvs); i += 2 {
			entry[kvs[i]] = RedactValue(kvs[i], kvs[i+1])
		}
		data, _ := json.Marshal(entry)
		fmt.Fprintf(l.out, "%s\n", data)
	} else {
		var sb strings.Builder
		fmt.Fprintf(&sb, "%s [%s] %s", now, level, msg)
		for i := 0; i+1 < len(kvs); i += 2 {
			fmt.Fprintf(&sb, " %s=%s", kvs[i], RedactValue(kvs[i], kvs[i+1]))
		}
		fmt.Fprintln(l.out, sb.String())
	}
}

func (l *Logger) Debug(msg string, kvs ...string) { l.log(LevelDebug, msg, kvs...) }
func (l *Logger) Info(msg string, kvs ...string)  { l.log(LevelInfo, msg, kvs...) }
func (l *Logger) Warn(msg string, kvs ...string)  { l.log(LevelWarn, msg, kvs...) }
func (l *Logger) Error(msg string, kvs ...string) { l.log(LevelError, msg, kvs...) }

var sensitiveKeys = map[string]bool{
	"password":      true,
	"secret":        true,
	"token":         true,
	"authorization": true,
	"auth":          true,
	"api_key":       true,
	"apikey":        true,
}

func RedactValue(key, value string) string {
	if sensitiveKeys[strings.ToLower(key)] {
		if len(value) == 0 {
			return ""
		}
		return "REDACTED"
	}
	return value
}
