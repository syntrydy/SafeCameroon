//! OpenRouter (https://openrouter.ai) adapter for
//! [`ReportExtractor`] -- a single OpenAI-compatible API in front of many
//! underlying models, so the model used can change (env var) without a
//! code or vendor-integration change.

use std::time::Duration;

use async_trait::async_trait;
use safe_cameroon_application::ai_extraction::{ExtractionError, ReportExtractor};
use safe_cameroon_domain::ExtractedReportFields;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

const OPENROUTER_URL: &str = "https://openrouter.ai/api/v1/chat/completions";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Instructs the model to extract only what the text actually says, into a
/// schema fixed regardless of incident type (docs/ROADMAP.md Stage 4) --
/// the model never sees or suggests a [`safe_cameroon_domain::IncidentType`]
/// value, only its own free-text guess at a category.
const SYSTEM_PROMPT: &str = "You are assisting a trained human reviewer triaging an anonymous \
    safety report (for example, a missing person or another protection incident). Extract only \
    information explicitly present in the report text into the given JSON schema. Do not guess, \
    infer, or invent any detail the text does not state. Leave a field null if the text does not \
    mention it. This is an unverified suggestion for a human reviewer, never a decision, and must \
    never be treated as confirmed fact.";

pub struct OpenRouterExtractor {
    http: reqwest::Client,
    api_key: String,
    model: String,
}

impl OpenRouterExtractor {
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

/// The model's raw JSON output, deserialized before
/// [`ExtractedReportFields::new`] normalizes blank strings to `None`.
#[derive(Deserialize)]
struct RawExtractionFields {
    person_description: Option<String>,
    age: Option<String>,
    time: Option<String>,
    place: Option<String>,
    incident_category: Option<String>,
    vehicle_details: Option<String>,
    contact_request: Option<String>,
}

/// OpenAI-compatible strict structured outputs require every property to be
/// listed in `required`; a field is made optional by making its *type*
/// nullable (`["string", "null"]`) instead of omitting it here.
fn json_schema() -> Value {
    let nullable_string = json!({"type": ["string", "null"]});
    json!({
        "type": "object",
        "properties": {
            "person_description": nullable_string,
            "age": nullable_string,
            "time": nullable_string,
            "place": nullable_string,
            "incident_category": nullable_string,
            "vehicle_details": nullable_string,
            "contact_request": nullable_string,
        },
        "required": [
            "person_description", "age", "time", "place",
            "incident_category", "vehicle_details", "contact_request",
        ],
        "additionalProperties": false,
    })
}

#[async_trait]
impl ReportExtractor for OpenRouterExtractor {
    fn provider(&self) -> &'static str {
        "OPENROUTER"
    }

    fn model(&self) -> &str {
        &self.model
    }

    async fn extract(&self, raw_content: &str) -> Result<ExtractedReportFields, ExtractionError> {
        let request = ChatRequest {
            model: &self.model,
            messages: vec![
                ChatMessage {
                    role: "system",
                    content: SYSTEM_PROMPT,
                },
                ChatMessage {
                    role: "user",
                    content: raw_content,
                },
            ],
            response_format: ResponseFormat {
                kind: "json_schema",
                json_schema: JsonSchemaSpec {
                    name: "report_extraction",
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
            .map_err(|error| ExtractionError {
                message: format!("could not reach OpenRouter: {error}"),
            })?;

        if !response.status().is_success() {
            return Err(ExtractionError {
                message: format!("OpenRouter rejected the request ({})", response.status()),
            });
        }

        let parsed: ChatResponse = response.json().await.map_err(|error| ExtractionError {
            message: format!("unexpected response from OpenRouter: {error}"),
        })?;

        let content = parsed
            .choices
            .into_iter()
            .next()
            .and_then(|choice| choice.message.content)
            .ok_or_else(|| ExtractionError {
                message: "OpenRouter returned no content".into(),
            })?;

        let raw: RawExtractionFields =
            serde_json::from_str(&content).map_err(|error| ExtractionError {
                message: format!("model output did not match the requested schema: {error}"),
            })?;

        Ok(ExtractedReportFields::new(
            raw.person_description,
            raw.age,
            raw.time,
            raw.place,
            raw.incident_category,
            raw.vehicle_details,
            raw.contact_request,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_json_schema_lists_every_field_as_required_but_nullable() {
        let schema = json_schema();
        let required = schema["required"].as_array().unwrap();
        assert_eq!(required.len(), 7);
        for field in [
            "person_description",
            "age",
            "time",
            "place",
            "incident_category",
            "vehicle_details",
            "contact_request",
        ] {
            assert!(
                required.iter().any(|value| value == field),
                "{field} must be in `required` for strict mode"
            );
            assert_eq!(
                schema["properties"][field]["type"],
                json!(["string", "null"])
            );
        }
        assert_eq!(schema["additionalProperties"], json!(false));
    }
}
