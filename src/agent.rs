use ollama_rs::{Ollama, generation::{completion::request::GenerationRequest, embeddings::request::{EmbeddingsInput, GenerateEmbeddingsRequest}}};
use serde_json::Value;

pub struct OllamaInstance {
    model: String,
    embeddings_model: String
}

impl OllamaInstance {
    pub fn new(model: &str, embeddings_model: &str) -> Self {
        Self { model: model.to_string() , embeddings_model: embeddings_model.to_string() }
    }

    pub async fn ask(&self, text: &str) -> Option<String> {
        let ollama = Ollama::default();
        let res = ollama.generate(
            GenerationRequest::new(self.model.clone(),
             text.to_string())).await;
        
        if let Ok(res) = res {
            Some(res.response)
        }
        else {
            None
        }
    }

    pub async fn get_embeddings(&self, text: &str) -> Option<Vec<f32>> {
        let ollama = Ollama::default();
        let embeddings_input = EmbeddingsInput::Single(text.to_string());
        let res = ollama.generate_embeddings(
            GenerateEmbeddingsRequest::new(self.embeddings_model.clone(), embeddings_input)).await;

        if let Ok(res) = res {
            Some(res.embeddings.into_iter().next().unwrap())
        }
        else {
            None
        }
    }
}

pub struct SortingAgentImpl {
    ollama_instance: OllamaInstance
}

impl SortingAgentImpl {
    pub async fn new(ollama_instance: OllamaInstance) -> Result<Self, String> {
        match ollama_instance.ask("hello").await {
            Some(_) => Ok(Self { ollama_instance }),
            None    => Err("Failed to connect to Ollama. Make sure the service is running.".into()),
        }
    }

    fn extract_json(&self, text: &str) -> Option<String> {
        let start = text.rfind('{')?;
        let end = text.rfind('}')?;
        let json = &text[start..=end];
        Some(String::from(json))
    }

    pub async fn classify_note(&self, available_classes: Vec<String>, note_text: &str) -> Option<String> { 
        let prompt = format!(
            "You are a note classifier. Available categories: `{}`. Note text: `{}`. \
            Reply with a JSON object specifying which category this note belongs to. \
            If none of the existing categories fit, suggest a new one in the same language as the existing categories. \
            Example response: {{\"class-name\": \"Watch Later (Movies)\"}} or {{\"class-name\": \"To Do\"}}",
            available_classes.join(";"), note_text
        );

        println!("DEBUG {}", prompt);

        if let Some(res) = self.ollama_instance.ask(&prompt).await {
            if let Some(json_str) = self.extract_json(res.as_str()) {
                let extracted_json: Value = serde_json::from_str(json_str.as_str()).unwrap();
                let class_name = extracted_json["class-name"].as_str().unwrap_or("Uncategorized");
                Some(String::from(class_name))
            }
            else {
                None
            }
        }
        else {
            None
        }
    }

    pub async fn get_embeddings(&self, note_text: &str) -> Result<Vec<f32>, String> {
        self.ollama_instance
            .get_embeddings(note_text)
            .await
            .ok_or_else(|| "Cannot get embeddings".into())
    }

}