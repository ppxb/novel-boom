//! Shared HTTP client built from application config.

use std::time::Duration;

use reqwest::header::{
    ACCEPT, ACCEPT_LANGUAGE, CACHE_CONTROL, CONNECTION, COOKIE, HeaderMap, HeaderName, HeaderValue,
    ORIGIN, REFERER, USER_AGENT,
};
use reqwest::{Client, Method, Proxy};
use tokio::time::sleep;

use crate::config::Config;
use crate::error::{Error, Result};

use super::ua;

const DEFAULT_TIMEOUT_SECS: u64 = 20;
const CONNECT_TIMEOUT_SECS: u64 = 12;
/// Extra attempts after the first try for transient network failures.
const DEFAULT_RETRIES: u32 = 2;
const RETRY_BASE_DELAY_MS: u64 = 400;

/// Thin wrapper around [`reqwest::Client`] with project defaults.
#[derive(Debug, Clone)]
pub struct HttpClient {
    inner: Client,
    retries: u32,
}

impl HttpClient {
    /// Build a client from TOML config (proxy, redirects, compression).
    pub fn from_config(config: &Config) -> Result<Self> {
        let mut builder = Client::builder()
            // Many novel sites have fragile stacks; HTTP/1.1 is more compatible.
            .http1_only()
            .connect_timeout(Duration::from_secs(CONNECT_TIMEOUT_SECS))
            .timeout(Duration::from_secs(DEFAULT_TIMEOUT_SECS))
            .pool_idle_timeout(Duration::from_secs(30))
            .tcp_keepalive(Duration::from_secs(30))
            .tcp_nodelay(true)
            .redirect(reqwest::redirect::Policy::limited(10))
            .gzip(true)
            .brotli(true)
            .deflate(true)
            .user_agent(ua::random_ua());

        if config.proxy.enabled {
            let proxy_url = format!(
                "http://{}:{}",
                config.proxy.host.trim(),
                config.proxy.port
            );
            let proxy = Proxy::all(&proxy_url).map_err(|err| {
                Error::Message(format!("代理地址无效 `{proxy_url}`: {err}"))
            })?;
            builder = builder.proxy(proxy);
            tracing::info!(%proxy_url, "HTTP 代理已启用");
        } else {
            // Avoid inheriting system proxy accidentally when user disabled it.
            builder = builder.no_proxy();
        }

        let inner = builder
            .build()
            .map_err(|err| Error::Http(format!("创建 HTTP 客户端失败: {err}")))?;
        Ok(Self {
            inner,
            retries: DEFAULT_RETRIES,
        })
    }

    /// GET and return response body text.
    pub async fn get_text(
        &self,
        url: &str,
        timeout_secs: u64,
        cookies: Option<&str>,
    ) -> Result<String> {
        self.request_text(Method::GET, url, timeout_secs, cookies, None)
            .await
    }

    /// POST `application/x-www-form-urlencoded` and return body text.
    pub async fn post_form(
        &self,
        url: &str,
        timeout_secs: u64,
        cookies: Option<&str>,
        form: &[(String, String)],
    ) -> Result<String> {
        self.request_text(Method::POST, url, timeout_secs, cookies, Some(form))
            .await
    }

    async fn request_text(
        &self,
        method: Method,
        url: &str,
        timeout_secs: u64,
        cookies: Option<&str>,
        form: Option<&[(String, String)]>,
    ) -> Result<String> {
        let attempts = self.retries + 1;
        let mut last_err = None;

        for attempt in 1..=attempts {
            match self
                .request_text_once(method.clone(), url, timeout_secs, cookies, form)
                .await
            {
                Ok(body) => return Ok(body),
                Err(err) => {
                    let retryable = is_retryable(&err);
                    tracing::warn!(
                        %url,
                        attempt,
                        attempts,
                        retryable,
                        error = %err,
                        "HTTP 请求失败"
                    );
                    last_err = Some(err);
                    if !retryable || attempt == attempts {
                        break;
                    }
                    let delay = RETRY_BASE_DELAY_MS * u64::from(attempt);
                    sleep(Duration::from_millis(delay)).await;
                }
            }
        }

        Err(last_err.unwrap_or_else(|| Error::Http("未知 HTTP 错误".into())))
    }

    async fn request_text_once(
        &self,
        method: Method,
        url: &str,
        timeout_secs: u64,
        cookies: Option<&str>,
        form: Option<&[(String, String)]>,
    ) -> Result<String> {
        let headers = build_headers(url, method == Method::POST, cookies)?;
        let timeout = Duration::from_secs(timeout_secs.max(1));

        let mut request = self
            .inner
            .request(method, url)
            .headers(headers)
            .timeout(timeout);

        if let Some(form) = form {
            request = request.form(form);
        }

        let response = request.send().await.map_err(|err| map_reqwest_error(err, url))?;
        let status = response.status();
        let final_url = response.url().clone();
        let body = response
            .text()
            .await
            .map_err(|err| map_reqwest_error(err, url))?;

        if !status.is_success() {
            return Err(Error::Http(format!(
                "HTTP {status} · {final_url}\n提示：站点可能限流、维护或拒绝访问，可稍后重试或换书源"
            )));
        }

        if body.trim().is_empty() {
            return Err(Error::Http(format!(
                "响应为空 · {final_url}\n提示：可能被反爬或搜索限流"
            )));
        }

        Ok(body)
    }
}

fn build_headers(url: &str, is_post: bool, cookies: Option<&str>) -> Result<HeaderMap> {
    let mut headers = HeaderMap::new();
    headers.insert(
        ACCEPT,
        HeaderValue::from_static(
            "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8",
        ),
    );
    headers.insert(
        ACCEPT_LANGUAGE,
        HeaderValue::from_static("zh-CN,zh;q=0.9,en;q=0.8"),
    );
    headers.insert(CONNECTION, HeaderValue::from_static("keep-alive"));
    headers.insert(CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    // Mimic a normal browser upgrade header without enabling HTTP/2 on the client.
    headers.insert(
        HeaderName::from_static("upgrade-insecure-requests"),
        HeaderValue::from_static("1"),
    );
    headers.insert(
        HeaderName::from_static("sec-fetch-dest"),
        HeaderValue::from_static("document"),
    );
    headers.insert(
        HeaderName::from_static("sec-fetch-mode"),
        HeaderValue::from_static("navigate"),
    );
    headers.insert(
        HeaderName::from_static("sec-fetch-site"),
        HeaderValue::from_static(if is_post { "same-origin" } else { "none" }),
    );
    headers.insert(
        HeaderName::from_static("sec-ch-ua"),
        HeaderValue::from_static("\"Chromium\";v=\"131\", \"Not_A Brand\";v=\"24\""),
    );
    headers.insert(
        HeaderName::from_static("sec-ch-ua-mobile"),
        HeaderValue::from_static("?0"),
    );
    headers.insert(
        HeaderName::from_static("sec-ch-ua-platform"),
        HeaderValue::from_static("\"Windows\""),
    );

    // Avoid from_static for rotating UA.
    let ua = ua::random_ua();
    headers.insert(
        USER_AGENT,
        HeaderValue::from_str(ua).map_err(|err| Error::Http(format!("非法 User-Agent: {err}")))?,
    );

    if let Ok(parsed) = url::Url::parse(url) {
        let origin = format!(
            "{}://{}",
            parsed.scheme(),
            parsed.host_str().unwrap_or_default()
        );
        if let Ok(value) = HeaderValue::from_str(&origin) {
            headers.insert(REFERER, value.clone());
            if is_post {
                headers.insert(ORIGIN, value);
            }
        }
    }

    if let Some(cookie) = cookies.map(str::trim).filter(|c| !c.is_empty()) {
        // Cookie values in rules may use single quotes; keep as-is like so-novel.
        if let Ok(value) = HeaderValue::from_str(cookie) {
            headers.insert(COOKIE, value);
        } else {
            tracing::warn!(cookie, "Cookie 头无法编码，已忽略");
        }
    }

    Ok(headers)
}

fn map_reqwest_error(err: reqwest::Error, url: &str) -> Error {
    let is_timeout = err.is_timeout();
    let is_connect = err.is_connect();
    let is_request = err.is_request();
    let detail = error_chain(err);

    // Generic "error sending request" often wraps connect/timeout at lower layers.
    let looks_unreachable = detail_suggests_unreachable(&detail);

    if is_timeout || (looks_unreachable && detail.to_ascii_lowercase().contains("timed out")) {
        return Error::Http(format!(
            "请求超时 · {url}\n\
             原因：{detail}\n\
             提示：站点可能不可达、被墙、限流或暂时维护。可稍后重试、换书源，或在 config.toml 启用 [proxy]"
        ));
    }

    if is_connect || looks_unreachable {
        return Error::Http(format!(
            "无法连接 · {url}\n\
             原因：{detail}\n\
             提示：检查网络/DNS/防火墙；部分书源对 IP 有要求（见 so-novel 书源说明），可尝试代理或更换书源"
        ));
    }

    if is_request {
        return Error::Http(format!(
            "请求发送失败 · {url}\n\
             原因：{detail}\n\
             提示：可能是 TLS/协议不兼容或网络中断，程序会自动重试；仍失败请换书源或开代理"
        ));
    }

    Error::Http(format!("HTTP 失败 · {url}\n原因：{detail}"))
}

fn error_chain(err: reqwest::Error) -> String {
    let mut parts = Vec::new();
    let mut current: &dyn std::error::Error = &err;
    loop {
        parts.push(current.to_string());
        match std::error::Error::source(current) {
            Some(next) => current = next,
            None => break,
        }
    }
    // Drop redundant URL noise from the top-level reqwest message when present.
    if let Some(first) = parts.first_mut() {
        if let Some(idx) = first.find(" for url ") {
            *first = first[..idx].to_string();
        }
    }
    parts.join(" ← ")
}

fn detail_suggests_unreachable(detail: &str) -> bool {
    let d = detail.to_ascii_lowercase();
    d.contains("error sending request")
        || d.contains("connection refused")
        || d.contains("connection reset")
        || d.contains("network is unreachable")
        || d.contains("no route to host")
        || d.contains("name or service not known")
        || d.contains("failed to lookup")
        || d.contains("dns")
        || d.contains("timed out")
        || d.contains("timeout")
        || d.contains("tls handshake")
        || d.contains("certificate")
}

fn is_retryable(err: &Error) -> bool {
    match err {
        Error::Http(msg) => {
            let m = msg.to_ascii_lowercase();
            m.contains("超时")
                || m.contains("timeout")
                || m.contains("无法连接")
                || m.contains("connect")
                || m.contains("发送失败")
                || m.contains("connection reset")
                || m.contains("connection refused")
                || m.contains("dns")
                || m.contains("tls")
                || m.contains("ssl")
                || m.contains("broken pipe")
                || m.contains("end of file")
                || m.contains("error sending request")
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_timeout_style_message() {
        // Ensure helper keeps Chinese guidance for user-facing paths.
        let err = Error::Http(
            "请求超时 · http://example.com\n原因：timeout\n提示：x".into(),
        );
        assert!(is_retryable(&err));
    }
}
