use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
 
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Credentials {
    pub username: Option<String>,
    pub password: Option<String>,
    /// LinkedIn `li_at` oturum çerezi — doğrudan tarayıcıdan alınabilir
    pub session_cookie: Option<String>,
    /// Kullanıcının kendi adı ve kısa özgeçmişi (ön yazı için)
    pub profile_summary: Option<String>,
}
 
impl Credentials {
    fn config_path() -> Result<PathBuf> {
        let dir = dirs::config_dir()
            .context("Yapılandırma dizini bulunamadı")?
            .join("linkedin-mcp");
        std::fs::create_dir_all(&dir)?;
        Ok(dir.join("credentials.json"))
    }
 
    pub fn load() -> Self {
        Self::try_load().unwrap_or_default()
    }
 
    fn try_load() -> Result<Self> {
        let path = Self::config_path()?;
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(&path)?;
        Ok(serde_json::from_str(&content)?)
    }
 
    pub fn save(&self) -> Result<()> {
        let path = Self::config_path()?;
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, &content)?;
 
        // Unix'te dosya izinlerini 600 yap (sadece sahip okuyabilsin)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
        }
 
        Ok(())
    }
 
    pub fn storage_path_display() -> String {
        Self::config_path()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| "~/.config/linkedin-mcp/credentials.json".to_string())
    }
}