use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde_json::Value;

// Public client id used by GitHub Copilot editor integrations for the device flow.
const CLIENT_ID: &str = "Iv1.b507a08c87ecfe98";
const DEVICE_CODE_URL: &str = "https://github.com/login/device/code";
const ACCESS_TOKEN_URL: &str = "https://github.com/login/oauth/access_token";
const COPILOT_TOKEN_URL: &str = "https://api.github.com/copilot_internal/v2/token";
const CHAT_URL: &str = "https://api.githubcopilot.com/chat/completions";

const EDITOR_VERSION: &str = "vscode/1.95.0";
const PLUGIN_VERSION: &str = "copilot-chat/0.22.0";
const USER_AGENT: &str = "GitHubCopilotChat/0.22.0";
const INTEGRATION_ID: &str = "vscode-chat";

/// Details returned when starting the GitHub device-login flow.
#[derive(Serialize, Clone)]
pub struct DeviceInfo {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub interval: u64,
}

/// Cached short-lived Copilot API token derived from the GitHub OAuth token.
struct CachedToken {
    token: String,
    expires_at: i64,
}

#[derive(Default)]
pub struct CopilotState {
    cached: Mutex<Option<CachedToken>>,
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Begin the device authorization flow and return the code the user must enter.
pub async fn start_device_flow() -> Result<DeviceInfo, String> {
    let client = reqwest::Client::new();
    let res = client
        .post(DEVICE_CODE_URL)
        .header("Accept", "application/json")
        .form(&[("client_id", CLIENT_ID), ("scope", "read:user")])
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let status = res.status();
    let json: Value = res.json().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(format!("GitHub device flow failed ({status})"));
    }

    Ok(DeviceInfo {
        device_code: json["device_code"].as_str().unwrap_or_default().to_string(),
        user_code: json["user_code"].as_str().unwrap_or_default().to_string(),
        verification_uri: json["verification_uri"]
            .as_str()
            .unwrap_or("https://github.com/login/device")
            .to_string(),
        interval: json["interval"].as_u64().unwrap_or(5),
    })
}

/// Poll for the OAuth token. Returns `Ok(None)` while the user hasn't finished yet.
pub async fn poll_access_token(device_code: &str) -> Result<Option<String>, String> {
    let client = reqwest::Client::new();
    let res = client
        .post(ACCESS_TOKEN_URL)
        .header("Accept", "application/json")
        .form(&[
            ("client_id", CLIENT_ID),
            ("device_code", device_code),
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
        ])
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let json: Value = res.json().await.map_err(|e| e.to_string())?;

    if let Some(token) = json["access_token"].as_str() {
        return Ok(Some(token.to_string()));
    }

    match json["error"].as_str() {
        Some("authorization_pending") | Some("slow_down") => Ok(None),
        Some("expired_token") => Err("Login expired, please try again.".into()),
        Some("access_denied") => Err("Login was denied.".into()),
        Some(other) => Err(format!("GitHub login error: {other}")),
        None => Err("Unexpected response from GitHub.".into()),
    }
}

/// Exchange the GitHub OAuth token for a short-lived Copilot API token, cached until expiry.
async fn copilot_token(state: &CopilotState, oauth_token: &str) -> Result<String, String> {
    {
        let guard = state.cached.lock().map_err(|e| e.to_string())?;
        if let Some(cached) = guard.as_ref() {
            if cached.expires_at - 60 > now() {
                return Ok(cached.token.clone());
            }
        }
    }

    let client = reqwest::Client::new();
    let res = client
        .get(COPILOT_TOKEN_URL)
        .header("Authorization", format!("token {oauth_token}"))
        .header("User-Agent", USER_AGENT)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let status = res.status();
    let json: Value = res.json().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(format!(
            "Could not get Copilot token ({status}). Make sure your GitHub account has Copilot access."
        ));
    }

    let token = json["token"]
        .as_str()
        .ok_or("No Copilot token in response")?
        .to_string();
    let expires_at = json["expires_at"].as_i64().unwrap_or(now() + 600);

    if let Ok(mut guard) = state.cached.lock() {
        *guard = Some(CachedToken {
            token: token.clone(),
            expires_at,
        });
    }

    Ok(token)
}

/// Send a chat completion request through the Copilot API and return the reply
/// text. The model is given a `fetch_url` tool so it can pull in live web
/// content (for example the user's portfolio) when it needs current facts.
pub async fn chat(
    state: &CopilotState,
    oauth_token: &str,
    model: &str,
    messages: Value,
) -> Result<String, String> {
    let token = copilot_token(state, oauth_token).await?;
    let client = reqwest::Client::new();

    let mut conversation: Vec<Value> = messages.as_array().cloned().unwrap_or_default();

    let tools = serde_json::json!([{
        "type": "function",
        "function": {
            "name": "fetch_url",
            "description": "Fetch the readable text of a public web page. Use this whenever you need up-to-date or online information, or to look up details about the user (his portfolio is at https://fanaperana.github.io/portfolio/).",
            "parameters": {
                "type": "object",
                "properties": {
                    "url": {
                        "type": "string",
                        "description": "Absolute http(s) URL to fetch."
                    }
                },
                "required": ["url"]
            }
        }
    }]);

    // Bounded tool loop: let the model call fetch_url a few times, feeding each
    // result back, then return its final text answer. Some models (e.g. Claude
    // or Gemini via Copilot) may reject the `tools` field — fall back to a
    // plain request in that case so every model still works.
    let mut use_tools = true;
    for _ in 0..5 {
        let body = if use_tools {
            serde_json::json!({ "model": model, "messages": conversation, "tools": tools })
        } else {
            serde_json::json!({ "model": model, "messages": conversation })
        };

        let res = client
            .post(CHAT_URL)
            .bearer_auth(&token)
            .header("Editor-Version", EDITOR_VERSION)
            .header("Editor-Plugin-Version", PLUGIN_VERSION)
            .header("Copilot-Integration-Id", INTEGRATION_ID)
            .header("User-Agent", USER_AGENT)
            .json(&body)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let status = res.status();
        let json: Value = res.json().await.map_err(|e| e.to_string())?;

        if !status.is_success() {
            // Retry once without tools if the model doesn't accept them.
            if use_tools && status.as_u16() == 400 {
                use_tools = false;
                continue;
            }
            let msg = json
                .pointer("/error/message")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown error from Copilot");
            return Err(format!("Copilot ({status}): {msg}"));
        }

        let message = json
            .pointer("/choices/0/message")
            .cloned()
            .ok_or_else(|| "No message in Copilot response".to_string())?;

        let tool_calls = message
            .get("tool_calls")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        if tool_calls.is_empty() {
            return message
                .get("content")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .ok_or_else(|| "No content in Copilot response".to_string());
        }

        // Record the assistant's tool request, then answer each call.
        conversation.push(message);
        for call in tool_calls {
            let id = call.get("id").and_then(|v| v.as_str()).unwrap_or_default();
            let name = call
                .pointer("/function/name")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            let args = call
                .pointer("/function/arguments")
                .and_then(|v| v.as_str())
                .unwrap_or("{}");

            let result = if name == "fetch_url" {
                let url = serde_json::from_str::<Value>(args)
                    .ok()
                    .and_then(|v| v.get("url").and_then(|u| u.as_str()).map(String::from));
                match url {
                    Some(u) => fetch_url(&client, &u).await,
                    None => "Error: missing 'url' argument.".to_string(),
                }
            } else {
                format!("Error: unknown tool '{name}'.")
            };

            conversation.push(serde_json::json!({
                "role": "tool",
                "tool_call_id": id,
                "content": result,
            }));
        }
    }

    Err("The assistant made too many tool calls without answering.".to_string())
}

/// Fetch a public web page and return a trimmed plain-text approximation.
async fn fetch_url(client: &reqwest::Client, url: &str) -> String {
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return "Error: only absolute http(s) URLs are supported.".to_string();
    }

    let res = match client
        .get(url)
        .header("User-Agent", "wg-widget/0.1 (+https://github.com/Fanaperana)")
        .timeout(std::time::Duration::from_secs(20))
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => return format!("Error fetching {url}: {e}"),
    };

    let status = res.status();
    let html = match res.text().await {
        Ok(t) => t,
        Err(e) => return format!("Error reading {url}: {e}"),
    };
    if !status.is_success() {
        return format!("Error: {url} returned HTTP {status}");
    }

    let text = html_to_text(&html);
    let trimmed: String = text.chars().take(8000).collect();
    if trimmed.is_empty() {
        format!("Fetched {url} but found no readable text.")
    } else {
        format!("Contents of {url}:\n{trimmed}")
    }
}

/// Crude HTML-to-text: drop script/style, strip tags, decode a few entities.
fn html_to_text(html: &str) -> String {
    let mut s = html.to_string();
    for tag in ["script", "style", "noscript", "svg"] {
        loop {
            let lower = s.to_ascii_lowercase();
            let Some(open) = lower.find(&format!("<{tag}")) else {
                break;
            };
            let close = format!("</{tag}>");
            let end = match lower[open..].find(&close) {
                Some(j) => open + j + close.len(),
                None => s.len(),
            };
            s.replace_range(open..end, " ");
        }
    }

    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }

    let out = out
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'");

    out.split_whitespace().collect::<Vec<_>>().join(" ")
}
