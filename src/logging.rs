use std::io::Write;
use std::sync::Mutex;
use std::time::SystemTime;

fn format_utc_now() -> String {
    let dur = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = dur.as_secs();
    let days = secs / 86400;
    let day_secs = secs % 86400;
    let h = day_secs / 3600;
    let m = (day_secs % 3600) / 60;
    let s = day_secs % 60;

    // Convert days since epoch to Y-M-D (civil calendar)
    let (y, mo, d) = days_to_ymd(days);
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", y, mo, d, h, m, s)
}

fn days_to_ymd(days_since_epoch: u64) -> (u64, u64, u64) {
    // Algorithm from Howard Hinnant's civil_from_days
    let z = days_since_epoch as i64 + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as u64, m, d)
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[allow(dead_code)]
pub enum Level {
    Debug,
    Info,
    Warn,
    Error,
}

impl std::fmt::Display for Level {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Level::Debug => write!(f, "DEBUG"),
            Level::Info => write!(f, "INFO"),
            Level::Warn => write!(f, "WARN"),
            Level::Error => write!(f, "ERROR"),
        }
    }
}

pub struct Logger {
    out: Mutex<Box<dyn Write + Send>>,
    json_mode: Mutex<bool>,
    level: Level,
}

impl Logger {
    pub fn new(out: Box<dyn Write + Send>, json_mode: bool, level: Level) -> Self {
        Self {
            out: Mutex::new(out),
            json_mode: Mutex::new(json_mode),
            level,
        }
    }

    pub fn default_logger() -> Self {
        Self::new(Box::new(std::io::stderr()), false, Level::Info)
    }

    pub fn set_json(&self, enabled: bool) {
        *self.json_mode.lock().unwrap() = enabled;
    }

    fn log(&self, level: Level, msg: &str, kvs: &[(&str, &str)]) {
        if level < self.level {
            return;
        }
        let now = format_utc_now();
        let json_mode = *self.json_mode.lock().unwrap();
        let mut out = self.out.lock().unwrap();

        if json_mode {
            let mut map = serde_json::Map::new();
            map.insert("time".into(), serde_json::Value::String(now));
            map.insert("level".into(), serde_json::Value::String(level.to_string()));
            map.insert("msg".into(), serde_json::Value::String(msg.into()));
            for (k, v) in kvs {
                map.insert((*k).into(), serde_json::Value::String(redact_value(k, v)));
            }
            let _ = writeln!(out, "{}", serde_json::Value::Object(map));
        } else {
            let mut line = format!("{} [{}] {}", now, level, msg);
            for (k, v) in kvs {
                line.push_str(&format!(" {}={}", k, redact_value(k, v)));
            }
            let _ = writeln!(out, "{}", line);
        }
    }

    pub fn debug(&self, msg: &str, kvs: &[(&str, &str)]) {
        self.log(Level::Debug, msg, kvs);
    }
    pub fn info(&self, msg: &str, kvs: &[(&str, &str)]) {
        self.log(Level::Info, msg, kvs);
    }
    #[allow(dead_code)]
    pub fn warn(&self, msg: &str, kvs: &[(&str, &str)]) {
        self.log(Level::Warn, msg, kvs);
    }
    pub fn error(&self, msg: &str, kvs: &[(&str, &str)]) {
        self.log(Level::Error, msg, kvs);
    }
}

const SENSITIVE_KEYS: &[&str] = &[
    "password",
    "secret",
    "token",
    "authorization",
    "auth",
    "api_key",
    "apikey",
];

pub fn redact_value(key: &str, value: &str) -> String {
    if SENSITIVE_KEYS.contains(&key.to_lowercase().as_str()) {
        if value.is_empty() {
            return String::new();
        }
        return "REDACTED".into();
    }
    value.into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn capture_logger(json: bool, level: Level) -> (Arc<Logger>, Arc<Mutex<Vec<u8>>>) {
        let buf = Arc::new(Mutex::new(Vec::new()));
        struct SharedBuf(Arc<Mutex<Vec<u8>>>);
        impl Write for SharedBuf {
            fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
                self.0.lock().unwrap().write(data)
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let logger = Arc::new(Logger::new(Box::new(SharedBuf(buf.clone())), json, level));
        (logger, buf)
    }

    #[test]
    fn test_text_output() {
        let (log, buf) = capture_logger(false, Level::Info);
        log.info("hello world", &[]);
        let output = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
        assert!(output.contains("[INFO]"));
        assert!(output.contains("hello world"));
    }

    #[test]
    fn test_json_output() {
        let (log, buf) = capture_logger(true, Level::Info);
        log.info("test message", &[("key", "val")]);
        let output = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
        assert!(output.contains("\"msg\""));
        assert!(output.contains("test message"));
        assert!(output.contains("\"key\""));
    }

    #[test]
    fn test_level_filtering() {
        let (log, buf) = capture_logger(false, Level::Warn);
        log.info("should not appear", &[]);
        log.warn("should appear", &[]);
        let output = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
        assert!(!output.contains("should not appear"));
        assert!(output.contains("should appear"));
    }

    #[test]
    fn test_redact_sensitive() {
        assert_eq!(redact_value("password", "secret123"), "REDACTED");
        assert_eq!(redact_value("Token", "abc"), "REDACTED");
        assert_eq!(redact_value("normal", "value"), "value");
        assert_eq!(redact_value("password", ""), "");
    }

    #[test]
    fn test_set_json() {
        let (log, buf) = capture_logger(false, Level::Info);
        log.info("text mode", &[]);
        log.set_json(true);
        log.info("json mode", &[]);
        let output = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
        assert!(output.contains("[INFO] text mode"));
        assert!(output.contains("\"msg\""));
    }

    #[test]
    fn test_kvs_in_text() {
        let (log, buf) = capture_logger(false, Level::Info);
        log.info("msg", &[("k1", "v1"), ("k2", "v2")]);
        let output = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
        assert!(output.contains("k1=v1"));
        assert!(output.contains("k2=v2"));
    }
}
