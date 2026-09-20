//! OpenRouter (https://openrouter.ai) adapter for
//! [`AlertDescriptionGenerator`] -- same OpenAI-compatible API, structured-
//! output pattern, and timeout as [`crate::ai::openrouter::OpenRouterExtractor`],
//! applied to a second AI capability (docs/AI.md section 2 "Translation").

use std::time::Duration;

use async_trait::async_trait;
use safe_cameroon_application::alert_description_generation::{
    AlertDescriptionGenerator, GenerationError,
};
use safe_cameroon_domain::GeneratedDescription;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

const OPENROUTER_URL: &str = "https://openrouter.ai/api/v1/chat/completions";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Instructs the model to formalize/translate only, never to add or infer
/// facts the reviewer's draft does not contain (docs/AI.md section 2:
/// "Translation must not alter factual fields without retaining source
/// provenance").
const SYSTEM_PROMPT: &str = "You are formalizing a community safety alert description for \
    public distribution, based on a reviewer's draft (which may be informal, in English, in \
    French, or a mix of both). Produce two versions of the same description: one in formal \
    English, one in formal French. Preserve every fact in the draft exactly -- do not add, \
    remove, or guess any detail the draft does not contain. Use a clear, professional tone \
    suitable for public broadcast. This is an unverified suggestion for a human reviewer, never \
    a decision, and must be reviewed before use.";

pub struct OpenRouterDescriptionGenerator {
    http: reqwest::Client,
    api_key: String,
    model: String,
}

impl OpenRouterDescriptionGenerator {
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
struct ChatMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: Vec<ChatMessage<'a>>,
    response_format: ResponseFormat,
}

#[derive(Serialize)]
struct ResponseFormat {
    #[serde(rename = "type")]
    kind: &'static str,
    json_schema: JsonSchemaSpec,
}

#[derive(Serialize)]
struct JsonSchemaSpec {
    name: &'static str,
    strict: bool,
    schema: Value,
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

/// The model's raw JSON output. Unlike [`crate::ai::openrouter`]'s
/// extraction schema, both fields here are required non-null: this is a
/// translation/formalization of text the reviewer already supplied, not an
/// open-ended search of unstructured text, so an empty result is a
/// provider failure to surface, not a valid "found nothing" outcome.
#[derive(Deserialize)]
struct RawGeneratedDescription {
    description_en: String,
    description_fr: String,
}

fn json_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "description_en": {"type": "string"},
            "description_fr": {"type": "string"},
        },
        "required": ["description_en", "description_fr"],
        "additionalProperties": false,
    })
}

#[async_trait]
impl AlertDescriptionGenerator for OpenRouterDescriptionGenerator {
    fn provider(&self) -> &'static str {
        "OPENROUTER"
    }

    fn model(&self) -> &str {
        &self.model
    }

    async fn generate(&self, source_text: &str) -> Result<GeneratedDescription, GenerationError> {
        let request = ChatRequest {
            model: &self.model,
            messages: vec![
                ChatMessage {
                    role: "system",
                    content: SYSTEM_PROMPT,
                },
                ChatMessage {
                    role: "user",
                    content: source_text,
                },
            ],
            response_format: ResponseFormat {
                kind: "json_schema",
                json_schema: JsonSchemaSpec {
                    name: "alert_description_generation",
                    strict: true,
                    schema: json_schema(),
                },
            },
        };

        let response = self
            .http
            .post(OPENROUTER_URL)
            .bearer_auth(&self.api_key)
            .json(&request)
            .send()
            .await
            .map_err(|error| GenerationError {
                message: format!("could not reach OpenRouter: {error}"),
            })?;

        if !response.status().is_success() {
            return Err(GenerationError {
                message: format!("OpenRouter rejected the request ({})", response.status()),
            });
        }

        let parsed: ChatResponse = response.json().await.map_err(|error| GenerationError {
            message: format!("unexpected response from OpenRouter: {error}"),
        })?;

        let content = parsed
            .choices
            .into_iter()
            .next()
            .and_then(|choice| choice.message.content)
            .ok_or_else(|| GenerationError {
                message: "OpenRouter returned no content".into(),
            })?;

        let raw: RawGeneratedDescription =
            serde_json::from_str(&content).map_err(|error| GenerationError {
                message: format!("model output did not match the requested schema: {error}"),
            })?;

        if raw.description_en.trim().is_empty() || raw.description_fr.trim().is_empty() {
            return Err(GenerationError {
                message: "model returned an empty description".into(),
            });
        }

        Ok(GeneratedDescription {
            description_en: raw.description_en,
            description_fr: raw.description_fr,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_json_schema_requires_both_descriptions_as_non_nullable_strings() {
        let schema = json_schema();
        let required = schema["required"].as_array().unwrap();
        assert_eq!(required.len(), 2);
        for field in ["description_en", "description_fr"] {
            assert!(
                required.iter().any(|value| value == field),
                "{field} must be in `required` for strict mode"
            );
            assert_eq!(schema["properties"][field]["type"], "string");
        }
        assert_eq!(schema["additionalProperties"], false);
    }
}
