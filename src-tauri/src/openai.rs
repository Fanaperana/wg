use serde_json::Value;

const CHAT_URL: &str = "https://api.openai.com/v1/chat/completions";
const TRANSCRIBE_URL: &str = "https://api.openai.com/v1/audio/transcriptions";

/// Send a chat completion request and return the assistant's reply text.
pub async fn chat(api_key: &str, model: &str, messages: Value) -> Result<String, String> {
    let client = reqwest::Client::new();
    let body = serde_json::json!({
        "model": model,
        "messages": messages,
    });

    let res = client
        .post(CHAT_URL)
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let status = res.status();
    let json: Value = res.json().await.map_err(|e| e.to_string())?;

    if !status.is_success() {
        let msg = json
            .pointer("/error/message")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown error from OpenAI");
        return Err(format!("OpenAI ({status}): {msg}"));
    }

    json.pointer("/choices/0/message/content")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "No content in OpenAI response".to_string())
}

/// Transcribe WAV audio bytes using a Whisper-compatible transcription model.
pub async fn transcribe(api_key: &str, model: &str, wav: Vec<u8>) -> Result<String, String> {
    let client = reqwest::Client::new();

    let part = reqwest::multipart::Part::bytes(wav)
        .file_name("audio.wav")
        .mime_str("audio/wav")
        .map_err(|e| e.to_string())?;

    let form = reqwest::multipart::Form::new()
        .text("model", model.to_string())
        .part("file", part);

    let res = client
        .post(TRANSCRIBE_URL)
        .bearer_auth(api_key)
        .multipart(form)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let status = res.status();
    let json: Value = res.json().await.map_err(|e| e.to_string())?;

    if !status.is_success() {
        let msg = json
            .pointer("/error/message")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown error from OpenAI");
        return Err(format!("OpenAI ({status}): {msg}"));
    }

    json.get("text")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "No text in transcription response".to_string())
}
