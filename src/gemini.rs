use serde::{Deserialize, Serialize};
use std::error::Error;

const GEMINI_API_URL: &str =
    "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash:generateContent";

#[derive(Serialize)]
struct GeminiRequest {
    contents: Vec<Content>,
}

#[derive(Serialize)]
struct Content {
    parts: Vec<Part>,
}

#[derive(Serialize)]
#[serde(untagged)]
enum Part {
    Text { text: String },
    InlineData { inline_data: InlineData },
}

#[derive(Serialize)]
struct InlineData {
    mime_type: String,
    data: String,
}

#[derive(Deserialize)]
struct GeminiResponse {
    candidates: Vec<Candidate>,
}

#[derive(Deserialize)]
struct Candidate {
    content: ResponseContent,
}

#[derive(Deserialize)]
struct ResponseContent {
    parts: Vec<ResponsePart>,
}

#[derive(Deserialize)]
struct ResponsePart {
    text: String,
}

/// Analyze a screenshot using Gemini API (from PNG data in memory)
pub fn analyze_screenshot_data(png_data: &[u8], api_key: &str) -> Result<String, Box<dyn Error>> {
    // Base64 encode the PNG data
    let base64_image = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, png_data);

    // Create the request with a meta prompt
    let meta_prompt = r#"You are a helpful AI assistant analyzing a screenshot. Please provide a clear, concise answer to help the user understand what they're seeing. 

IMPORTANT INSTRUCTIONS:
- Keep your response under 200 words for easy reading on screen if not generating code.
- Be specific and actionable in your analysis
- If you see text, code, or UI elements, explain what they are and what they do
- If you see an error or problem, suggest solutions
- If you see a question or task, provide a helpful answer
- Format your response with clear sections if needed
- Use simple, direct language that's easy to read quickly

What do you see in this screenshot and how can you help the user?"#;

    let request = GeminiRequest {
        contents: vec![Content {
            parts: vec![
                Part::Text {
                    text: meta_prompt.to_string(),
                },
                Part::InlineData {
                    inline_data: InlineData {
                        mime_type: "image/png".to_string(),
                        data: base64_image,
                    },
                },
            ],
        }],
    };

    // Make the API request
    let client = reqwest::blocking::Client::new();
    let url = format!("{}?key={}", GEMINI_API_URL, api_key);

    let response = client.post(&url).json(&request).send()?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().unwrap_or_else(|_| "Unknown error".to_string());
        
        let error_msg = match status.as_u16() {
            400 => format!("❌ Bad Request (400): Invalid API request format\nDetails: {}", error_text),
            401 => "❌ Unauthorized (401): Invalid API key\n💡 Check your GEMINI_API_KEY is correct".to_string(),
            403 => "❌ Forbidden (403): API key doesn't have permission\n💡 Verify your API key has Gemini access".to_string(),
            429 => "❌ Rate Limited (429): Too many requests\n💡 Wait a moment and try again".to_string(),
            500..=599 => format!("❌ Server Error ({}): Gemini service temporarily unavailable\n💡 Try again in a few minutes", status.as_u16()),
            _ => format!("❌ HTTP Error ({}): {}", status.as_u16(), error_text),
        };
        
        return Err(error_msg.into());
    }

    let gemini_response: GeminiResponse = response.json()?;

    // Extract the text from the response
    if let Some(candidate) = gemini_response.candidates.first() {
        if let Some(part) = candidate.content.parts.first() {
            return Ok(part.text.clone());
        }
    }

    Err("No response from Gemini API".into())
}

/// Get API key from config or environment variable
pub fn get_api_key(config_key: Option<String>) -> Result<String, Box<dyn Error>> {
    // Try config first
    if let Some(key) = config_key {
        if !key.is_empty() {
            return Ok(key);
        }
    }

    // Fall back to environment variable
    match std::env::var("GEMINI_API_KEY") {
        Ok(key) if !key.is_empty() => Ok(key),
        Ok(_) => Err("❌ GEMINI_API_KEY is empty\n💡 Set a valid API key in environment or config".into()),
        Err(_) => Err("❌ GEMINI_API_KEY not found\n💡 Get your key from https://makersuite.google.com/app/apikey\n💡 Then: export GEMINI_API_KEY=your_key_here".into()),
    }
}
