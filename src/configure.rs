use anyhow::{anyhow, Result};
use serde_json::{Map, Value};
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

pub fn run() -> Result<()> {
    let path = crate::config::config_path();
    let mut root = load_config_value(&path)?;
    let mut dirty = false;

    println!("femtobot configure");
    println!("Config path: {}", path.display());
    println!();

    loop {
        println!("Menu:");
        println!("1. Configure provider (OpenRouter/OpenAI)");
        println!("2. Configure Telegram");
        println!("3. Configure model");
        println!("4. Configure web search (Brave)");
        println!("5. Configure transcription");
        println!("6. Show config path");
        println!("7. Save and exit");
        println!("8. Exit without saving");
        print!("Select an option: ");
        io::stdout().flush().ok();

        let choice = read_line()?.trim().to_string();
        println!();

        match choice.as_str() {
            "1" => {
                dirty |= configure_provider(&mut root)?;
            }
            "2" => {
                dirty |= configure_telegram(&mut root)?;
            }
            "3" => {
                dirty |= configure_model(&mut root)?;
            }
            "4" => {
                dirty |= configure_web_search(&mut root)?;
            }
            "5" => {
                dirty |= configure_transcription(&mut root)?;
            }
            "6" => {
                println!("Config path: {}", path.display());
            }
            "7" => {
                if dirty {
                    save_config_value(&path, &root)?;
                    println!("Saved.");
                } else {
                    println!("No changes to save.");
                }
                break;
            }
            "8" | "q" | "Q" => {
                if dirty {
                    println!("Exited without saving.");
                }
                break;
            }
            _ => {
                println!("Invalid option.");
            }
        }
        println!();
    }

    Ok(())
}

fn configure_provider(root: &mut Value) -> Result<bool> {
    let current_provider =
        get_str_at(root, &["agents", "defaults", "provider"]).unwrap_or("openrouter");
    let provider = prompt_with_current("Active provider (openrouter/openai)", current_provider)?;
    let provider = if provider.trim().is_empty() {
        current_provider.to_string()
    } else {
        provider.trim().to_ascii_lowercase()
    };

    if !matches!(provider.as_str(), "openrouter" | "openai") {
        println!("Invalid provider. Supported: openrouter, openai");
        return Ok(false);
    }
    let normalized = provider;

    set_path(
        root,
        &["agents", "defaults", "provider"],
        Value::String(normalized.clone()),
    )?;

    match normalized.as_str() {
        "openrouter" => {
            let current_key =
                get_str_at(root, &["providers", "openrouter", "apiKey"]).unwrap_or("");
            let current_base = get_str_at(root, &["providers", "openrouter", "apiBase"])
                .unwrap_or("https://openrouter.ai/api/v1");
            let key = prompt_secret("OpenRouter API key", current_key)?;
            let base = prompt_with_current("OpenRouter base URL", current_base)?;
            set_path(
                root,
                &["providers", "openrouter", "apiKey"],
                Value::String(key),
            )?;
            set_path(
                root,
                &["providers", "openrouter", "apiBase"],
                Value::String(base),
            )?;
        }
        "openai" => {
            let current_key = get_str_at(root, &["providers", "openai", "apiKey"]).unwrap_or("");
            let current_base = get_str_at(root, &["providers", "openai", "apiBase"])
                .unwrap_or("https://api.openai.com/v1");
            let key = prompt_secret("OpenAI API key", current_key)?;
            let base = prompt_with_current("OpenAI base URL", current_base)?;
            set_path(root, &["providers", "openai", "apiKey"], Value::String(key))?;
            set_path(
                root,
                &["providers", "openai", "apiBase"],
                Value::String(base),
            )?;
        }
        _ => {}
    }

    Ok(true)
}

fn configure_telegram(root: &mut Value) -> Result<bool> {
    let current_token = get_str_at(root, &["channels", "telegram", "token"]).unwrap_or("");
    let current_allow = get_array_at(root, &["channels", "telegram", "allow_from"]);
    let current_allow_str = if current_allow.is_empty() {
        String::new()
    } else {
        current_allow.join(",")
    };

    let token = prompt_secret("Telegram bot token", current_token)?;
    let allow_from = prompt_with_current(
        "Allowed Telegram user IDs (comma separated)",
        &current_allow_str,
    )?;

    let allow_list = if allow_from.trim().is_empty() {
        current_allow
    } else {
        allow_from
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect::<Vec<_>>()
    };

    set_path(
        root,
        &["channels", "telegram", "token"],
        Value::String(token),
    )?;
    set_path(
        root,
        &["channels", "telegram", "allow_from"],
        Value::Array(allow_list.into_iter().map(Value::String).collect()),
    )?;

    Ok(true)
}

fn configure_model(root: &mut Value) -> Result<bool> {
    let current_model =
        get_str_at(root, &["agents", "defaults", "model"]).unwrap_or("anthropic/claude-opus-4-5");
    let current_fallbacks = get_array_at(root, &["agents", "defaults", "model_fallbacks"]);
    let current_fallbacks_str = if current_fallbacks.is_empty() {
        String::new()
    } else {
        current_fallbacks.join(",")
    };
    let model = prompt_with_current("Default model", current_model)?;
    let fallbacks = prompt_with_current(
        "Fallback models (comma separated, e.g. openrouter/anthropic/claude-sonnet-4-5,openai/gpt-4o-mini)",
        &current_fallbacks_str,
    )?;

    let fallback_list = if fallbacks.trim().is_empty() {
        current_fallbacks
    } else {
        fallbacks
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect::<Vec<_>>()
    };

    set_path(root, &["agents", "defaults", "model"], Value::String(model))?;
    set_path(
        root,
        &["agents", "defaults", "model_fallbacks"],
        Value::Array(fallback_list.into_iter().map(Value::String).collect()),
    )?;
    Ok(true)
}

fn configure_web_search(root: &mut Value) -> Result<bool> {
    let current = get_str_at(root, &["tools", "web", "search", "apiKey"]).unwrap_or("");
    let key = prompt_secret("Brave API key", current)?;
    set_path(
        root,
        &["tools", "web", "search", "apiKey"],
        Value::String(key),
    )?;
    Ok(true)
}

fn configure_transcription(root: &mut Value) -> Result<bool> {
    let current_enabled =
        get_bool_at(root, &["channels", "telegram", "transcription", "enabled"]).unwrap_or(true);
    let current_provider = get_str_at(root, &["channels", "telegram", "transcription", "provider"])
        .unwrap_or("openai");
    let current_model = get_str_at(root, &["channels", "telegram", "transcription", "model"])
        .unwrap_or("whisper-1");
    let current_language =
        get_str_at(root, &["channels", "telegram", "transcription", "language"]).unwrap_or("");
    let current_max_bytes = get_u64_at(
        root,
        &["channels", "telegram", "transcription", "max_bytes"],
    )
    .unwrap_or(20 * 1024 * 1024);
    let current_diarize =
        get_bool_at(root, &["channels", "telegram", "transcription", "diarize"]).unwrap_or(false);
    let current_context_bias = get_str_at(
        root,
        &["channels", "telegram", "transcription", "context_bias"],
    )
    .unwrap_or("");
    let current_grans = get_array_at(
        root,
        &[
            "channels",
            "telegram",
            "transcription",
            "timestamp_granularities",
        ],
    );
    let current_grans_str = if current_grans.is_empty() {
        String::new()
    } else {
        current_grans.join(",")
    };

    let enabled_raw = prompt_with_current(
        "Enable transcription (true/false)",
        if current_enabled { "true" } else { "false" },
    )?;
    let enabled = parse_bool_input(&enabled_raw).unwrap_or(current_enabled);

    let provider_raw =
        prompt_with_current("Transcription provider (openai/mistral)", current_provider)?;
    let provider = if provider_raw.trim().is_empty() {
        current_provider.to_string()
    } else {
        provider_raw.trim().to_ascii_lowercase()
    };
    if !matches!(provider.as_str(), "openai" | "mistral") {
        println!("Invalid transcription provider. Supported: openai, mistral");
        return Ok(false);
    }

    let model = prompt_with_current("Transcription model", current_model)?;
    let language = prompt_with_current(
        "Transcription language (empty = auto-detect)",
        current_language,
    )?;
    let max_bytes_raw = prompt_with_current("Max audio bytes", &current_max_bytes.to_string())?;
    let max_bytes = max_bytes_raw.parse::<u64>().unwrap_or(current_max_bytes);

    let diarize_raw = prompt_with_current(
        "Enable diarization (Mistral only, true/false)",
        if current_diarize { "true" } else { "false" },
    )?;
    let diarize = parse_bool_input(&diarize_raw).unwrap_or(current_diarize);
    let context_bias = prompt_with_current(
        "Context bias (Mistral only, comma-separated terms)",
        current_context_bias,
    )?;
    let grans_raw = prompt_with_current(
        "Timestamp granularities (Mistral only, comma-separated e.g. segment,word)",
        &current_grans_str,
    )?;
    let grans = if grans_raw.trim().is_empty() {
        current_grans
    } else {
        grans_raw
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect::<Vec<_>>()
    };

    set_path(
        root,
        &["channels", "telegram", "transcription", "enabled"],
        Value::Bool(enabled),
    )?;
    set_path(
        root,
        &["channels", "telegram", "transcription", "provider"],
        Value::String(provider.clone()),
    )?;
    set_path(
        root,
        &["channels", "telegram", "transcription", "model"],
        Value::String(model),
    )?;
    set_path(
        root,
        &["channels", "telegram", "transcription", "language"],
        Value::String(language),
    )?;
    set_path(
        root,
        &["channels", "telegram", "transcription", "max_bytes"],
        Value::Number(serde_json::Number::from(max_bytes)),
    )?;
    set_path(
        root,
        &["channels", "telegram", "transcription", "diarize"],
        Value::Bool(diarize),
    )?;
    set_path(
        root,
        &["channels", "telegram", "transcription", "context_bias"],
        Value::String(context_bias),
    )?;
    set_path(
        root,
        &[
            "channels",
            "telegram",
            "transcription",
            "timestamp_granularities",
        ],
        Value::Array(grans.into_iter().map(Value::String).collect()),
    )?;

    if provider == "mistral" {
        let current_key = get_str_at(root, &["providers", "mistral", "apiKey"]).unwrap_or("");
        let current_base = get_str_at(root, &["providers", "mistral", "apiBase"])
            .unwrap_or("https://api.mistral.ai/v1");
        let key = prompt_secret("Mistral API key", current_key)?;
        let base = prompt_with_current("Mistral base URL", current_base)?;
        set_path(
            root,
            &["providers", "mistral", "apiKey"],
            Value::String(key),
        )?;
        set_path(
            root,
            &["providers", "mistral", "apiBase"],
            Value::String(base),
        )?;
    }

    Ok(true)
}

fn load_config_value(path: &PathBuf) -> Result<Value> {
    if path.exists() {
        let content = fs::read_to_string(path)?;
        let parsed: Value = serde_json::from_str(&content)
            .map_err(|e| anyhow!("failed to parse config at {}: {e}", path.display()))?;
        if !parsed.is_object() {
            return Err(anyhow!(
                "invalid config at {}: root must be a JSON object",
                path.display()
            ));
        }
        Ok(parsed)
    } else {
        Ok(Value::Object(Map::new()))
    }
}

fn save_config_value(path: &PathBuf, value: &Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let content = serde_json::to_string_pretty(value)?;
    fs::write(path, content)?;
    Ok(())
}

fn prompt_with_current(label: &str, current: &str) -> Result<String> {
    prompt_with_options(label, current, false)
}

fn prompt_secret(label: &str, current: &str) -> Result<String> {
    prompt_with_options(label, current, true)
}

fn prompt_with_options(label: &str, current: &str, secret: bool) -> Result<String> {
    match (secret, current.trim().is_empty()) {
        (true, true) => print!("{label}: "),
        (true, false) => print!("{label} [set]: "),
        (false, true) => print!("{label}: "),
        (false, false) => print!("{label} [{current}]: "),
    }
    io::stdout().flush().ok();
    let input = read_line()?.trim().to_string();
    if input.is_empty() && !current.trim().is_empty() {
        Ok(current.to_string())
    } else {
        Ok(input)
    }
}

fn read_line() -> Result<String> {
    let mut buf = String::new();
    io::stdin().read_line(&mut buf)?;
    Ok(buf)
}

fn set_path(value: &mut Value, path: &[&str], new_value: Value) -> Result<()> {
    if path.is_empty() {
        return Ok(());
    }
    if !value.is_object() {
        return Err(anyhow!("invalid config: root must be a JSON object"));
    }
    let mut cur = value;
    for (idx, key) in path[..path.len() - 1].iter().enumerate() {
        let obj = cur
            .as_object_mut()
            .ok_or_else(|| anyhow!("invalid config: '{}' must be an object", path[..idx].join(".")))?;
        cur = obj
            .entry((*key).to_string())
            .or_insert_with(|| Value::Object(Map::new()));
        if !cur.is_object() {
            return Err(anyhow!(
                "invalid config: '{}' must be an object",
                path[..=idx].join(".")
            ));
        }
    }
    let obj = cur.as_object_mut().ok_or_else(|| {
        anyhow!(
            "invalid config: '{}' must be an object",
            path[..path.len() - 1].join(".")
        )
    })?;
    obj.insert(path[path.len() - 1].to_string(), new_value);
    Ok(())
}

fn get_str_at<'a>(value: &'a Value, path: &[&str]) -> Option<&'a str> {
    let mut cur = value;
    for key in path {
        cur = cur.get(*key)?;
    }
    cur.as_str()
}

fn get_array_at(value: &Value, path: &[&str]) -> Vec<String> {
    let mut cur = value;
    for key in path {
        match cur.get(*key) {
            Some(v) => cur = v,
            None => return Vec::new(),
        }
    }
    match cur.as_array() {
        Some(arr) => arr
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect(),
        None => Vec::new(),
    }
}

fn get_bool_at(value: &Value, path: &[&str]) -> Option<bool> {
    let mut cur = value;
    for key in path {
        cur = cur.get(*key)?;
    }
    cur.as_bool()
}

fn get_u64_at(value: &Value, path: &[&str]) -> Option<u64> {
    let mut cur = value;
    for key in path {
        cur = cur.get(*key)?;
    }
    cur.as_u64()
}

fn parse_bool_input(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "y" => Some(true),
        "0" | "false" | "no" | "n" => Some(false),
        _ => None,
    }
}
