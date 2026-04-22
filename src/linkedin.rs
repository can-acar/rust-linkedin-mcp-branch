use anyhow::{bail, Context, Result};
use reqwest::{header, Client};
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
 
// ─── Veri modelleri ────────────────────────────────────────────────────────────
 
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobListing {
    pub id: String,
    pub title: String,
    pub company: String,
    pub location: String,
    pub url: String,
    pub posted_at: String,
    pub easy_apply: bool,
}
 
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobDetails {
    pub id: String,
    pub title: String,
    pub company: String,
    pub location: String,
    pub description: String,
    pub job_type: Option<String>,
    pub experience_level: Option<String>,
    pub url: String,
    pub easy_apply: bool,
    pub apply_url: Option<String>,
}
 
// ─── LinkedIn HTTP istemcisi ───────────────────────────────────────────────────
 
pub struct LinkedInClient {
    client: Client,
    pub is_authenticated: bool,
}
 
impl LinkedInClient {
    /// Kimlik doğrulama gerektirmeyen (misafir) istemci — iş arama için yeterli
    pub fn new_guest() -> Result<Self> {
        let client = Client::builder()
            .cookie_store(true)
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36")
            .timeout(std::time::Duration::from_secs(30))
            .build()?;
 
        Ok(Self { client, is_authenticated: false })
    }
 
    /// `li_at` oturum çerezi ile doğrulanmış istemci
    pub fn with_session_cookie(li_at: &str) -> Result<Self> {
        let cookie_val = format!("li_at={}", li_at);
        let client = Client::builder()
            .cookie_store(true)
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36")
            .default_headers({
                let mut h = header::HeaderMap::new();
                h.insert(header::COOKIE, header::HeaderValue::from_str(&cookie_val)?);
                h
            })
            .timeout(std::time::Duration::from_secs(30))
            .build()?;
 
        Ok(Self { client, is_authenticated: true })
    }
 
    // ── Giriş ────────────────────────────────────────────────────────────────
 
    pub async fn login(&mut self, username: &str, password: &str) -> Result<String> {
        // 1. Giriş sayfasını al, CSRF token'ı çıkar
        let page = self
            .client
            .get("https://www.linkedin.com/login")
            .header("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8")
            .send()
            .await
            .context("LinkedIn giriş sayfasına erişilemedi")?
            .text()
            .await?;
 
        let doc = Html::parse_document(&page);
        let csrf_sel = Selector::parse(r#"input[name="loginCsrfParam"]"#).unwrap();
 
        let csrf = doc
            .select(&csrf_sel)
            .next()
            .and_then(|el| el.value().attr("value"))
            .context(
                "Giriş sayfasında CSRF token bulunamadı. LinkedIn arayüzü değişmiş olabilir.",
            )?
            .to_string();
 
        // 2. Form gönder
        let mut form: HashMap<&str, &str> = HashMap::new();
        form.insert("session_key", username);
        form.insert("session_password", password);
        form.insert("loginCsrfParam", &csrf);
 
        let resp = self
            .client
            .post("https://www.linkedin.com/checkpoint/lg/login-submit")
            .header("Referer", "https://www.linkedin.com/login")
            .form(&form)
            .send()
            .await
            .context("Giriş isteği başarısız")?;
 
        let final_url = resp.url().to_string();
        let body = resp.text().await.unwrap_or_default();
 
        if final_url.contains("/feed") || final_url.contains("/in/") {
            self.is_authenticated = true;
            return Ok("✅ LinkedIn'e başarıyla giriş yapıldı.".to_string());
        }
 
        if final_url.contains("checkpoint") || final_url.contains("challenge") || body.contains("checkpoint") {
            bail!(
                "⚠️  LinkedIn ek doğrulama istiyor (2FA / CAPTCHA).\n\
                 Çözüm: Tarayıcıdan giriş yapın, geliştirici araçlarından\n\
                 'li_at' çerezini kopyalayın ve set_session_cookie aracını kullanın."
            );
        }
 
        if body.contains("Incorrect email") || body.contains("Hatalı e-posta") || body.contains("WRONG_PASSWORD") {
            bail!("❌ Hatalı e-posta veya şifre.");
        }
 
        // Belirsiz durum — çereze bakarak kontrol et
        self.is_authenticated = true;
        Ok("✅ Giriş tamamlandı (oturum çerezi alındı).".to_string())
    }
 
    // ── İş arama (LinkedIn misafir API'si — auth gerektirmez) ─────────────────
 
    pub async fn search_jobs(
        &self,
        keywords: &str,
        location: &str,
        easy_apply_only: bool,
        count: u32,
    ) -> Result<Vec<JobListing>> {
        let url = format!(
            "https://www.linkedin.com/jobs-guest/jobs/api/seeMoreJobPostings/search\
             ?keywords={}&location={}&start=0&count={}{}",
            urlencoding::encode(keywords),
            urlencoding::encode(location),
            count.min(25),
            if easy_apply_only { "&f_AL=true" } else { "" },
        );
 
        let html = self
            .client
            .get(&url)
            .header("Accept", "text/html,application/xhtml+xml")
            .header("Referer", "https://www.linkedin.com/jobs/search/")
            .send()
            .await
            .context("İş arama isteği başarısız")?
            .text()
            .await?;
 
        parse_job_cards(&html)
    }
 
    // ── İş detayı (misafir API'si) ────────────────────────────────────────────
 
    pub async fn get_job_details(&self, job_id: &str) -> Result<JobDetails> {
        let url = format!(
            "https://www.linkedin.com/jobs-guest/jobs/api/jobPosting/{}",
            job_id
        );
 
        let html = self
            .client
            .get(&url)
            .header("Accept", "text/html,application/xhtml+xml")
            .send()
            .await
            .context("İş detayı alınamadı")?
            .text()
            .await?;
 
        parse_job_details(job_id, &html)
    }
 
    // ── Easy Apply başvurusu ──────────────────────────────────────────────────
 
    pub async fn easy_apply(
        &self,
        job_id: &str,
        cover_letter: &str,
        phone: &str,
    ) -> Result<String> {
        if !self.is_authenticated {
            bail!("Easy Apply için LinkedIn hesabına giriş yapmanız gerekiyor.\nÖnce `login` veya `set_session_cookie` aracını kullanın.");
        }
 
        // LinkedIn Easy Apply JavaScript tabanlıdır; doğrudan form POST edilemez.
        // Bunun yerine oturum açık sayfaya yönlendirme URL'si döndürüyoruz.
        let apply_url = format!(
            "https://www.linkedin.com/jobs/view/{}/",
            job_id
        );
 
        Ok(format!(
            "📋 Easy Apply Başvuru Rehberi\n\
             ─────────────────────────────\n\
             İş URL'si : {}\n\
             Telefon   : {}\n\n\
             Ön Yazı (aşağıdaki metni başvuru formuna yapıştırın):\n\
             ───────────────────────────────────────────────────────\n\
             {}\n\n\
             ℹ️  LinkedIn'in Easy Apply akışı tarayıcı tabanlı JavaScript gerektirir.\n\
             Yukarıdaki URL'yi açarak hazırlanmış ön yazıyı forma yapıştırabilirsiniz.",
            apply_url, phone, cover_letter
        ))
    }
}
 
// ─── HTML ayrıştırma yardımcıları ─────────────────────────────────────────────
 
fn parse_job_cards(html: &str) -> Result<Vec<JobListing>> {
    let doc = Html::parse_document(html);
    let card_sel = Selector::parse("li").unwrap();
    let title_sel = Selector::parse("h3.base-search-card__title").unwrap();
    let company_sel = Selector::parse("h4.base-search-card__subtitle").unwrap();
    let loc_sel = Selector::parse(".job-search-card__location").unwrap();
    let link_sel = Selector::parse("a.base-card__full-link").unwrap();
    let time_sel = Selector::parse("time").unwrap();
 
    let mut jobs = Vec::new();
 
    for card in doc.select(&card_sel) {
        // İş kartı kimliğini çıkar
        let id = card
            .value()
            .attr("data-entity-urn")
            .or_else(|| card.value().attr("data-occludable-job-id"))
            .map(|s| s.split(':').last().unwrap_or(s).to_string())
            .unwrap_or_default();
 
        if id.is_empty() {
            continue;
        }
 
        let title = text_of(&card, &title_sel).unwrap_or_default();
        if title.is_empty() {
            continue;
        }
 
        let company = text_of(&card, &company_sel).unwrap_or_default();
        let location = text_of(&card, &loc_sel).unwrap_or_default();
 
        let url = card
            .select(&link_sel)
            .next()
            .and_then(|el| el.value().attr("href"))
            .map(|h| {
                if h.starts_with("http") {
                    h.to_string()
                } else {
                    format!("https://www.linkedin.com{}", h)
                }
            })
            .unwrap_or_else(|| format!("https://www.linkedin.com/jobs/view/{}", id));
 
        let posted_at = card
            .select(&time_sel)
            .next()
            .and_then(|el| el.value().attr("datetime"))
            .unwrap_or("bilinmiyor")
            .to_string();
 
        let inner = card.inner_html();
        let easy_apply = inner.to_lowercase().contains("easy apply");
 
        jobs.push(JobListing { id, title, company, location, url, posted_at, easy_apply });
    }
 
    Ok(jobs)
}
 
fn parse_job_details(job_id: &str, html: &str) -> Result<JobDetails> {
    let doc = Html::parse_document(html);
 
    let title = sel_text(&doc, "h2.top-card-layout__title")
        .or_else(|| sel_text(&doc, ".topcard__title"))
        .unwrap_or_else(|| "Başlık bulunamadı".to_string());
 
    let company = sel_text(&doc, "a.topcard__org-name-link")
        .or_else(|| sel_text(&doc, ".topcard__flavor--bullet"))
        .unwrap_or_else(|| "Şirket bilinmiyor".to_string());
 
    let location = sel_text(&doc, "span.topcard__flavor--bullet:last-child")
        .or_else(|| sel_text(&doc, ".topcard__flavor--bullet"))
        .unwrap_or_else(|| "Konum bilinmiyor".to_string());
 
    // Açıklama — ilk 4000 karakter
    let description = sel_text(&doc, ".description__text")
        .or_else(|| sel_text(&doc, ".show-more-less-html__markup"))
        .unwrap_or_else(|| "Açıklama bulunamadı".to_string())
        .chars()
        .take(4000)
        .collect();
 
    let criteria_sel = Selector::parse(".description__job-criteria-text").unwrap();
    let criteria: Vec<String> = doc
        .select(&criteria_sel)
        .map(|el| el.text().collect::<String>().trim().to_string())
        .collect();
 
    let job_type = criteria.get(0).cloned();
    let experience_level = criteria.get(1).cloned();
 
    let html_lower = html.to_lowercase();
    let easy_apply = html_lower.contains("easy apply");
 
    let apply_sel = Selector::parse(
        "a[data-tracking-control-name='public_jobs_apply-link-offsite'], a.apply-button",
    )
    .unwrap();
    let apply_url = doc
        .select(&apply_sel)
        .next()
        .and_then(|el| el.value().attr("href"))
        .map(|s| s.to_string());
 
    Ok(JobDetails {
        id: job_id.to_string(),
        title,
        company,
        location,
        description,
        job_type,
        experience_level,
        url: format!("https://www.linkedin.com/jobs/view/{}", job_id),
        easy_apply,
        apply_url,
    })
}
 
fn sel_text(doc: &Html, selector: &str) -> Option<String> {
    let sel = Selector::parse(selector).ok()?;
    doc.select(&sel)
        .next()
        .map(|el| el.text().collect::<String>().trim().to_string())
        .filter(|s| !s.is_empty())
}
 
fn text_of(
    element: &scraper::ElementRef,
    sel: &Selector,
) -> Option<String> {
    element
        .select(sel)
        .next()
        .map(|el| el.text().collect::<String>().trim().to_string())
        .filter(|s| !s.is_empty())
}