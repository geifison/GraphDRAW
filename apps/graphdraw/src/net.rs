//! HTTP for the services TraceDraw talks to (AI Drawing, GitHub): one
//! request at a time, on a thread on the desktop and as a `fetch` in a
//! browser, its answer delivered on a channel.

/// One HTTP request.
#[derive(Debug, Clone, PartialEq)]
pub struct HttpRequest {
    /// `GET`, `POST` or `PUT`.
    pub method: &'static str,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
    /// A binary body (multipart uploads); sent instead of `body` when
    /// not empty.
    pub bytes: Vec<u8>,
}

/// What the service answered.
#[derive(Debug, Clone, PartialEq)]
pub struct HttpResponse {
    pub status: u16,
    pub body: String,
}

/// A response, or why none came (network, address, certificate).
pub type HttpResult = Result<HttpResponse, String>;

/// Send `req` and deliver the answer (at most `limit` bytes of body) on
/// `tx`: on a thread on the desktop, as a fetch in a browser.
#[cfg(not(target_arch = "wasm32"))]
pub fn spawn(req: HttpRequest, limit: u64, tx: std::sync::mpsc::Sender<HttpResult>) {
    std::thread::spawn(move || {
        let _ = tx.send(send(&req, limit));
    });
}

#[cfg(target_arch = "wasm32")]
pub fn spawn(req: HttpRequest, limit: u64, tx: std::sync::mpsc::Sender<HttpResult>) {
    let _ = limit;
    wasm_bindgen_futures::spawn_local(async move {
        let _ = tx.send(send(req).await);
        crate::web::request_repaint();
    });
}

/// One request over HTTPS (or HTTP for local servers), waiting up to
/// five minutes. Certificates are checked against the system's trusted
/// ones; proxies come from the usual environment variables.
#[cfg(not(target_arch = "wasm32"))]
pub fn send(req: &HttpRequest, limit: u64) -> HttpResult {
    use ureq::tls::{RootCerts, TlsConfig};
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(300)))
        .http_status_as_error(false)
        .tls_config(
            TlsConfig::builder()
                .root_certs(RootCerts::PlatformVerifier)
                .build(),
        )
        .build()
        .into();
    let mut resp = match req.method {
        "GET" => {
            let mut r = agent.get(&req.url);
            for (name, value) in &req.headers {
                r = r.header(name, value);
            }
            r.call()
        }
        "PUT" => {
            let mut r = agent.put(&req.url);
            for (name, value) in &req.headers {
                r = r.header(name, value);
            }
            r.send(req.body.as_str())
        }
        _ => {
            let mut r = agent.post(&req.url);
            for (name, value) in &req.headers {
                r = r.header(name, value);
            }
            if req.bytes.is_empty() {
                r.send(req.body.as_str())
            } else {
                r.send(&req.bytes[..])
            }
        }
    }
    .map_err(|e| e.to_string())?;
    let status = resp.status().as_u16();
    let body = resp
        .body_mut()
        .with_config()
        .limit(limit)
        .read_to_string()
        .map_err(|e| e.to_string())?;
    Ok(HttpResponse { status, body })
}

#[cfg(target_arch = "wasm32")]
pub async fn send(req: HttpRequest) -> HttpResult {
    use wasm_bindgen::JsCast;
    let js = |e: wasm_bindgen::JsValue| {
        e.as_string()
            .or_else(|| {
                e.dyn_ref::<js_sys::Error>()
                    .map(|e| String::from(e.message()))
            })
            .unwrap_or_else(|| "the request failed".to_string())
    };
    let init = web_sys::RequestInit::new();
    init.set_method(req.method);
    init.set_mode(web_sys::RequestMode::Cors);
    if req.method != "GET" {
        if req.bytes.is_empty() {
            init.set_body(&wasm_bindgen::JsValue::from_str(&req.body));
        } else {
            let array = js_sys::Uint8Array::from(&req.bytes[..]);
            init.set_body(&array);
        }
    }
    let headers = web_sys::Headers::new().map_err(js)?;
    for (name, value) in &req.headers {
        headers.set(name, value).map_err(js)?;
    }
    init.set_headers(headers.as_ref());
    let request = web_sys::Request::new_with_str_and_init(&req.url, &init).map_err(js)?;
    let window = web_sys::window().ok_or("no browser window")?;
    let resp = wasm_bindgen_futures::JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(js)?;
    let resp: web_sys::Response = resp.dyn_into().map_err(js)?;
    let status = resp.status();
    let text = wasm_bindgen_futures::JsFuture::from(resp.text().map_err(js)?)
        .await
        .map_err(js)?;
    Ok(HttpResponse {
        status,
        body: text.as_string().unwrap_or_default(),
    })
}
