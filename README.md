# LinkedIn MCP Server

LinkedIn is arama ve basvuru islemlerini Claude Desktop uzerinden gerceklestiren bir MCP (Model Context Protocol) sunucusu.

## Ozellikler

| Arac | Aciklama |
|------|----------|
| `set_credentials` | LinkedIn e-posta ve sifresini yerel olarak kaydeder |
| `set_session_cookie` | `li_at` oturum cerezini kaydeder (2FA/CAPTCHA durumunda) |
| `login` | Kaydedilmis bilgilerle LinkedIn'e giris yapar |
| `search_jobs` | Anahtar kelime ve konuma gore is ilani arar |
| `get_job_details` | Belirli bir ilanin detaylarini getirir |
| `apply_to_job` | Easy Apply ile basvuru rehberi ve on yazi olusturur |
| `get_status` | Sunucu durumunu gosterir |

## Kurulum

### Gereksinimler

- Rust toolchain (1.70+)
- Claude Desktop

### Build

```bash
cargo build --release
```

### Claude Desktop Konfigurasyonu

`~/Library/Application Support/Claude/claude_desktop_config.json` dosyasina ekleyin:

```json
{
  "mcpServers": {
    "linkedin": {
      "command": "/PROJE/YOLU/target/release/linkedin-mcp",
      "args": []
    }
  }
}
```

`/PROJE/YOLU` kismini gercek proje dizini ile degistirin.

## Kullanim

### Tipik Akis

1. **Kimlik bilgilerini kaydedin** — `set_credentials` ile e-posta ve sifre
2. **Giris yapin** — `login` araci ile (veya 2FA varsa `set_session_cookie`)
3. **Is arayin** — `search_jobs` ile anahtar kelime ve konum belirtin
4. **Detaylari inceleyin** — `get_job_details` ile ilan bilgilerini alin
5. **Basvurun** — `apply_to_job` ile on yazi olusturup Easy Apply rehberini takip edin

### Kimlik Dogrulama

- **Misafir modu:** Is arama ve detay goruntuleme icin giris gerektirmez
- **Oturum acik mod:** Easy Apply basvurulari icin gerekli
- **2FA/CAPTCHA:** Tarayicidan `li_at` cerezini kopyalayip `set_session_cookie` ile kullanin

### Kimlik Bilgisi Depolama

Bilgiler `~/.config/linkedin-mcp/credentials.json` dosyasinda saklanir.
Unix sistemlerde dosya izinleri `0600` olarak ayarlanir (yalnizca sahip erisebilir).

## Guvenlik

- Kimlik bilgileri yalnizca yerel diskde saklanir
- Sifreler yalnizca LinkedIn'e giris icin iletilir
- Oturum cerezleri dosya sistemi izinleriyle korunur
