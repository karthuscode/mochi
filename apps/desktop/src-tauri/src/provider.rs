//! Fixed-endpoint HTTPS adapter. Keys, requests and raw response bodies are never logged.
use crate::credentials::Secret;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderError {
    Cancelled,
    Authentication,
    Network,
    RateLimit,
    Rejected,
    TooLarge,
    Unavailable,
}
impl ProviderError {
    pub fn message(self) -> &'static str {
        match self {
            Self::Cancelled => "Analysis cancelled or permission changed.",
            Self::Authentication => {
                "OpenAI rejected the API key or its permissions. Check Keychain settings."
            }
            Self::Network => {
                "OpenAI is unavailable or the request timed out. You can review and retry."
            }
            Self::RateLimit => {
                "OpenAI rate limit or API quota reached. Check your API account and retry later."
            }
            Self::Rejected => "OpenAI rejected this request. No learning result was published.",
            Self::TooLarge => "The provider response exceeded safe bounds.",
            Self::Unavailable => "The analysis adapter is unavailable.",
        }
    }
}
pub trait AnalysisProvider: Send + Sync {
    fn send(
        &self,
        request: &str,
        key: &Secret,
        cancel: &AtomicBool,
        current: &(dyn Fn() -> bool + Sync),
    ) -> Result<Vec<u8>, ProviderError>;
}
pub struct OpenAiProvider;
impl AnalysisProvider for OpenAiProvider {
    fn send(
        &self,
        request: &str,
        key: &Secret,
        cancel: &AtomicBool,
        current: &(dyn Fn() -> bool + Sync),
    ) -> Result<Vec<u8>, ProviderError> {
        if request.len() > 128 * 1024 {
            return Err(ProviderError::TooLarge);
        }
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| ProviderError::Unavailable)?;
        runtime.block_on(async {
            let client = reqwest::Client::builder()
                .https_only(true)
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .retry(reqwest::retry::never())
                .connect_timeout(Duration::from_secs(10))
                .timeout(Duration::from_secs(60))
                .build()
                .map_err(|_| ProviderError::Unavailable)?;
            let mut last = ProviderError::Network;
            for attempt in 0..3 {
                if cancel.load(Ordering::Acquire) || !current() {
                    return Err(ProviderError::Cancelled);
                }
                if attempt > 0 {
                    let duration = if attempt == 1 { 2 } else { 8 };
                    for _ in 0..duration * 10 {
                        if cancel.load(Ordering::Acquire) {
                            return Err(ProviderError::Cancelled);
                        }
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                    if !current() {
                        return Err(ProviderError::Cancelled);
                    }
                }
                let send = client
                    .post(mochi_learning::ENDPOINT)
                    .header(reqwest::header::CONTENT_TYPE, "application/json")
                    .bearer_auth(key.text().map_err(|_| ProviderError::Authentication)?)
                    .body(request.to_owned())
                    .send();
                let response = cancelable(send, cancel).await;
                let mut response = match response {
                    Err(ProviderError::Cancelled) => return Err(ProviderError::Cancelled),
                    Err(_) => {
                        last = ProviderError::Network;
                        continue;
                    }
                    Ok(r) => r,
                };
                let status = response.status().as_u16();
                match status {
                    401 | 403 => return Err(ProviderError::Authentication),
                    429 => {
                        let mut metadata = Vec::new();
                        loop {
                            let Some(chunk) = cancelable(response.chunk(), cancel).await? else {
                                break;
                            };
                            if metadata.len() + chunk.len() > 32 * 1024 {
                                return Err(ProviderError::RateLimit);
                            }
                            metadata.extend_from_slice(&chunk);
                        }
                        if !retryable_rate_limit(&metadata) {
                            return Err(ProviderError::RateLimit);
                        }
                        last = ProviderError::RateLimit;
                        continue;
                    }
                    500..=599 => {
                        last = ProviderError::Network;
                        continue;
                    }
                    200..=299 => {}
                    _ => return Err(ProviderError::Rejected),
                }
                if response.content_length().is_some_and(|n| n > 512 * 1024) {
                    return Err(ProviderError::TooLarge);
                }
                let mut bytes = Vec::new();
                loop {
                    let chunk = cancelable(response.chunk(), cancel).await?;
                    let Some(chunk) = chunk else { break };
                    if bytes.len() + chunk.len() > 512 * 1024 {
                        return Err(ProviderError::TooLarge);
                    }
                    bytes.extend_from_slice(&chunk);
                }
                if !current() || cancel.load(Ordering::Acquire) {
                    return Err(ProviderError::Cancelled);
                }
                return Ok(bytes);
            }
            Err(last)
        })
    }
}
async fn cancelable<T>(
    future: impl std::future::Future<Output = Result<T, reqwest::Error>>,
    cancel: &AtomicBool,
) -> Result<T, ProviderError> {
    tokio::pin!(future);
    loop {
        tokio::select! {result=&mut future=>return result.map_err(|_|ProviderError::Network),_ = tokio::time::sleep(Duration::from_millis(100))=>{if cancel.load(Ordering::Acquire){return Err(ProviderError::Cancelled)}}}
    }
}

fn retryable_rate_limit(bytes: &[u8]) -> bool {
    let Ok(value) = mochi_privacy::parse_bounded_json(bytes, 32 * 1024) else {
        return false;
    };
    // Only documented transient metadata permits a retry. Quota/authentication
    // failures and unknown bodies are never retried or surfaced as provider text.
    value.get("error").is_some_and(|error| {
        error.get("code").and_then(|v| v.as_str()) == Some("rate_limit_exceeded")
            && error.get("type").and_then(|v| v.as_str()) != Some("insufficient_quota")
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quota_unknown_and_malformed_errors_never_retry() {
        assert!(retryable_rate_limit(
            br#"{"error":{"code":"rate_limit_exceeded","type":"tokens"}}"#
        ));
        for bytes in [
            br#"{"error":{"code":"insufficient_quota"}}"#.as_slice(),
            br#"{"error":{"message":"synthetic private text"}}"#.as_slice(),
            br#"{"error":{"code":"rate_limit_exceeded","type":"insufficient_quota"}}"#.as_slice(),
            b"malformed",
        ] {
            assert!(!retryable_rate_limit(bytes));
        }
    }
}
