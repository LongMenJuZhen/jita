use anyhow::Result;
use std::collections::HashMap;

const SERVICE_NAME: &str = "jita";

pub struct SettingsManager;

impl SettingsManager {
    pub fn new() -> Self {
        Self
    }

    pub fn get(&self, key: &str) -> Result<Option<String>> {
        let entry = keyring::Entry::new(SERVICE_NAME, key)?;
        match entry.get_password() {
            Ok(password) => Ok(Some(password)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn set(&self, key: &str, value: &str) -> Result<()> {
        let entry = keyring::Entry::new(SERVICE_NAME, key)?;
        entry.set_password(value)?;
        Ok(())
    }

    pub fn delete(&self, key: &str) -> Result<()> {
        let entry = keyring::Entry::new(SERVICE_NAME, key)?;
        entry.delete_credential()?;
        Ok(())
    }

    pub fn list_keys(&self) -> Result<Vec<String>> {
        // keyring crate doesn't provide a way to list all keys for a service
        // We store an index in the database for this
        Ok(Vec::new())
    }
}

#[derive(Debug, Clone, Default)]
pub struct AiConfig {
    pub api_key: String,
    pub model: String,
    pub api_base: Option<String>,
}

impl AiConfig {
    pub fn load(settings: &SettingsManager) -> Result<Self> {
        // Environment variables take precedence over keyring
        let api_key = std::env::var("JITA_API_KEY")
            .ok()
            .or_else(|| settings.get("ai_api_key").ok().flatten())
            .unwrap_or_default();

        let model = std::env::var("JITA_MODEL")
            .ok()
            .or_else(|| settings.get("ai_model").ok().flatten())
            .unwrap_or_else(|| "claude-sonnet-4-6".to_string());

        let api_base = std::env::var("JITA_API_BASE")
            .ok()
            .or_else(|| settings.get("ai_api_base").ok().flatten());

        Ok(Self {
            api_key,
            model,
            api_base,
        })
    }

    pub fn save(&self, settings: &SettingsManager) -> Result<()> {
        if !self.api_key.is_empty() {
            settings.set("ai_api_key", &self.api_key)?;
        }
        settings.set("ai_model", &self.model)?;
        if let Some(ref base) = self.api_base {
            settings.set("ai_api_base", base)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default)]
pub struct AppSettings {
    pub ai: AiConfig,
    pub hotkey: String,
    pub asr_enabled: bool,
    pub asr_model_path: Option<String>,
    pub asr_keep_in_memory: bool,
}

impl AppSettings {
    pub fn load(settings: &SettingsManager) -> Result<Self> {
        Ok(Self {
            ai: AiConfig::load(settings)?,
            hotkey: settings.get("hotkey")?.unwrap_or_else(|| "Ctrl+Space".to_string()),
            asr_enabled: settings.get("asr_enabled")?.map(|s| s == "true").unwrap_or(false),
            asr_model_path: settings.get("asr_model_path")?,
            asr_keep_in_memory: settings.get("asr_keep_in_memory")?.map(|s| s == "true").unwrap_or(true),
        })
    }

    pub fn save(&self, settings: &SettingsManager) -> Result<()> {
        self.ai.save(settings)?;
        settings.set("hotkey", &self.hotkey)?;
        settings.set("asr_enabled", &self.asr_enabled.to_string())?;
        if let Some(ref path) = self.asr_model_path {
            settings.set("asr_model_path", path)?;
        }
        settings.set("asr_keep_in_memory", &self.asr_keep_in_memory.to_string())?;
        Ok(())
    }
}
