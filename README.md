# Magic Quickstart

A command line app to generate a project quick start guide, using your environment and shell history (if enabled).

## Prerequisites

- Ensure you have Rust installed. You can install it from [rustup.rs](https://rustup.rs/).


## Build and Install

```sh
cargo build --release
cargo install --path .
```

## Run

1. Navigate to a project directory
2. Set up your environment variables in a `.env` file in your project root:

  ```
  OPENAI_API_KEY=your_openai_api_key
  OPENAI_MODEL=gpt-4o
  ENABLE_OPENAI=true
  HOURS_OF_SHELL_HISTORY=5
  MAX_FILE_COUNT_FOR_CONTEXT=5
  DEBUG_REQUEST=false
  INCLUDE_SHELL_HISTORY=true
  INCLUDE_REPOSITORY_FILES=true
  INCLUDE_ENV_FILE_KEYS=true
  ```

3. Run: `magic_quickstart`

## Examples

- [Go project ](/images/example_go_quickstart.png)
- [Python project ](/images/example_python_quickstart.png)
- [Rust project (this one)](/images/example_rust_quickstart.png)