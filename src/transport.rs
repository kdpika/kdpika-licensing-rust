use crate::{
    error::{Error, ErrorCode, Result},
    json,
};
use reqwest::{blocking::Client, redirect::Policy, Method};
use std::{collections::BTreeMap, io::Read, time::Duration};
use url::Url;

/// Request for a trusted application-owned transport adapter. Debug is deliberately redacted.
pub struct Request {
    pub method: &'static str,
    pub path: &'static str,
    pub headers: BTreeMap<String, String>,
    pub body: Option<Vec<u8>>,
}
/// Raw response bytes are retained so PHP-signed numeric spellings survive parsing.
pub struct HttpResponse {
    pub status: u16,
    pub body: Vec<u8>,
    pub retry_after: Option<String>,
}
/// A network failure (no authoritative HTTP response). Never classify HTTP errors as network loss.
#[derive(Clone, Copy, Debug)]
pub struct NetworkError;
/// Advanced trusted injection; an adapter must preserve bytes and distinguish HTTP refusals.
pub trait Transport: Send + Sync {
    fn request(&self, request: Request) -> std::result::Result<HttpResponse, NetworkError>;
}
pub(crate) struct HttpTransport {
    client: Client,
    origin: String,
}
impl HttpTransport {
    pub fn new(origin: &str, timeout: Duration) -> Result<Self> {
        let client = Client::builder()
            .timeout(timeout)
            .connect_timeout(timeout.min(Duration::from_secs(10)))
            .redirect(Policy::none())
            .retry(reqwest::retry::never())
            .referer(false)
            .no_proxy()
            .user_agent(concat!("KDPika-Licensing-Rust/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|_| Error(ErrorCode::Configuration))?;
        Ok(Self {
            client,
            origin: origin.to_string(),
        })
    }
}
impl Transport for HttpTransport {
    fn request(&self, request: Request) -> std::result::Result<HttpResponse, NetworkError> {
        let method = Method::from_bytes(request.method.as_bytes()).map_err(|_| NetworkError)?;
        let mut req = self
            .client
            .request(method, self.origin.clone() + request.path)
            .header("Accept", "application/json");
        for (k, v) in request.headers {
            req = req.header(k, v);
        }
        if let Some(body) = request.body {
            req = req.header("Content-Type", "application/json").body(body);
        }
        let response = req.send().map_err(|_| NetworkError)?;
        let status = response.status().as_u16();
        let retry_after = response
            .headers()
            .get("Retry-After")
            .and_then(|v| v.to_str().ok())
            .filter(|v| v.len() <= 128)
            .map(String::from);
        let mut body = Vec::new();
        match response
            .take((json::MAX_BYTES + 1) as u64)
            .read_to_end(&mut body)
        {
            Ok(_) => {}
            Err(_) => {
                // An HTTP response was received: truncated body must deny, not grant old cache.
                return Ok(HttpResponse {
                    status,
                    body: Vec::new(),
                    retry_after,
                });
            }
        }
        Ok(HttpResponse {
            status,
            body,
            retry_after,
        })
    }
}
pub(crate) fn safe_origin(value: &str, allow_loopback: bool) -> Result<String> {
    let url = Url::parse(value).map_err(|_| Error(ErrorCode::Configuration))?;
    let local = allow_loopback
        && url.scheme() == "http"
        && matches!(url.host(),Some(url::Host::Ipv4(ip))if ip.is_loopback())
        || allow_loopback
            && url.scheme() == "http"
            && matches!(url.host_str(), Some("localhost" | "[::1]"));
    if url.host_str().is_none()
        || (!local && url.scheme() != "https")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.path() != "/"
        || value.chars().any(|c| c.is_control() || c == '\\')
    {
        return Err(Error(ErrorCode::Configuration));
    }
    Ok(url.origin().ascii_serialization())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{io::Write, net::TcpListener, thread};

    fn exchange(response: Vec<u8>) -> HttpResponse {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut input = [0; 4096];
            assert!(stream.read(&mut input).unwrap() > 0);
            let _ = stream.write_all(&response);
            // A redirect/retry would require another connection, which is never accepted.
        });
        let result = HttpTransport::new(&origin, Duration::from_secs(2))
            .unwrap()
            .request(Request {
                method: "POST",
                path: "/api/v1/licenses/activate",
                headers: BTreeMap::new(),
                body: Some(b"{}".to_vec()),
            })
            .unwrap();
        server.join().unwrap();
        result
    }

    #[test]
    fn redirects_are_returned_without_following() {
        let result = exchange(b"HTTP/1.1 307 Temporary Redirect\r\nLocation: http://127.0.0.1:1/secret\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}".to_vec());
        assert_eq!(result.status, 307);
        assert_eq!(result.body, b"{}");
    }

    #[test]
    fn truncated_http_body_is_not_network_fallback() {
        let result = exchange(
            b"HTTP/1.1 503 Unavailable\r\nContent-Length: 9999\r\nConnection: close\r\n\r\n{}"
                .to_vec(),
        );
        assert_eq!(result.status, 503);
        assert!(result.body.is_empty());
    }

    #[test]
    fn response_read_is_bounded() {
        let size = json::MAX_BYTES + 4096;
        let mut response =
            format!("HTTP/1.1 200 OK\r\nContent-Length: {size}\r\nConnection: close\r\n\r\n")
                .into_bytes();
        response.extend(vec![b' '; size]);
        assert_eq!(exchange(response).body.len(), json::MAX_BYTES + 1);
    }
}
