use chrono::{DateTime, Duration, Utc};
use reqwest::blocking::Client;
use rev_lines::RevLines;
use serde_json::json;
use std::env;
use std::fs;
use std::fs::File;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

fn main() {
    // Load environment variables from .env file
    dotenv::dotenv().expect("Failed to load .env file");
    let api_key = env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY not found in environment variables");
    let max_file_context = env::var("MAX_FILE_CONTEXT")
        .unwrap_or("5".to_string())
        .parse::<usize>()
        .expect("Invalid MAX_FILE_CONTEXT");
    let project_files = find_project_files(max_file_context);

    // print these files inline
    println!("Relevant project files: {:?}", project_files);

    let time_back_hours: i64 = env::var("TIME_BACK_HOURS")
        .unwrap_or_else(|_| "5".to_string())
        .parse()
        .expect("Invalid TIME_BACK_HOURS");
    let cutoff_time = Utc::now() - Duration::hours(time_back_hours);
    println!("Cutoff time: {}", cutoff_time);

    // Read the zsh history file
    let history_path = format!("{}/.zsh_history", env::var("HOME").unwrap());
    let file = File::open(&history_path).expect("Failed to open .zsh_history");
    let rev_lines = RevLines::new(file);

    let mut command_history = vec![];
    println!("History path is: {}", history_path);

    for line in rev_lines {
        if let Ok(line) = line {
            if let Some((timestamp, exit_code, command)) = parse_zsh_history(&line) {
                if timestamp >= cutoff_time.timestamp() {
                    let command_time = DateTime::from_timestamp(timestamp, 0)
                        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                        .unwrap_or_else(|| timestamp.to_string());

                    let relative_time = Utc::now().timestamp() - timestamp;
                    let relative_duration = Duration::seconds(relative_time);
                    let formatted_relative_time = humantime::format_duration(relative_duration.to_std().unwrap()).to_string();

                    command_history.push(json!({
                        "timestamp": command_time,
                        "relative_time": formatted_relative_time,
                        "exit_code": exit_code,
                        "command": command
                    }));
                }
            } else {
                // We reached all invalid timestamps so we can exit early
                break;
            }
        } else {
            println!("Skipping invalid UTF-8 sequence");
        }
    }

    // output the command history to a temporary file called: command_history.json
    let mut file = File::create("command_history.json").expect("Failed to create command_history.json");
    file.write_all(json!(command_history).to_string().as_bytes())
        .expect("Failed to write to command_history.json");

    // Read the contents of the project files
    let mut project_files_content = vec![];
    for file_path in &project_files {
        let content = fs::read_to_string(file_path).unwrap_or_else(|_| String::new());
        project_files_content.push(json!({
            "file_path": file_path.display().to_string(),
            "content": content
        }));
    }

    // write the file contents that will be sent to the api to a temporary file called: project_files_content.json
    let mut file = File::create("project_files_content.json").expect("Failed to create project_files_content.json");
    file.write_all(json!(project_files_content).to_string().as_bytes())
        .expect("Failed to write to project_files_content.json");

    // Get the structure of the .env file if it exists.
    let env_file_keys = get_env_file_keys(".env");

    // Write the env file keys to a temporary file called: env_file_keys.json
    let mut file = File::create("env_file_keys.json").expect("Failed to create env_file_keys.json");
    file.write_all(json!(env_file_keys).to_string().as_bytes())
        .expect("Failed to write to env_file_keys.json");

    // Create an HTTP client
    let client = Client::new();
    let url = "https://api.openai.com/v1/chat/completions";

    // Construct the request payload
    let request_body = json!({
        "model": env::var("OPENAI_MODEL").unwrap_or("gpt-4o".to_string()),
        "messages": [
            { "role": "system", "content": "You are a helpful assistant who is excellent at building projects from the ground up and understanding how a user would need a readable quickstart section to get started. Ensure that the guide is strictly relevant to the detected project type." },
            { "role": "user", "content": "I'm building a project and I want to build a quickstart guide. The commands that I ran are included, but could also contain commands that don't apply to this project. Using these files as reference, build the quickstart guide to my README.md that would just list the commands to get started on this project." },
            { "role": "system", "content": "Only consider relevant commands and files for the detected project type. If it is a Rust project, do not include Node.js or npm-related instructions. If it is a Python project, do not include Rust-related instructions." },
            { "role": "user", "content": format!("My shell history contained this for {:?}: {:?}", time_back_hours, command_history) },
            { "role": "user", "content": format!("My files include: {:?}", project_files) },
            { "role": "user", "content": format!("File contents: {:?}", project_files_content) },
            { "role": "user", "content": format!("Environment file structure if it exists: {:?}", env_file_keys) },
            { "role": "system", "content": "Only output the Markdown content without any explanation, preamble, or additional context. Do not include triple backticks before or after the output.  If an environment structure was provided, also include instructions on setting up the environment.  If a project description was included in any TOML file, include that under the heading." }
        ]
    });

    // Write the request_body to request.json
    let mut file = File::create("request.json").expect("Failed to create request.json");
    file.write_all(request_body.to_string().as_bytes())
        .expect("Failed to write to request.json");

    // Send the request if ENABLE_OPENAI is set to true
    if env::var("ENABLE_OPENAI").unwrap_or("false".to_string()).to_lowercase() != "true" {
        println!("ENABLE_OPENAI is not set to true. Exiting early.");
        return;
    }
    let response = client
        .post(url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .expect("Failed to send request");

    // Print the response
    let response_json: serde_json::Value = response.json().expect("Failed to parse response");
    let markdown_content = response_json["choices"][0]["message"]["content"].as_str().unwrap_or("");

    // Write only the markdown content to README_TMP.md
    let mut file = File::create("README_TMP.md").expect("Failed to create README_TMP.md");
    file.write_all(markdown_content.as_bytes())
        .expect("Failed to write to README_TMP.md");
}

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
    let current_dir = std::env::current_dir().expect("Failed to get current working directory");
    let mut files_to_include = Vec::new();

    // Check for Rust project files
    let cargo_toml = current_dir.join("Cargo.toml");
    if cargo_toml.exists() {
        files_to_include.push(PathBuf::from("Cargo.toml"));
        files_to_include.extend(find_source_files(&current_dir.join("src"), "rs", max_files));
    }

    // Check for Python project files
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
                if let Ok(relative_path) = path.strip_prefix(std::env::current_dir().unwrap()) {
                    found_files.push(relative_path.to_path_buf());
                } else {
                    found_files.push(path);
                }
                if found_files.len() >= max_files {
                    break;
                }
            }
        }
    }

    found_files
}

/// Reads the structure of the .env file and returns the keys without values.
fn get_env_file_keys(file_path: &str) -> Vec<String> {
    let file = File::open(file_path).expect("Failed to open .env file");
    let reader = io::BufReader::new(file);
    let mut keys = vec![];

    for line in reader.lines().map_while(Result::ok) {
        if let Some((key, _)) = line.split_once('=') {
            keys.push(key.trim().to_string());
        }
    }

    keys
}
