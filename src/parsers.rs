use chrono::{Duration, TimeZone, Utc};
use rev_lines::RevLines;
use serde_json::json;
use std::env;
use std::fs;
use std::fs::File;
use std::io::{self, BufRead};
use std::path::{Path, PathBuf};

/// Processes the zsh history file and returns a vector of command entries as JSON values.
pub fn process_zsh_history(history_path: &str, cutoff_timestamp: i64) -> Vec<serde_json::Value> {
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

/// Identifies relevant project files for various project types in the current directory.
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

    // Check for Node.js project files.
    let package_json = current_dir.join("package.json");
    if package_json.exists() {
        files_to_include.push(PathBuf::from("package.json"));
        files_to_include.extend(find_source_files(&current_dir.join("src"), "js", max_files));
        files_to_include.extend(find_source_files(&current_dir.join("src"), "ts", max_files));
    }

    // Check for Go project files.
    let go_mod = current_dir.join("go.mod");
    if go_mod.exists() {
        files_to_include.push(PathBuf::from("go.mod"));
        files_to_include.extend(find_source_files(&current_dir, "go", max_files));
    }

    files_to_include
}

/// Finds source files with a given extension in the specified directory, up to a maximum count.
pub fn find_source_files(directory: &Path, extension: &str, max_files: usize) -> Vec<PathBuf> {
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
pub fn get_env_file_keys(file_path: &str) -> Vec<String> {
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

/// Reads the contents of project files and returns a vector of JSON objects.
pub fn read_project_files_content(project_files: &[PathBuf]) -> Vec<serde_json::Value> {
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
