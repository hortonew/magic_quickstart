use chrono::{Duration, Utc};
use reqwest::blocking::Client;
use serde_json::json;
use std::env;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

mod parsers;
use parsers::{find_project_files, get_env_file_keys, process_zsh_history, read_project_files_content};

fn main() {
    // Load environment variables from .env file.
    dotenv::dotenv().expect("Failed to load .env file");

    // Load configuration from environment variables.
    let config = Config::from_env();

    // Identify project files to be used for context.
    let project_files = find_project_files(config.max_file_context);
    println!("Relevant project files: {:?}", project_files);

    // Calculate the cutoff time for shell history.
    let cutoff_time = Utc::now() - Duration::hours(config.time_back_hours);
    println!("Cutoff time for shell history: {}", cutoff_time);

    // Process the shell history.
    let history_path = format!("{}/.zsh_history", env::var("HOME").unwrap());
    println!("History path is: {}", history_path);
    let command_history = process_zsh_history(&history_path, cutoff_time.timestamp());

    // Write command history to a temporary file if DEBUG_REQUEST is true.
    if config.debug_request {
        write_json_to_file("command_history.json", &json!(command_history));
    }

    // Read project file contents.
    let project_files_content = read_project_files_content(&project_files);
    if config.debug_request {
        write_json_to_file("project_files_content.json", &json!(project_files_content));
    }

    // Read .env file keys.
    let env_file_keys = get_env_file_keys(".env");
    if config.debug_request {
        write_json_to_file("env_file_keys.json", &json!(env_file_keys));
    }

    // Build the request payload for OpenAI.
    let request_body = build_request_payload(
        config.openai_model.clone(),
        config.time_back_hours,
        &command_history,
        &project_files,
        &project_files_content,
        &env_file_keys,
    );
    if config.debug_request {
        write_json_to_file("request.json", &request_body);
    }

    // Only send the request if ENABLE_OPENAI is set to true.
    if !config.enable_openai {
        println!("ENABLE_OPENAI is not set to true. Exiting early.");
        return;
    }

    // Send the API request and write the Markdown result.
    let markdown_content = send_openai_request(&config, &request_body);
    // with timestamp at end of generated file
    write_to_file(
        format!("README_GENERATED_{}.md", Utc::now().format("%Y-%m-%d_%H-%M-%S")),
        markdown_content.as_bytes(),
    );
}

/// Holds configuration values loaded from environment variables.
struct Config {
    openai_api_key: String,
    max_file_context: usize,
    time_back_hours: i64,
    openai_model: String,
    enable_openai: bool,
    debug_request: bool,
}

impl Config {
    /// Loads the configuration from environment variables.
    fn from_env() -> Self {
        let openai_api_key = env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY not found in environment variables");
        let max_file_context = env::var("MAX_FILE_COUNT_FOR_CONTEXT")
            .unwrap_or_else(|_| "5".to_string())
            .parse::<usize>()
            .expect("Invalid MAX_FILE_COUNT_FOR_CONTEXT");
        let time_back_hours = env::var("HOURS_OF_SHELL_HISTORY")
            .unwrap_or_else(|_| "5".to_string())
            .parse::<i64>()
            .expect("Invalid HOURS_OF_SHELL_HISTORY");
        let openai_model = env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-4o".to_string());
        let enable_openai = env::var("ENABLE_OPENAI").unwrap_or_else(|_| "false".to_string()).to_lowercase() == "true";
        let debug_request = env::var("DEBUG_REQUEST").unwrap_or_else(|_| "false".to_string()).to_lowercase() == "true";

        Config {
            openai_api_key,
            max_file_context,
            time_back_hours,
            openai_model,
            enable_openai,
            debug_request,
        }
    }
}

/// Writes JSON data to the specified file.
fn write_json_to_file<P: AsRef<Path>>(file_path: P, data: &serde_json::Value) {
    let mut file = File::create(&file_path).unwrap_or_else(|_| panic!("Failed to create {}", file_path.as_ref().display()));
    file.write_all(data.to_string().as_bytes())
        .unwrap_or_else(|_| panic!("Failed to write to {}", file_path.as_ref().display()));
}

/// Writes raw bytes to the specified file.
fn write_to_file<P: AsRef<Path>>(file_path: P, data: &[u8]) {
    let mut file = File::create(&file_path).unwrap_or_else(|_| panic!("Failed to create {}", file_path.as_ref().display()));
    file.write_all(data)
        .unwrap_or_else(|_| panic!("Failed to write to {}", file_path.as_ref().display()));
}

/// Constructs the JSON request payload for the OpenAI API.
fn build_request_payload(
    model: String,
    time_back_hours: i64,
    command_history: &[serde_json::Value],
    project_files: &[PathBuf],
    project_files_content: &[serde_json::Value],
    env_file_keys: &[String],
) -> serde_json::Value {
    json!({
        "model": model,
        "messages": [
            {"role": "system","content": "You are a helpful assistant specialized in creating concise project quickstart guides. Use the provided context to generate a Markdown README.md that lists only the essential commands to get started. Ensure the guide is strictly relevant to the detected project type (for example, if it is a Rust project, do not include Node.js instructions, and vice versa). Output only Markdown content without any extra explanation, preamble, or code fences.  If you see any descriptions of the project in the TOML files, be sure to include this under the project name.  If any output is generated, be sure to call this out in the generated readme.  Dependencies can be called out, but don't merely include what's in the toml file as this is redundant."},
            {"role": "user","content": "Generate a quickstart guide for my project based on the following data. Note that some commands may be irrelevant."},
            {"role": "user","content": format!("Shell history (last {} hours): {:?}", time_back_hours, command_history)},
            {"role": "user","content": format!("Project files: {:?}", project_files)},
            {"role": "user","content": format!("File contents: {:?}", project_files_content)},
            {"role": "user","content": format!("Environment file keys (if any): {:?}", env_file_keys)}
        ]
    })
}

/// Sends the request to the OpenAI API and returns the Markdown content from the response.
fn send_openai_request(config: &Config, request_body: &serde_json::Value) -> String {
    let client = Client::new();
    let url = "https://api.openai.com/v1/chat/completions";

    let response = client
        .post(url)
        .header("Authorization", format!("Bearer {}", config.openai_api_key))
        .header("Content-Type", "application/json")
        .json(request_body)
        .send()
        .expect("Failed to send request");

    let response_json: serde_json::Value = response.json().expect("Failed to parse response");
    response_json["choices"][0]["message"]["content"].as_str().unwrap_or("").to_string()
}
