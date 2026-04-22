use crate::credentials::Credentials;
use crate::linkedin::LinkedInClient;
use anyhow::{bail, Result};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::Mutex;
 
// ─── Sunucu durumu ─────────────────────────────────────────────────────────────
 
pub struct ServerState {
    pub creds: Credentials,
    pub client: Option<LinkedInClient>,
}
 
impl ServerState {
    pub async fn new() -> Self {
        let creds = Credentials::load();
        // Kayıtlı oturum çerezi varsa istemciyi hazırla
        let client = creds
            .session_cookie
            .as_deref()
            .and_then(|c| LinkedInClient::with_session_cookie(c).ok());
 
        Self { creds, client }
    }
}
 
// ─── Tool şemaları (Claude'a gösterilen açıklamalar) ──────────────────────────
 
pub fn get_tools() -> Value {
    json!([
        {
            "name": "set_credentials",
            "description": "LinkedIn e-posta ve şifresini yerel olarak güvenli biçimde kaydeder. İsteğe bağlı olarak profil özeti (ön yazı için) da eklenebilir.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "username": { "type": "string", "description": "LinkedIn e-posta adresi" },
                    "password": { "type": "string", "description": "LinkedIn şifresi" },
                    "profile_summary": {
                        "type": "string",
                        "description": "Ön yazıda kullanılacak kısa özgeçmiş (opsiyonel, örn: '5 yıllık Rust geliştirici, dağıtık sistemler uzmanı')"
                    }
                },
                "required": ["username", "password"]
            }
        },
        {
            "name": "set_session_cookie",
            "description": "LinkedIn `li_at` oturum çerezini kaydeder. 2FA veya CAPTCHA ile karşılaşıldığında tarayıcı üzerinden giriş yapıp bu çerezi kullanın.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "cookie": { "type": "string", "description": "Tarayıcıdan kopyalanan li_at çerez değeri" }
                },
                "required": ["cookie"]
            }
        },
        {
            "name": "login",
            "description": "Kaydedilmiş kullanıcı adı ve şifre ile LinkedIn'e giriş yapar.",
            "inputSchema": {
                "type": "object",
                "properties": {}
            }
        },
        {
            "name": "search_jobs",
            "description": "LinkedIn'de iş ilanı arar ve liste döndürür.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "keywords": { "type": "string", "description": "Arama terimi (örn: 'Rust Developer', 'Backend Engineer')" },
                    "location": { "type": "string", "description": "Konum (örn: 'Istanbul', 'Remote', 'Germany')" },
                    "easy_apply_only": {
                        "type": "boolean",
                        "description": "Yalnızca Easy Apply ilanlarını getir (varsayılan: false)"
                    },
                    "count": {
                        "type": "integer",
                        "description": "Getirilecek ilan sayısı (1-25, varsayılan: 10)",
                        "minimum": 1,
                        "maximum": 25
                    }
                },
                "required": ["keywords", "location"]
            }
        },
        {
            "name": "get_job_details",
            "description": "Belirli bir iş ilanının tüm detaylarını (açıklama, gereksinimler, şirket) getirir.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "job_id": { "type": "string", "description": "LinkedIn iş ilanı ID'si" }
                },
                "required": ["job_id"]
            }
        },
        {
            "name": "apply_to_job",
            "description": "Belirtilen iş ilanına ön yazı ve iletişim bilgisiyle başvurur. Easy Apply ilanları için detaylı rehber sağlar.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "job_id": { "type": "string", "description": "Başvurulacak iş ilanı ID'si" },
                    "cover_letter": { "type": "string", "description": "Kişiselleştirilmiş ön yazı metni" },
                    "phone": { "type": "string", "description": "İletişim telefon numarası" }
                },
                "required": ["job_id", "cover_letter"]
            }
        },
        {
            "name": "get_status",
            "description": "MCP sunucusunun mevcut durumunu ve kayıtlı kimlik bilgilerini gösterir.",
            "inputSchema": {
                "type": "object",
                "properties": {}
            }
        }
    ])
}
 
// ─── Tool çağrı yönlendirici ───────────────────────────────────────────────────
 
pub async fn call_tool(
    state: Arc<Mutex<ServerState>>,
    name: &str,
    args: Value,
) -> Result<String> {
    match name {
        "set_credentials"   => tool_set_credentials(state, args).await,
        "set_session_cookie" => tool_set_session_cookie(state, args).await,
        "login"             => tool_login(state).await,
        "search_jobs"       => tool_search_jobs(state, args).await,
        "get_job_details"   => tool_get_job_details(state, args).await,
        "apply_to_job"      => tool_apply_to_job(state, args).await,
        "get_status"        => tool_get_status(state).await,
        other               => bail!("Bilinmeyen araç: {}", other),
    }
}
 
// ─── Tool implementasyonları ───────────────────────────────────────────────────
 
async fn tool_set_credentials(
    state: Arc<Mutex<ServerState>>,
    args: Value,
) -> Result<String> {
    let username = str_arg(&args, "username")?;
    let password = str_arg(&args, "password")?;
    let profile = args.get("profile_summary")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
 
    let mut s = state.lock().await;
    s.creds.username = Some(username.clone());
    s.creds.password = Some(password);
    if profile.is_some() {
        s.creds.profile_summary = profile;
    }
    s.creds.save()?;
 
    Ok(format!(
        "✅ Kimlik bilgileri kaydedildi.\nKullanıcı: {}\nDepolama: {}\n\nArtık `login` aracını kullanarak giriş yapabilirsiniz.",
        username,
        Credentials::storage_path_display()
    ))
}
 
async fn tool_set_session_cookie(
    state: Arc<Mutex<ServerState>>,
    args: Value,
) -> Result<String> {
    let cookie = str_arg(&args, "cookie")?;
 
    let mut s = state.lock().await;
    s.creds.session_cookie = Some(cookie.clone());
    s.creds.save()?;
    s.client = Some(LinkedInClient::with_session_cookie(&cookie)?);
 
    Ok("✅ Oturum çerezi kaydedildi ve aktif edildi. LinkedIn'e bağlısınız.".to_string())
}
 
async fn tool_login(state: Arc<Mutex<ServerState>>) -> Result<String> {
    let (username, password) = {
        let s = state.lock().await;
        let u = s.creds.username.clone()
            .ok_or_else(|| anyhow::anyhow!("Önce `set_credentials` ile kullanıcı adı ve şifre kaydedin."))?;
        let p = s.creds.password.clone()
            .ok_or_else(|| anyhow::anyhow!("Önce `set_credentials` ile şifre kaydedin."))?;
        (u, p)
    };
 
    let mut client = LinkedInClient::new_guest()?;
    let msg = client.login(&username, &password).await?;
 
    let mut s = state.lock().await;
    s.client = Some(client);
    Ok(msg)
}
 
async fn tool_search_jobs(
    _state: Arc<Mutex<ServerState>>,
    args: Value,
) -> Result<String> {
    let keywords = str_arg(&args, "keywords")?;
    let location = str_arg(&args, "location")?;
    let easy_apply = args.get("easy_apply_only")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let count = args.get("count")
        .and_then(|v| v.as_u64())
        .unwrap_or(10) as u32;
 
    // Arama için kimlik doğrulaması zorunlu değil
    let client = LinkedInClient::new_guest()?;
    let jobs = client.search_jobs(&keywords, &location, easy_apply, count).await?;
 
    if jobs.is_empty() {
        return Ok(format!(
            "'{}' için {} konumunda hiç ilan bulunamadı.\nFarklı anahtar kelimeler veya konum deneyin.",
            keywords, location
        ));
    }
 
    let mut out = format!(
        "🔍 '{}' — {} ({} sonuç{})\n{}\n\n",
        keywords,
        location,
        jobs.len(),
        if easy_apply { ", Easy Apply" } else { "" },
        "─".repeat(50)
    );
 
    for (i, job) in jobs.iter().enumerate() {
        out.push_str(&format!(
            "{}. {}\n   🏢 {}  📍 {}  📅 {}{}\n   ID: {}  |  {}\n\n",
            i + 1,
            job.title,
            job.company,
            job.location,
            job.posted_at,
            if job.easy_apply { "  ✅ Easy Apply" } else { "" },
            job.id,
            job.url
        ));
    }
 
    out.push_str("💡 Detay için: get_job_details(job_id)  |  Başvuru için: apply_to_job(job_id, cover_letter)");
    Ok(out)
}
 
async fn tool_get_job_details(
    _state: Arc<Mutex<ServerState>>,
    args: Value,
) -> Result<String> {
    let job_id = str_arg(&args, "job_id")?;
    let client = LinkedInClient::new_guest()?;
    let job = client.get_job_details(&job_id).await?;
 
    let mut out = format!(
        "📋 {} — {}\n{}\n\n",
        job.title, job.company, "─".repeat(50)
    );
    out.push_str(&format!("📍 Konum    : {}\n", job.location));
    if let Some(jt) = &job.job_type {
        out.push_str(&format!("💼 Tür      : {}\n", jt));
    }
    if let Some(exp) = &job.experience_level {
        out.push_str(&format!("🎯 Deneyim  : {}\n", exp));
    }
    out.push_str(&format!("🔗 URL      : {}\n", job.url));
    out.push_str(&format!(
        "✅ Easy Apply: {}\n",
        if job.easy_apply { "Evet" } else { "Hayır" }
    ));
    if let Some(url) = &job.apply_url {
        out.push_str(&format!("🌐 Başvuru  : {}\n", url));
    }
    out.push_str(&format!("\n📝 Açıklama:\n{}\n", job.description));
    out.push_str(&format!(
        "\n💡 Ön yazı için Claude'a şunu söyleyin:\n\
         \"Yukarıdaki iş ilanına başvurmak istiyorum, benim için ön yazı yazar mısın?\"\n\
         Profil özetini kaydetmek için set_credentials(profile_summary='...') kullanın."
    ));
 
    Ok(out)
}
 
async fn tool_apply_to_job(
    state: Arc<Mutex<ServerState>>,
    args: Value,
) -> Result<String> {
    let job_id = str_arg(&args, "job_id")?;
    let cover_letter = str_arg(&args, "cover_letter")?;
    let phone = args.get("phone")
        .and_then(|v| v.as_str())
        .unwrap_or("Belirtilmedi")
        .to_string();
 
    let _is_auth = {
        let s = state.lock().await;
        s.client.as_ref().map(|c| c.is_authenticated).unwrap_or(false)
    };
 
    let client = LinkedInClient::new_guest()?;
    client.easy_apply(&job_id, &cover_letter, &phone).await
}
 
async fn tool_get_status(state: Arc<Mutex<ServerState>>) -> Result<String> {
    let s = state.lock().await;
    let auth = s.client.as_ref().map(|c| c.is_authenticated).unwrap_or(false);
 
    Ok(format!(
        "🤖 LinkedIn MCP Sunucu Durumu\n{}\n\
         Kimlik bilgileri : {}\n\
         Oturum çerezi   : {}\n\
         Giriş durumu    : {}\n\
         Profil özeti    : {}\n\
         Depolama yolu   : {}",
        "─".repeat(40),
        if s.creds.username.is_some() { "✅ Kayıtlı" } else { "❌ Yok" },
        if s.creds.session_cookie.is_some() { "✅ Kayıtlı" } else { "❌ Yok" },
        if auth { "✅ Giriş yapıldı" } else { "⚠️  Giriş yapılmadı" },
        s.creds.profile_summary.as_deref().unwrap_or("Kayıtlı değil"),
        Credentials::storage_path_display()
    ))
}
 
// ─── Yardımcı ─────────────────────────────────────────────────────────────────
 
fn str_arg<'a>(args: &'a Value, key: &str) -> Result<String> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow::anyhow!("Eksik parametre: '{}'", key))
}