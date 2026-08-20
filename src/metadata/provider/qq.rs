use crate::metadata::{
    merge::verified_search,
    model::{Candidate, Metadata},
};
use reqwest::{blocking::Client, redirect::Policy, StatusCode};
use std::{
    collections::HashMap,
    sync::{Arc, Condvar, Mutex},
    time::Duration,
};
use url::{Host, Url};

const QQ_API_HOST: &str = "u.y.qq.com";
#[derive(Clone)]
pub struct MetadataClient {
    base: String,
    max_body: usize,
    cache: Arc<Mutex<HashMap<String, Vec<u8>>>>,
    inflight: Arc<Mutex<HashMap<String, Arc<Flight>>>>,
    permits: Arc<(Mutex<usize>, Condvar)>,
    #[cfg(test)]
    transport: Transport,
}
#[cfg(test)]
#[derive(Clone, Copy)]
enum Transport {
    Production,
    #[cfg(test)]
    LocalTest,
}
#[derive(Clone, Copy)]
enum RequestKind {
    Provider,
    Cover,
}
struct Flight {
    result: Mutex<Option<std::result::Result<Vec<u8>, String>>>,
    done: Condvar,
}
impl MetadataClient {
    pub fn new(limit: usize) -> Self {
        Self {
            base: format!("https://{QQ_API_HOST}"),
            max_body: 10 * 1024 * 1024,
            cache: Arc::new(Mutex::new(HashMap::new())),
            inflight: Arc::new(Mutex::new(HashMap::new())),
            permits: Arc::new((Mutex::new(limit), Condvar::new())),
            #[cfg(test)]
            transport: Transport::Production,
        }
    }
    #[cfg(test)]
    pub(crate) fn with_test_base(limit: usize, base: String) -> Self {
        Self {
            base,
            max_body: 10 * 1024 * 1024,
            cache: Arc::new(Mutex::new(HashMap::new())),
            inflight: Arc::new(Mutex::new(HashMap::new())),
            permits: Arc::new((Mutex::new(limit), Condvar::new())),
            transport: Transport::LocalTest,
        }
    }
    fn allowed_url(&self, raw: &str, kind: RequestKind) -> std::result::Result<Url, String> {
        let url = Url::parse(raw).map_err(|_| "provider URL is invalid")?;
        #[cfg(test)]
        if matches!(self.transport, Transport::LocalTest) {
            return Ok(url);
        }
        if url.scheme() != "https" {
            return Err("provider URL must use HTTPS".into());
        }
        if !url.username().is_empty() || url.password().is_some() {
            return Err("provider URL must not contain userinfo".into());
        }
        let host = match url.host() {
            Some(Host::Domain(host)) => host.to_ascii_lowercase(),
            Some(Host::Ipv4(_)) | Some(Host::Ipv6(_)) => {
                return Err("provider URL must not use an IP literal".into())
            }
            None => return Err("provider URL is missing a host".into()),
        };
        if host == "localhost" || host.ends_with(".local") {
            return Err("provider URL host is not trusted".into());
        }
        let trusted = match kind {
            RequestKind::Provider => host == QQ_API_HOST,
            RequestKind::Cover => {
                host == "y.gtimg.cn"
                    || host == "imgcache.qq.com"
                    || host == "music.126.net"
                    || host.ends_with(".music.126.net")
            }
        };
        trusted
            .then_some(url)
            .ok_or_else(|| "provider URL host is not trusted".into())
    }
    fn bytes(&self, raw: &str, kind: RequestKind) -> std::result::Result<Vec<u8>, String> {
        // Validate before cache/in-flight access as well as before dispatch: a
        // crafted URL must never become a reusable cache key or a network job.
        let url = self.allowed_url(raw, kind)?;
        let cache_key = url.as_str();
        if let Some(value) = self.cache.lock().unwrap().get(cache_key).cloned() {
            return Ok(value);
        }
        let (flight, leader) = {
            let mut flights = self.inflight.lock().unwrap();
            if let Some(flight) = flights.get(cache_key) {
                (flight.clone(), false)
            } else {
                let flight = Arc::new(Flight {
                    result: Mutex::new(None),
                    done: Condvar::new(),
                });
                flights.insert(cache_key.to_owned(), flight.clone());
                (flight, true)
            }
        };
        if !leader {
            let mut result = flight.result.lock().unwrap();
            while result.is_none() {
                result = flight.done.wait(result).unwrap()
            }
            return result.clone().expect("set before notify");
        }
        let (lock, cv) = &*self.permits;
        let mut permits = lock.lock().unwrap();
        while *permits == 0 {
            permits = cv.wait(permits).unwrap()
        }
        *permits -= 1;
        drop(permits);
        let result = (|| {
            // This is invoked only from the bounded blocking preflight worker.
            // Constructing it here avoids dropping reqwest's runtime in Tokio's
            // async scheduler when a batch completes without metadata work.
            let client = Client::builder()
                .redirect(Policy::none())
                .connect_timeout(Duration::from_secs(3))
                .timeout(Duration::from_secs(8))
                .build()
                .map_err(|e| e.to_string())?;
            let response = client.get(url.clone()).send().map_err(|e| e.to_string())?;
            if response.status() != StatusCode::OK {
                return Err(format!("provider status {}", response.status()));
            }
            if response
                .content_length()
                .is_some_and(|n| n as usize > self.max_body)
            {
                return Err("provider body too large".into());
            }
            let body = response.bytes().map_err(|e| e.to_string())?.to_vec();
            if body.len() > self.max_body {
                return Err("provider body too large".into());
            }
            Ok(body)
        })();
        let mut permits = lock.lock().unwrap();
        *permits += 1;
        cv.notify_one();
        if let Ok(body) = &result {
            self.cache
                .lock()
                .unwrap()
                .insert(cache_key.into(), body.clone());
        }
        *flight.result.lock().unwrap() = Some(result.clone());
        flight.done.notify_all();
        self.inflight.lock().unwrap().remove(cache_key);
        result
    }
    pub fn cover(&self, url: &str) -> std::result::Result<Vec<u8>, String> {
        let body = self.bytes(url, RequestKind::Cover)?;
        crate::metadata::tags::validate_cover(&body)?;
        Ok(body)
    }
    pub fn exact(&self, id: &str) -> std::result::Result<Metadata, String> {
        let body = self.bytes(
            &format!("{}/exact/{}", self.base, id),
            RequestKind::Provider,
        )?;
        serde_json::from_slice(&body).map_err(|e| e.to_string())
    }
    pub fn search_verified(
        &self,
        title: &str,
        artist: &str,
        duration: Option<u32>,
    ) -> std::result::Result<Option<Metadata>, String> {
        let body = self.bytes(
            &format!("{}/search?q={}", self.base, title),
            RequestKind::Provider,
        )?;
        let candidates: Vec<Candidate> =
            serde_json::from_slice(&body).map_err(|e| e.to_string())?;
        Ok(verified_search(title, artist, duration, &candidates))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
    };
    fn server(response: &'static str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut request = [0; 1024];
                let _ = stream.read(&mut request);
                stream.write_all(response.as_bytes()).unwrap();
            }
        });
        format!("http://{address}")
    }
    #[test]
    fn exact_response_is_cached_and_redirects_are_not_followed() {
        let base =
            server("HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n{\"title\":null,\"album\":null}");
        let client = std::sync::Arc::new(MetadataClient::with_test_base(1, base));
        let other = client.clone();
        let worker = thread::spawn(move || other.exact("1").unwrap());
        client.exact("1").unwrap();
        worker.join().unwrap();
        client.exact("1").unwrap();
        let redirect =
            server("HTTP/1.1 302 Found\r\nLocation: /other\r\nContent-Length: 0\r\n\r\n");
        assert!(MetadataClient::with_test_base(1, redirect)
            .exact("1")
            .is_err());
    }
    #[test]
    fn production_url_policy_accepts_only_trusted_https_origins() {
        let client = MetadataClient::new(1);
        for url in [
            "https://y.gtimg.cn/music/photo.jpg",
            "https://imgcache.qq.com/music/photo.jpg",
            "https://p1.music.126.net/photo.jpg",
        ] {
            assert!(client.allowed_url(url, RequestKind::Cover).is_ok(), "{url}");
        }
        for url in [
            "http://y.gtimg.cn/photo.jpg",
            "https://127.0.0.1/photo.jpg",
            "https://localhost/photo.jpg",
            "https://cover.local/photo.jpg",
            "https://evil.example/photo.jpg",
            "https://user@y.gtimg.cn/photo.jpg",
        ] {
            assert!(
                client.allowed_url(url, RequestKind::Cover).is_err(),
                "{url}"
            );
        }
        assert!(client
            .allowed_url("https://u.y.qq.com/exact/1", RequestKind::Provider)
            .is_ok());
        assert!(client
            .allowed_url("https://y.gtimg.cn/exact/1", RequestKind::Provider)
            .is_err());
    }
}
