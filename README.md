# Magic Quickstart

A command line app to generate a project quick start guide, using your environment and shell history (if enabled).

## Prerequisites

- Ensure you have Rust installed. You can install it from [rustup.rs](https://rustup.rs/).
- [OpenAPI API Key](https://platform.openai.com/api-keys)


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

## Shell support

- zsh

## .zshrc setup

⚠️ The last thing you want is to have shell history used for context, and your shell history to contain sensitive info.  `HIST_IGNORE_SPACE` is there so you can still type sensitive commands but start them with a space.  Then they won't be added to your history.

```sh
setopt EXTENDED_HISTORY      # Write the history file in the ':start:elapsed;command' format.
setopt INC_APPEND_HISTORY    # Write to the history file immediately, not when the shell exits.
setopt SHARE_HISTORY         # Share history between all sessions.
setopt HIST_IGNORE_DUPS      # Do not record an event that was just recorded again.
setopt HIST_IGNORE_ALL_DUPS  # Delete an old recorded event if a new event is a duplicate.
setopt HIST_IGNORE_SPACE     # Do not record an event starting with a space.
setopt HIST_SAVE_NO_DUPS     # Do not write a duplicate event to the history file.
setopt HIST_VERIFY           # Do not execute immediately upon history expansion.
setopt APPEND_HISTORY        # append to history file (Default)
setopt HIST_NO_STORE         # Don't store history commands
setopt HIST_REDUCE_BLANKS    # Remove superfluous blanks from each command line being added to the history.
HISTFILE="$HOME/.zsh_history"
HISTSIZE=10000000
SAVEHIST=10000000
HIST_STAMPS="yyyy-mm-dd"
HISTORY_IGNORE="(ls|pwd|history|exit)*"
```