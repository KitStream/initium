use std::time::{Duration, Instant};

pub struct Config {
    pub max_attempts: u32,
    pub initial_delay: Duration,
    pub max_delay: Duration,
    pub backoff_factor: f64,
    pub jitter_fraction: f64,
}

impl Config {
    pub fn validate(&self) -> Result<(), String> {
        if self.max_attempts < 1 {
            return Err(format!(
                "max-attempts must be >= 1, got {}",
                self.max_attempts
            ));
        }
        if self.initial_delay.is_zero() {
            return Err("initial-delay must be > 0".into());
        }
        if self.max_delay < self.initial_delay {
            return Err(format!(
                "max-delay ({:?}) must be >= initial-delay ({:?})",
                self.max_delay, self.initial_delay
            ));
        }
        if self.backoff_factor < 1.0 {
            return Err(format!(
                "backoff-factor must be >= 1.0, got {}",
                self.backoff_factor
            ));
        }
        if !(0.0..=1.0).contains(&self.jitter_fraction) {
            return Err(format!(
                "jitter-fraction must be in [0, 1], got {}",
                self.jitter_fraction
            ));
        }
        Ok(())
    }
}

pub fn delay(cfg: &Config, attempt: u32) -> Duration {
    let base = cfg.initial_delay.as_secs_f64() * cfg.backoff_factor.powi(attempt as i32);
    let capped = base.min(cfg.max_delay.as_secs_f64());
    let jitter = if cfg.jitter_fraction > 0.0 {
        capped * cfg.jitter_fraction * rand::random::<f64>()
    } else {
        0.0
    };
    Duration::from_secs_f64(capped + jitter)
}

pub struct RetryResult {
    pub attempt: u32,
    pub err: Option<String>,
}

pub fn do_retry<F>(cfg: &Config, deadline: Option<Instant>, mut f: F) -> RetryResult
where
    F: FnMut(u32) -> std::result::Result<(), String>,
{
    for attempt in 0..cfg.max_attempts {
        match f(attempt) {
            Ok(()) => return RetryResult { attempt, err: None },
            Err(e) => {
                if attempt == cfg.max_attempts - 1 {
                    return RetryResult {
                        attempt,
                        err: Some(format!(
                            "all {} attempts failed, last error: {}",
                            cfg.max_attempts, e
                        )),
                    };
                }
                let d = delay(cfg, attempt);
                if let Some(dl) = deadline {
                    if Instant::now() + d > dl {
                        return RetryResult {
                            attempt,
                            err: Some(format!("deadline exceeded after attempt {}", attempt + 1)),
                        };
                    }
                }
                std::thread::sleep(d);
            }
        }
    }
    RetryResult {
        attempt: 0,
        err: Some("max attempts reached".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> Config {
        Config {
            max_attempts: 3,
            initial_delay: Duration::from_millis(10),
            max_delay: Duration::from_millis(100),
            backoff_factor: 2.0,
            jitter_fraction: 0.0,
        }
    }

    #[test]
    fn test_validate_ok() {
        assert!(test_config().validate().is_ok());
    }

    #[test]
    fn test_validate_max_attempts() {
        let mut cfg = test_config();
        cfg.max_attempts = 0;
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_validate_initial_delay() {
        let mut cfg = test_config();
        cfg.initial_delay = Duration::ZERO;
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_validate_max_delay() {
        let mut cfg = test_config();
        cfg.max_delay = Duration::from_millis(1);
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_validate_backoff() {
        let mut cfg = test_config();
        cfg.backoff_factor = 0.5;
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_validate_jitter() {
        let mut cfg = test_config();
        cfg.jitter_fraction = 1.5;
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_delay_exponential() {
        let cfg = test_config();
        let d0 = delay(&cfg, 0);
        let d1 = delay(&cfg, 1);
        let d2 = delay(&cfg, 2);
        assert!(d1 > d0);
        assert!(d2 > d1);
    }

    #[test]
    fn test_delay_capped() {
        let cfg = test_config();
        let d = delay(&cfg, 100);
        assert!(d <= cfg.max_delay + Duration::from_millis(1));
    }

    #[test]
    fn test_do_success() {
        let cfg = test_config();
        let result = do_retry(&cfg, None, |_| Ok(()));
        assert!(result.err.is_none());
        assert_eq!(result.attempt, 0);
    }

    #[test]
    fn test_do_eventual_success() {
        let cfg = test_config();
        let result = do_retry(&cfg, None, |attempt| {
            if attempt < 2 {
                Err("not yet".into())
            } else {
                Ok(())
            }
        });
        assert!(result.err.is_none());
        assert_eq!(result.attempt, 2);
    }

    #[test]
    fn test_do_all_fail() {
        let cfg = test_config();
        let result = do_retry(&cfg, None, |_| Err("fail".into()));
        assert!(result.err.is_some());
        assert!(result.err.unwrap().contains("all 3 attempts failed"));
    }

    #[test]
    fn test_do_deadline() {
        let cfg = Config {
            max_attempts: 100,
            initial_delay: Duration::from_millis(50),
            max_delay: Duration::from_secs(1),
            backoff_factor: 1.0,
            jitter_fraction: 0.0,
        };
        let deadline = Instant::now() + Duration::from_millis(10);
        let result = do_retry(&cfg, Some(deadline), |_| Err("fail".into()));
        assert!(result.err.is_some());
        assert!(result.err.unwrap().contains("deadline"));
    }
}
