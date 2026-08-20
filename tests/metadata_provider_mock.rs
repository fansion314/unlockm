use std::{
    net::TcpListener,
    thread,
    time::{Duration, Instant},
};
use unlockm::metadata::provider::qq::MetadataClient;

fn rejected_without_dispatch(url: &str) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    let local = format!("http://{address}/cover");
    let client = MetadataClient::new(1);
    assert!(client
        .cover(if url == "$LOCAL" { &local } else { url })
        .is_err());
    let until = Instant::now() + Duration::from_millis(150);
    while Instant::now() < until {
        match listener.accept() {
            Ok(_) => panic!("rejected cover URL dispatched a request"),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(5))
            }
            Err(error) => panic!("probe listener failed: {error}"),
        }
    }
}

#[test]
fn production_cover_policy_rejects_malicious_ncm_and_qmc_urls_without_dispatch() {
    // Container (NCM) and provider (QMC) cover URLs both meet the same
    // dispatch boundary; provenance must not grant local-network access.
    for url in [
        "$LOCAL",
        "http://127.0.0.1/cover",
        "https://127.0.0.1/cover",
        "https://[::1]/cover",
        "https://localhost/cover",
        "https://cover.local/cover",
        "https://evil.example/cover",
        "https://user@y.gtimg.cn/cover",
    ] {
        rejected_without_dispatch(url);
    }
}
