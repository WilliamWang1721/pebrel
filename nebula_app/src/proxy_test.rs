//! Semantic results for the settings-page network connectivity test.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProxyTestRoute {
    Direct,
    DirectAddress,
    CustomCommand,
    ProxyServer(String),
    SshJump(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProxyTestFailure {
    InvalidTarget,
    Tls(String),
    LoadSettings(String),
    SaveSettings(String),
    InvalidSettings(String),
    Timeout { seconds: u64 },
    SendRequest(String),
    ReadResponse(String),
    InvalidHttpStatusLine(String),
    HttpStatus { status: u16 },
    ProxyServer { server: String, error: String },
    CustomCommand(String),
    JumpTask(String),
    JumpResolve { target: String, error: String },
    JumpConnect { target: String, error: String },
    JumpChannel { target: String, error: String },
    Direct(String),
    Start(String),
    UnexpectedEnd,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProxyTestOutcome {
    Success(ProxyTestRoute),
    Failed(ProxyTestFailure),
}

impl ProxyTestOutcome {
    pub const fn is_success(&self) -> bool {
        matches!(self, Self::Success(_))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProxyTestResult {
    pub request_id: u64,
    pub outcome: ProxyTestOutcome,
    pub elapsed_ms: u64,
}

pub(crate) struct NetworkTestTarget {
    pub(crate) uri: ureq::http::Uri,
    pub(crate) host: String,
    pub(crate) port: u16,
    pub(crate) tls: bool,
}

impl NetworkTestTarget {
    pub(crate) fn parse(value: &str) -> Result<Self, ProxyTestFailure> {
        let uri: ureq::http::Uri =
            value.trim().parse().map_err(|_| ProxyTestFailure::InvalidTarget)?;
        let tls = match uri.scheme_str() {
            Some("http") => false,
            Some("https") => true,
            _ => return Err(ProxyTestFailure::InvalidTarget),
        };
        let authority = uri.authority().ok_or(ProxyTestFailure::InvalidTarget)?;
        if authority.as_str().contains('@') {
            return Err(ProxyTestFailure::InvalidTarget);
        }
        let suffix = &authority.as_str()[authority.host().len()..];
        let port = if let Some(port) = suffix.strip_prefix(':') {
            port.parse::<u16>().map_err(|_| ProxyTestFailure::InvalidTarget)?
        } else if tls {
            443
        } else {
            80
        };
        let host = authority.host().trim_start_matches('[').trim_end_matches(']').to_owned();
        if host.is_empty() || port == 0 || value.contains('#') {
            return Err(ProxyTestFailure::InvalidTarget);
        }
        Ok(Self { uri, host, port, tls })
    }

    pub(crate) fn request(&self) -> String {
        format!(
            "GET {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\nUser-Agent: Nebula-Network-Test\r\n\r\n",
            self.uri.path_and_query().map_or("/", |path| path.as_str()),
            self.uri.authority().expect("validated authority"),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_targets_preserve_host_port_path_and_query() {
        let target = NetworkTestTarget::parse("http://127.0.0.1:8080/health?probe=1").unwrap();
        assert_eq!((target.host.as_str(), target.port, target.tls), ("127.0.0.1", 8080, false));
        assert!(
            target
                .request()
                .starts_with("GET /health?probe=1 HTTP/1.1\r\nHost: 127.0.0.1:8080\r\n")
        );
        let target = NetworkTestTarget::parse("https://[::1]/health").unwrap();
        assert_eq!((target.host.as_str(), target.port, target.tls), ("::1", 443, true));
        assert!(target.request().contains("Host: [::1]\r\n"));
    }

    #[test]
    fn invalid_targets_are_rejected_before_connecting() {
        for url in [
            "example.com",
            "ftp://example.com",
            "http://user:pass@example.com/",
            "http://example.com:0",
            "http://example.com:99999",
            "http://example.com/#fragment",
            "http://example.com/\r\nX-Test: injected",
        ] {
            assert!(
                matches!(NetworkTestTarget::parse(url), Err(ProxyTestFailure::InvalidTarget)),
                "{url:?}"
            );
        }
    }
}
