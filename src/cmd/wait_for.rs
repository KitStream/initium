use crate::logging::Logger;
use crate::retry;
use std::net::TcpStream;
use std::time::{Duration, Instant};
pub fn run(
    log: &Logger,
    targets: &[String],
    cfg: &retry::Config,
    timeout: Duration,
    http_status: u16,
    insecure_tls: bool,
) -> Result<(), String> {
    if targets.is_empty() {
        return Err("at least one --target is required".into());
    }
    let deadline = Instant::now() + timeout;
    for target in targets {
        log.info("waiting for target", &[("target", target)]);
        let result = retry::do_retry(cfg, Some(deadline), |attempt| {
            log.debug(
                "attempt",
                &[("target", target), ("attempt", &format!("{}", attempt + 1))],
            );
            check_target(target, http_status, insecure_tls, timeout)
        });
        if let Some(e) = result.err {
            log.error("target not reachable", &[("target", target), ("error", &e)]);
            return Err(format!("target {} not reachable: {}", target, e));
        }
        log.info(
            "target is reachable",
            &[
                ("target", target),
                ("attempts", &format!("{}", result.attempt + 1)),
            ],
        );
    }
    log.info("all targets reachable", &[]);
    Ok(())
}
fn check_target(
    target: &str,
    expected_status: u16,
    insecure_tls: bool,
    timeout: Duration,
) -> Result<(), String> {
    if let Some(addr) = target.strip_prefix("tcp://") {
        check_tcp(addr, timeout)
    } else if target.starts_with("http://") || target.starts_with("https://") {
        check_http(target, expected_status, insecure_tls, timeout)
    } else {
        Err(format!(
            "unsupported target scheme in {:?}; use tcp://, http://, or https://",
            target
        ))
    }
}
fn check_tcp(addr: &str, timeout: Duration) -> Result<(), String> {
    let per_req = timeout.min(Duration::from_secs(5));
    let addrs: Vec<std::net::SocketAddr> = addr
        .to_socket_addrs_safe()
        .map_err(|e| format!("resolving {}: {}", addr, e))?;
    if addrs.is_empty() {
        return Err(format!("could not resolve {}", addr));
    }
    TcpStream::connect_timeout(&addrs[0], per_req)
        .map_err(|e| format!("tcp dial {}: {}", addr, e))?;
    Ok(())
}
fn check_http(
    url: &str,
    expected_status: u16,
    insecure_tls: bool,
    timeout: Duration,
) -> Result<(), String> {
    let per_req = timeout.min(Duration::from_secs(5));
    let agent = if insecure_tls {
        use std::sync::Arc;
        let crypto_provider = rustls::crypto::ring::default_provider();
        let tls_config = rustls::ClientConfig::builder_with_provider(Arc::new(crypto_provider))
            .with_safe_default_protocol_versions()
            .unwrap()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(NoVerifier))
            .with_no_client_auth();
        ureq::AgentBuilder::new()
            .timeout(per_req)
            .tls_config(Arc::new(tls_config))
            .build()
    } else {
        ureq::AgentBuilder::new().timeout(per_req).build()
    };
    let resp = agent
        .get(url)
        .call()
        .map_err(|e| format!("http request to {}: {}", url, e))?;
    let status = resp.status();
    if status != expected_status {
        return Err(format!(
            "http {} returned status {}, expected {}",
            url, status, expected_status
        ));
    }
    Ok(())
}
trait ToSocketAddrs {
    fn to_socket_addrs_safe(&self) -> std::io::Result<Vec<std::net::SocketAddr>>;
}
impl ToSocketAddrs for str {
    fn to_socket_addrs_safe(&self) -> std::io::Result<Vec<std::net::SocketAddr>> {
        use std::net::ToSocketAddrs;
        Ok(self.to_socket_addrs()?.collect())
    }
}
#[derive(Debug)]
pub struct NoVerifier;
impl rustls::client::danger::ServerCertVerifier for NoVerifier {
    fn verify_server_cert(
        &self,
        _: &rustls::pki_types::CertificateDer<'_>,
        _: &[rustls::pki_types::CertificateDer<'_>],
        _: &rustls::pki_types::ServerName<'_>,
        _: &[u8],
        _: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }
    fn verify_tls12_signature(
        &self,
        _: &[u8],
        _: &rustls::pki_types::CertificateDer<'_>,
        _: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }
    fn verify_tls13_signature(
        &self,
        _: &[u8],
        _: &rustls::pki_types::CertificateDer<'_>,
        _: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }
    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        rustls::crypto::ring::default_provider()
            .signature_verification_algorithms
            .supported_schemes()
    }
}
