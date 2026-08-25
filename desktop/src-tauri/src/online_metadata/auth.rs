use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use getrandom::fill;
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
use unlockm::metadata::provider::OnlineMetadataError;

const MAX_REQUEST_BYTES: usize = 8 * 1024;
const CALLBACK_TIMEOUT: Duration = Duration::from_secs(180);
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(2);

pub struct AuthorizationFlow {
    listener: TcpListener,
    pub redirect_uri: String,
    pub verifier: String,
    pub challenge: String,
    pub state: String,
    cancelled: Arc<AtomicBool>,
    pub generation: u64,
}

impl AuthorizationFlow {
    pub fn begin() -> Result<Self, OnlineMetadataError> {
        let listener = TcpListener::bind("127.0.0.1:0")
            .map_err(|_| OnlineMetadataError::AuthorizationFailed)?;
        listener
            .set_nonblocking(true)
            .map_err(|_| OnlineMetadataError::AuthorizationFailed)?;
        let port = listener
            .local_addr()
            .map_err(|_| OnlineMetadataError::AuthorizationFailed)?
            .port();
        let verifier = random_url_safe(48)?;
        let state = random_url_safe(32)?;
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        Ok(Self {
            listener,
            redirect_uri: format!("http://127.0.0.1:{port}/callback"),
            verifier,
            challenge,
            state,
            cancelled: Arc::new(AtomicBool::new(false)),
            generation: 0,
        })
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }
    pub fn cancellation_handle(&self) -> Arc<AtomicBool> {
        self.cancelled.clone()
    }

    /// Waits for a single bounded callback and returns only the code; no token
    /// or callback query is ever logged or forwarded to the webview.
    pub fn wait_for_code(&self) -> Result<String, OnlineMetadataError> {
        self.wait_for_code_with_timeouts(CALLBACK_TIMEOUT, CONNECTION_TIMEOUT)
    }

    fn wait_for_code_with_timeouts(
        &self,
        callback_timeout: Duration,
        connection_timeout: Duration,
    ) -> Result<String, OnlineMetadataError> {
        let started = Instant::now();
        loop {
            if self.cancelled.load(Ordering::Acquire) {
                return Err(OnlineMetadataError::AuthorizationCancelled);
            }
            if started.elapsed() >= callback_timeout {
                return Err(OnlineMetadataError::AuthorizationCancelled);
            }
            match self.listener.accept() {
                Ok((mut stream, peer)) => {
                    if !peer.ip().is_loopback() {
                        continue;
                    }
                    let result = read_callback(
                        &mut stream,
                        &self.state,
                        &self.cancelled,
                        connection_timeout,
                    );
                    let page = if result.is_ok() {
                        SUCCESS_PAGE
                    } else {
                        FAILURE_PAGE
                    };
                    let _ = stream.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", page.len(), page).as_bytes());
                    if let Ok(code) = result {
                        return Ok(code);
                    }
                    // A stray browser probe, wrong path/state, malformed
                    // callback, or duplicate request is rejected but does not
                    // let a local process consume the legitimate one-active
                    // authorization flow.
                    continue;
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(40))
                }
                Err(_) => return Err(OnlineMetadataError::AuthorizationFailed),
            }
        }
    }
}

const SUCCESS_PAGE: &str = "<!doctype html><title>UnlockM</title><p>Authorization complete. You may return to UnlockM.</p>";
const FAILURE_PAGE: &str = "<!doctype html><title>UnlockM</title><p>Authorization failed or was cancelled. You may close this window.</p>";

fn random_url_safe(bytes: usize) -> Result<String, OnlineMetadataError> {
    let mut value = vec![0_u8; bytes];
    fill(&mut value).map_err(|_| OnlineMetadataError::AuthorizationFailed)?;
    Ok(URL_SAFE_NO_PAD.encode(value))
}

fn read_callback(
    stream: &mut std::net::TcpStream,
    expected_state: &str,
    cancelled: &AtomicBool,
    connection_timeout: Duration,
) -> Result<String, OnlineMetadataError> {
    stream
        .set_read_timeout(Some(Duration::from_millis(150)))
        .map_err(|_| OnlineMetadataError::AuthorizationFailed)?;
    let deadline = Instant::now() + connection_timeout;
    let mut buffer = Vec::with_capacity(1024);
    while !buffer.windows(2).any(|line| line == b"\r\n") {
        if cancelled.load(Ordering::Acquire) || Instant::now() >= deadline {
            return Err(OnlineMetadataError::AuthorizationCancelled);
        }
        let mut chunk = [0_u8; 512];
        match stream.read(&mut chunk) {
            Ok(0) => return Err(OnlineMetadataError::AuthorizationFailed),
            Ok(read) => {
                buffer.extend_from_slice(&chunk[..read]);
                if buffer.len() > MAX_REQUEST_BYTES {
                    return Err(OnlineMetadataError::AuthorizationFailed);
                }
            }
            Err(error)
                if error.kind() == std::io::ErrorKind::WouldBlock
                    || error.kind() == std::io::ErrorKind::TimedOut =>
            {
                continue;
            }
            Err(_) => return Err(OnlineMetadataError::AuthorizationFailed),
        }
    }
    let request =
        std::str::from_utf8(&buffer).map_err(|_| OnlineMetadataError::AuthorizationFailed)?;
    parse_callback(request, expected_state)
}

fn parse_callback(request: &str, expected_state: &str) -> Result<String, OnlineMetadataError> {
    let line = request
        .split("\r\n")
        .next()
        .ok_or(OnlineMetadataError::AuthorizationFailed)?;
    let target = line
        .strip_prefix("GET ")
        .and_then(|value| value.strip_suffix(" HTTP/1.1"))
        .ok_or(OnlineMetadataError::AuthorizationFailed)?;
    let (path, query) = target
        .split_once('?')
        .ok_or(OnlineMetadataError::AuthorizationFailed)?;
    if path != "/callback" {
        return Err(OnlineMetadataError::AuthorizationFailed);
    }
    if malformed_percent_encoding(query) {
        return Err(OnlineMetadataError::AuthorizationFailed);
    }
    let values = strict_query_pairs(query)?;
    let states = values
        .iter()
        .filter(|(key, _)| key == "state")
        .collect::<Vec<_>>();
    let codes = values
        .iter()
        .filter(|(key, _)| key == "code")
        .collect::<Vec<_>>();
    if states.len() != 1 || codes.len() != 1 {
        return Err(OnlineMetadataError::AuthorizationFailed);
    }
    let state = states[0].1.as_str();
    if !constant_time_eq(state.as_bytes(), expected_state.as_bytes()) {
        return Err(OnlineMetadataError::AuthorizationFailed);
    }
    let code = codes[0].1.clone();
    (!code.is_empty())
        .then_some(code)
        .ok_or(OnlineMetadataError::AuthorizationFailed)
}

fn malformed_percent_encoding(query: &str) -> bool {
    let bytes = query.as_bytes();
    bytes.iter().enumerate().any(|(index, byte)| {
        *byte == b'%'
            && (index + 2 >= bytes.len()
                || !bytes[index + 1].is_ascii_hexdigit()
                || !bytes[index + 2].is_ascii_hexdigit())
    })
}

fn strict_query_pairs(query: &str) -> Result<Vec<(String, String)>, OnlineMetadataError> {
    query
        .split('&')
        .map(|pair| {
            let (key, value) = pair
                .split_once('=')
                .ok_or(OnlineMetadataError::AuthorizationFailed)?;
            Ok((strict_percent_decode(key)?, strict_percent_decode(value)?))
        })
        .collect()
}

fn strict_percent_decode(value: &str) -> Result<String, OnlineMetadataError> {
    let bytes = value.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'+' => {
                output.push(b' ');
                index += 1;
            }
            b'%' if index + 2 < bytes.len()
                && bytes[index + 1].is_ascii_hexdigit()
                && bytes[index + 2].is_ascii_hexdigit() =>
            {
                let high = (bytes[index + 1] as char).to_digit(16).expect("checked") as u8;
                let low = (bytes[index + 2] as char).to_digit(16).expect("checked") as u8;
                output.push((high << 4) | low);
                index += 3;
            }
            b'%' => return Err(OnlineMetadataError::AuthorizationFailed),
            byte => {
                output.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8(output).map_err(|_| OnlineMetadataError::AuthorizationFailed)
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    let mut difference = left.len() ^ right.len();
    for (a, b) in left.iter().zip(right.iter()) {
        difference |= usize::from(*a ^ *b);
    }
    difference == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pkce_values_and_constant_time_callback_parser_are_strict() {
        let flow = AuthorizationFlow::begin().unwrap();
        assert!(flow.redirect_uri.starts_with("http://127.0.0.1:"));
        assert!(flow.verifier.len() >= 43);
        assert_eq!(flow.challenge.len(), 43);
        assert!(constant_time_eq(b"a", b"a"));
        assert!(!constant_time_eq(b"a", b"b"));
    }

    #[test]
    fn callback_parser_rejects_csrf_duplicate_and_malformed_requests() {
        let valid = "GET /callback?state=expected&code=code HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n";
        assert_eq!(parse_callback(valid, "expected").unwrap(), "code");
        for request in [
            "GET /wrong?state=expected&code=code HTTP/1.1\r\n\r\n",
            "GET /callback?state=wrong&code=code HTTP/1.1\r\n\r\n",
            "GET /callback?state=expected&state=expected&code=code HTTP/1.1\r\n\r\n",
            "GET /callback?state=expected&code=one&code=two HTTP/1.1\r\n\r\n",
            "GET /callback?state=%ZZ&code=code HTTP/1.1\r\n\r\n",
            "GET /callback?state=expected&code=%FF HTTP/1.1\r\n\r\n",
            "GET /callback?state=expected&code=%E2%28%A1 HTTP/1.1\r\n\r\n",
        ] {
            assert!(parse_callback(request, "expected").is_err(), "{request}");
        }
    }

    #[test]
    fn listener_ignores_bad_callback_then_accepts_legitimate_callback() {
        let flow = AuthorizationFlow::begin().unwrap();
        let address = flow.listener.local_addr().unwrap();
        let state = flow.state.clone();
        std::thread::scope(|scope| {
            let waiter = scope.spawn(|| flow.wait_for_code());
            std::thread::sleep(Duration::from_millis(80));
            let mut bad = std::net::TcpStream::connect(address).unwrap();
            bad.write_all(b"GET /callback?state=bad&code=no HTTP/1.1\r\n\r\n")
                .unwrap();
            let mut good = std::net::TcpStream::connect(address).unwrap();
            good.write_all(
                format!("GET /callback?state={state}&code=yes HTTP/1.1\r\n\r\n").as_bytes(),
            )
            .unwrap();
            assert_eq!(waiter.join().unwrap().unwrap(), "yes");
        });
    }

    #[test]
    fn listener_cancel_interrupts_a_stalled_request() {
        let flow = AuthorizationFlow::begin().unwrap();
        let address = flow.listener.local_addr().unwrap();
        let cancel = flow.cancellation_handle();
        std::thread::scope(|scope| {
            let waiter = scope.spawn(|| flow.wait_for_code());
            std::thread::sleep(Duration::from_millis(80));
            let mut stalled = std::net::TcpStream::connect(address).unwrap();
            stalled.write_all(b"GET /callback?").unwrap();
            std::thread::sleep(Duration::from_millis(40));
            cancel.store(true, Ordering::Release);
            assert!(matches!(
                waiter.join().unwrap(),
                Err(OnlineMetadataError::AuthorizationCancelled)
            ));
        });
    }

    #[test]
    fn connection_deadline_does_not_consume_a_later_valid_callback() {
        let flow = AuthorizationFlow::begin().unwrap();
        let address = flow.listener.local_addr().unwrap();
        let state = flow.state.clone();
        std::thread::scope(|scope| {
            let waiter = scope.spawn(|| {
                flow.wait_for_code_with_timeouts(Duration::from_secs(1), Duration::from_millis(80))
            });
            std::thread::sleep(Duration::from_millis(30));
            let mut stalled = std::net::TcpStream::connect(address).unwrap();
            stalled.write_all(b"GET /callback?").unwrap();
            std::thread::sleep(Duration::from_millis(120));
            let mut good = std::net::TcpStream::connect(address).unwrap();
            good.write_all(
                format!("GET /callback?state={state}&code=after-stall HTTP/1.1\r\n\r\n").as_bytes(),
            )
            .unwrap();
            assert_eq!(waiter.join().unwrap().unwrap(), "after-stall");
        });
    }

    #[test]
    fn injected_global_deadline_returns_without_waiting_for_production_timeout() {
        let flow = AuthorizationFlow::begin().unwrap();
        assert!(matches!(
            flow.wait_for_code_with_timeouts(Duration::from_millis(1), Duration::from_millis(1)),
            Err(OnlineMetadataError::AuthorizationCancelled)
        ));
    }
}
