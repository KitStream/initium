use crate::logging::Logger;
use crate::retry;
use crate::safety;
use std::fs;
use std::io::Read;
use std::time::{Duration, Instant};
pub struct Config {
    pub url: String,
    pub output: String,
    pub workdir: String,
    pub auth_env: String,
    pub insecure_tls: bool,
    pub follow_redirects: bool,
    pub allow_cross_site_redirects: bool,
    pub timeout: Duration,
}
impl Config {
    pub fn validate(&self) -> Result<(), String> {
        if self.url.is_empty() { return Err("--url is required".into()); }
        if self.output.is_empty() { return Err("--output is required".into()); }
        if self.allow_cross_site_redirects && !self.follow_redirects {
            return Err("--allow-cross-site-redirects requires --follow-redirects".into());
        }
        Ok(())
    }
}
pub fn run(log: &Logger, cfg: &Config, retry_cfg: &retry::Config) -> Result<(), String> {
    cfg.validate()?;
    let deadline = Instant::now() + cfg.timeout;
    log.info("fetching", &[("url", &cfg.url), ("output", &cfg.output)]);
    let result = retry::do_retry(retry_cfg, Some(deadline), |attempt| {
        log.debug("fetch attempt", &[("attempt", &format!("{}", attempt + 1))]);
        do_fetch(cfg)
    });
    if let Some(e) = result.err {
        log.error("fetch failed", &[("url", &cfg.url), ("error", &e)]);
        return Err(format!("fetch {} failed: {}", cfg.url, e));
    }
    log.info("fetch completed", &[("url", &cfg.url), ("output", &cfg.output), ("attempts", &format!("{}", result.attempt + 1))]);
    Ok(())
}
fn do_fetch(cfg: &Config) -> Result<(), String> {
    let out_path = safety::validate_file_path(&cfg.workdir, &cfg.output)?;
    let agent = if cfg.insecure_tls {
        use std::sync::Arc;
        let crypto_provider = rustls::crypto::ring::default_provider();
        let tls_config = rustls::ClientConfig::builder_with_provider(Arc::new(crypto_provider))
            .with_safe_default_protocol_versions().unwrap()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(super::wait_for::NoVerifier))
            .with_no_client_auth();
        ureq::AgentBuilder::new()
            .timeout(cfg.timeout)
            .tls_config(Arc::new(tls_config))
            .redirects(if cfg.follow_redirects { 10 } else { 0 })
            .build()
    } else {
        ureq::AgentBuilder::new()
            .timeout(cfg.timeout)
            .redirects(if cfg.follow_redirects { 10 } else { 0 })
            .build()
    };
    let mut req = agent.get(&cfg.url);
    if !cfg.auth_env.is_empty() {
        let auth_val = std::env::var(&cfg.auth_env)
            .map_err(|_| format!("auth env var {:?} is empty or not set", cfg.auth_env))?;
        if auth_val.is_empty() {
            return Err(format!("auth env var {:?} is empty or not set", cfg.auth_env));
        }
        req = req.set("Authorization", &auth_val);
    }
    let resp = req.call().map_err(|e| format!("HTTP request to {}: {}", cfg.url, e))?;
    let status = resp.status();
    if status < 200 || status >= 300 {
        return Err(format!("HTTP {} returned status {}", cfg.url, status));
    }
    let mut body = Vec::new();
    resp.into_reader().read_to_end(&mut body)
        .map_err(|e| format!("reading response body: {}", e))?;
    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("creating output directory: {}", e))?;
    }
    fs::write(&out_path, &body).map_err(|e| format!("writing output {:?}: {}", out_path, e))?;
    Ok(())
}
