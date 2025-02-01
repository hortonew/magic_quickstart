use chrono::{Duration, TimeZone, Utc};
use reqwest::blocking::Client;
use rev_lines::RevLines;
use serde_json::json;
use std::env;
use std::fs;
use std::fs::File;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

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
    println!("Cutoff time: {}", cutoff_time);

    // Process the shell history.
    let history_path = format!("{}/.zsh_history", env::var("HOME").unwrap());
    println!("History path is: {}", history_path);
    let command_history = process_zsh_history(&history_path, cutoff_time.timestamp());

    // Write command history to a temporary file.
    write_json_to_file("command_history.json", &json!(command_history));

    // Read project file contents.
    let project_files_content = read_project_files_content(&project_files);
    write_json_to_file("project_files_content.json", &json!(project_files_content));

    // Read .env file keys.
    let env_file_keys = get_env_file_keys(".env");
    write_json_to_file("env_file_keys.json", &json!(env_file_keys));

    // Build the request payload for OpenAI.
    let request_body = build_request_payload(
        config.openai_model.clone(),
        config.time_back_hours,
        &command_history,
        &project_files,
        &project_files_content,
        &env_file_keys,
    );
    write_json_to_file("request.json", &request_body);

    // Only send the request if ENABLE_OPENAI is set to true.
    if !config.enable_openai {
        println!("ENABLE_OPENAI is not set to true. Exiting early.");
        return;
    }

    // Send the API request and write the Markdown result.
    let markdown_content = send_openai_request(&config, &request_body);
    write_to_file("README_TMP.md", markdown_content.as_bytes());
}

/// Holds configuration values loaded from environment variables.
struct Config {
    openai_api_key: String,
    max_file_context: usize,
    time_back_hours: i64,
    openai_model: String,
    enable_openai: bool,
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

        Config {
            openai_api_key,
            max_file_context,
            time_back_hours,
            openai_model,
            enable_openai,
        }
    }
}

/// Processes the zsh history file and returns a vector of command entries as JSON values.
fn process_zsh_history(history_path: &str, cutoff_timestamp: i64) -> Vec<serde_json::Value> {
    let file = File::open(history_path).expect("Failed to open .zsh_history");
    let rev_lines = RevLines::new(file);
    let mut command_history = Vec::new();

    for line_result in rev_lines {
        match line_result {
            Ok(line) => {
                if let Some((timestamp, exit_code, command)) = parse_zsh_history(&line) {
                    if timestamp >= cutoff_timestamp {
                        let command_time = match Utc.timestamp_opt(timestamp, 0) {
                            chrono::LocalResult::Single(time) => time.format("%Y-%m-%d %H:%M:%S").to_string(),
                            _ => "Invalid timestamp".to_string(),
                        };

                        let elapsed_secs = Utc::now().timestamp() - timestamp;
                        let relative_duration = Duration::seconds(elapsed_secs);
                        let formatted_relative_time = humantime::format_duration(relative_duration.to_std().unwrap()).to_string();

                        command_history.push(json!({
                            "timestamp": command_time,
                            "relative_time": formatted_relative_time,
                            "exit_code": exit_code,
                            "command": command
                        }));
                    }
                } else {
                    // Exit early if the history entry cannot be parsed.
                    break;
                }
            }
            Err(_) => {
                println!("Skipping invalid UTF-8 sequence");
            }
        }
    }

    command_history
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

/// Reads the contents of project files and returns a vector of JSON objects.
fn read_project_files_content(project_files: &[PathBuf]) -> Vec<serde_json::Value> {
    project_files
        .iter()
        .map(|file_path| {
            let content = fs::read_to_string(file_path).unwrap_or_default();
            json!({
                "file_path": file_path.display().to_string(),
                "content": content
            })
        })
        .collect()
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
            {"role": "system","content": "You are a helpful assistant specialized in creating concise project quickstart guides. Use the provided context to generate a Markdown README.md that lists only the essential commands to get started. Ensure the guide is strictly relevant to the detected project type (for example, if it is a Rust project, do not include Node.js instructions, and vice versa). Output only Markdown content without any extra explanation, preamble, or code fences."},
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

/// Parses a line from the zsh history and returns a tuple of (timestamp, exit_code, command).
fn parse_zsh_history(entry: &str) -> Option<(i64, String, String)> {
    if !entry.starts_with(':') {
        return None;
    }

    let parts: Vec<&str> = entry.splitn(3, ':').collect();
    if parts.len() < 3 {
        return None;
    }

    let timestamp_str = parts[1].trim();
    let command_part = parts[2];

    let timestamp = match timestamp_str.parse::<i64>() {
        Ok(t) => t,
        Err(_) => {
            println!("Failed to parse timestamp: {}", timestamp_str);
            return None;
        }
    };

    let command_parts: Vec<&str> = command_part.splitn(2, ';').collect();
    if command_parts.len() < 2 {
        return None;
    }

    let exit_code = command_parts[0].trim().to_string();
    let command = command_parts[1].trim().to_string();

    Some((timestamp, exit_code, command))
}

/// Identifies relevant project files for Rust or Python projects in the current directory.
pub fn find_project_files(max_files: usize) -> Vec<PathBuf> {
    let current_dir = env::current_dir().expect("Failed to get current working directory");
    let mut files_to_include = Vec::new();

    // Check for Rust project files.
    let cargo_toml = current_dir.join("Cargo.toml");
    if cargo_toml.exists() {
        files_to_include.push(PathBuf::from("Cargo.toml"));
        files_to_include.extend(find_source_files(&current_dir.join("src"), "rs", max_files));
    }

    // Check for Python project files.
    let pyproject_toml = current_dir.join("pyproject.toml");
    if pyproject_toml.exists() {
        files_to_include.push(PathBuf::from("pyproject.toml"));
        files_to_include.extend(find_source_files(&current_dir.join("src"), "py", max_files));
    }

    files_to_include
}

/// Finds source files with a given extension in the specified directory, up to a maximum count.
fn find_source_files(directory: &Path, extension: &str, max_files: usize) -> Vec<PathBuf> {
    let mut found_files = Vec::new();

    if let Ok(entries) = fs::read_dir(directory) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) == Some(extension) {
                let relative_path = path.strip_prefix(env::current_dir().unwrap()).unwrap_or(&path).to_path_buf();
                found_files.push(relative_path);
                if found_files.len() >= max_files {
                    break;
                }
            }
        }
    }

    found_files
}

/// Reads the structure of the .env file and returns the keys (without values).
fn get_env_file_keys(file_path: &str) -> Vec<String> {
    let file = File::open(file_path).expect("Failed to open .env file");
    let reader = io::BufReader::new(file);
    let mut keys = Vec::new();

    for line in reader.lines().map_while(Result::ok) {
        if let Some((key, _)) = line.split_once('=') {
            keys.push(key.trim().to_string());
        }
    }

    keys
}
