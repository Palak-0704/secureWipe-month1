use std::io::{self, Write};
use std::env;
use reqwest::blocking::Client;
use serde_json::json;

fn ask_groq(prompt: &str, api_key: &str) -> Result<String, String> {
    let client = Client::new();
    let url = "https://api.groq.com/openai/v1/chat/completions";
    let body = json!({
        "model": "openai/gpt-oss-120b",
        "messages": [
            {"role": "system", "content": "You are a helpful AI assistant for secure data wiping."},
            {"role": "user", "content": prompt}
        ],
        "max_tokens": 256,
        "temperature": 0.7
    });
    let resp = client.post(url)
        .bearer_auth(api_key)
        .json(&body)
        .send();
    match resp {
        Ok(r) => {
            if r.status().is_success() {
                let v: serde_json::Value = r.json().unwrap_or(json!({"choices": [{"message": {"content": "(No response)"}}]}));
                Ok(v["choices"][0]["message"]["content"].as_str().unwrap_or("(No response)").to_string())
            } else {
                Err(format!("API error: {}", r.text().unwrap_or_default()))
            }
        }
        Err(e) => Err(format!("Request error: {}", e)),
    }
}

fn main() {
    let api_key = env::var("GROQ_API_KEY").unwrap_or_else(|_| {
        println!("Please set the GROQ_API_KEY environment variable.");
        std::process::exit(1);
    });
    println!("Groq AI Chatbot (Rust, Month 1)");
    println!("Type your question and press Enter. Type 'exit' to quit.\n");
    loop {
        print!("You: ");
        io::stdout().flush().unwrap();
        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_err() {
            println!("Error reading input. Try again.");
            continue;
        }
        let input = input.trim();
        if input.eq_ignore_ascii_case("exit") {
            println!("Goodbye!");
            break;
        }
        match ask_groq(input, &api_key) {
            Ok(response) => println!("Bot: {}\n", response),
            Err(err) => println!("Error: {}\n", err),
        }
    }
}