//! OpenRouter audio-input adapter for
//! [`AudioTranscriber`] (issue #160). Same vendor, endpoint, and auth as
//! [`super::openrouter::OpenRouterExtractor`] -- OpenRouter has no separate
//! Whisper-style `/audio/transcriptions` endpoint, but its chat-completions
//! endpoint accepts an `input_audio` content part (base64-encoded) on a
//! multimodal-audio-capable model, verified against
//! https://openrouter.ai/docs/features/multimodal/audio before writing
//! this adapter.

use std::time::Duration;

use async_trait::async_trait;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use safe_cameroon_application::audio_transcription::{AudioTranscriber, TranscriptionError};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

const OPENROUTER_URL: &str = "https://openrouter.ai/api/v1/chat/completions";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(45);

/// Deliberately narrow: transcribe only, no interpretation. The reporter
/// reviews and confirms this text client-side before it ever becomes a
/// report (AGENTS.md: "AI may not... become the sole verification
/// mechanism"; here there is no verification claim at all -- it's a
/// dictation aid, not an extraction).
const SYSTEM_PROMPT: &str = "Transcribe the spoken audio exactly as spoken, in the language it \
    was spoken in. Return only the transcript text, with no commentary, translation, or \
    formatting added.";

pub struct OpenRouterTranscriber {
    http: reqwest::Client,
    api_key: String,
    model: String,
}

impl OpenRouterTranscriber {
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(REQUEST_TIMEOUT)
                .build()
                .expect("reqwest client with only a timeout must build"),
            api_key,
            model,
        }
    }
}

#[derive(Serialize)]
struct InputAudio<'a> {
    data: String,
    format: &'a str,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ContentPart<'a> {
    Text { text: &'a str },
    InputAudio { input_audio: InputAudio<'a> },
}

#[derive(Serialize)]
struct ChatMessage<'a> {
    role: &'a str,
    content: Value,
}

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: Vec<ChatMessage<'a>>,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatResponseMessage,
}

#[derive(Deserialize)]
struct ChatResponseMessage {
    content: Option<String>,
}

#[async_trait]
impl AudioTranscriber for OpenRouterTranscriber {
    fn provider(&self) -> &'static str {
        "OPENROUTER"
    }

    fn model(&self) -> &str {
        &self.model
    }

    async fn transcribe(
        &self,
        audio_bytes: &[u8],
        format: &str,
    ) -> Result<String, TranscriptionError> {
        let encoded = BASE64.encode(audio_bytes);
        let request = ChatRequest {
            model: &self.model,
            messages: vec![
                ChatMessage {
                    role: "system",
                    content: json!(SYSTEM_PROMPT),
                },
                ChatMessage {
                    role: "user",
                    content: json!([
                        ContentPart::InputAudio {
                            input_audio: InputAudio {
                                data: encoded,
                                format,
                            },
                        },
                        ContentPart::Text {
                            text: "Transcribe this audio.",
                        },
                    ]),
                },
            ],
        };

        let response = self
            .http
            .post(OPENROUTER_URL)
            .bearer_auth(&self.api_key)
            .json(&request)
            .send()
            .await
            .map_err(|error| TranscriptionError {
                message: format!("could not reach OpenRouter: {error}"),
            })?;

        if !response.status().is_success() {
            return Err(TranscriptionError {
                message: format!("OpenRouter rejected the request ({})", response.status()),
            });
        }

        let parsed: ChatResponse = response.json().await.map_err(|error| TranscriptionError {
            message: format!("unexpected response from OpenRouter: {error}"),
        })?;

        parsed
            .choices
            .into_iter()
            .next()
            .and_then(|choice| choice.message.content)
            .filter(|content| !content.trim().is_empty())
            .ok_or_else(|| TranscriptionError {
                message: "OpenRouter returned no transcript".into(),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_audio_content_part_serializes_to_the_documented_shape() {
        let part = ContentPart::InputAudio {
            input_audio: InputAudio {
                data: "abc123".into(),
                format: "webm",
            },
        };
        let value = serde_json::to_value(&part).unwrap();
        assert_eq!(value["type"], json!("input_audio"));
        assert_eq!(value["input_audio"]["data"], json!("abc123"));
        assert_eq!(value["input_audio"]["format"], json!("webm"));
    }
}
