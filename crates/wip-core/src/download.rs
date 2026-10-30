//! Official Windows downloads via Microsoft's Media Creation Tool catalog
//! (products.cab → products.xml): list ESD files, validate URLs and names,
//! and classify the images inside a downloaded ESD.

use crate::ImageSummary;
use serde::{Deserialize, Serialize};

/// Media Creation Tool catalogs published by Microsoft.
pub const CATALOG_WIN11: &str = "https://go.microsoft.com/fwlink/?LinkId=2156292";
pub const CATALOG_WIN10: &str = "https://go.microsoft.com/fwlink/?LinkId=841361";

/// Hosts WinISOPilot will download from.
const ALLOWED_HOSTS: &[&str] = &["dl.delivery.mp.microsoft.com", "download.microsoft.com", "go.microsoft.com"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EsdFile {
    pub file_name: String,
    pub language_code: String,
    pub language: String,
    pub edition: String,
    pub architecture: String,
    pub size: u64,
    pub sha1: Option<String>,
    pub sha256: Option<String>,
    pub url: String,
}

impl EsdFile {
    /// Windows build number encoded in the file name (e.g. `26100.…`).
    pub fn build(&self) -> Option<u32> {
        self.file_name.split('.').find_map(|p| p.parse::<u32>().ok().filter(|n| *n >= 10240 && *n < 100000))
    }

    pub fn label(&self) -> String {
        format!(
            "{} · {} · {} · {}",
            if self.language.is_empty() { &self.language_code } else { &self.language },
            self.architecture,
            self.edition,
            crate::human_bytes(self.size)
        )
    }

    /// The hash to verify against: SHA-256 when published, else SHA-1.
    pub fn expected_hash(&self) -> Option<(&'static str, String)> {
        match (&self.sha256, &self.sha1) {
            (Some(h), _) if is_hex(h, 64) => Some(("SHA256", h.to_ascii_uppercase())),
            (_, Some(h)) if is_hex(h, 40) => Some(("SHA1", h.to_ascii_uppercase())),
            _ => None,
        }
    }
}

fn is_hex(s: &str, len: usize) -> bool {
    s.len() == len && s.bytes().all(|b| b.is_ascii_hexdigit())
}

fn unescape(s: &str) -> String {
    s.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&amp;", "&")
}

/// Text of the first `<name>…</name>` child inside `block`.
fn tag(block: &str, name: &str) -> Option<String> {
    let open = format!("<{name}>");
    let close = format!("</{name}>");
    let start = block.find(&open)? + open.len();
    let end = block[start..].find(&close)? + start;
    let v = unescape(block[start..end].trim());
    (!v.is_empty()).then_some(v)
}

/// True when `bytes` start with the Microsoft Cabinet signature.
pub fn is_cab(bytes: &[u8]) -> bool {
    bytes.starts_with(b"MSCF")
}

/// Decodes catalog text: UTF-8 (with or without BOM) or UTF-16 with a BOM.
pub fn decode_text(bytes: &[u8]) -> String {
    let utf16 = |b: &[u8], le: bool| {
        let units: Vec<u16> = b
            .chunks_exact(2)
            .map(|c| if le { u16::from_le_bytes([c[0], c[1]]) } else { u16::from_be_bytes([c[0], c[1]]) })
            .collect();
        String::from_utf16_lossy(&units)
    };
    match bytes {
        [0xFF, 0xFE, rest @ ..] => utf16(rest, true),
        [0xFE, 0xFF, rest @ ..] => utf16(rest, false),
        _ => String::from_utf8_lossy(bytes).trim_start_matches('\u{feff}').to_string(),
    }
}

/// True when the text looks like a Media Creation Tool product list.
pub fn looks_like_catalog(text: &str) -> bool {
    text.contains("<MCT") && text.contains("<File")
}

/// Parses products.xml. Entries without a name, URL or size are skipped;
/// duplicates (same file name) are dropped.
pub fn parse_products(xml: &str) -> Result<Vec<EsdFile>, String> {
    let xml = xml.trim_start_matches('\u{feff}');
    if !xml.contains("<File") {
        return Err("The Microsoft catalog didn't contain any downloads.".into());
    }
    let mut out: Vec<EsdFile> = Vec::new();
    let mut rest = xml;
    while let Some(i) = rest.find("<File") {
        let after = &rest[i + 5..];
        // `<File>` or `<File id=…>`, not `<FileName>` / `<FilePath>`.
        if !after.starts_with('>') && !after.starts_with(' ') {
            rest = after;
            continue;
        }
        let Some(end) = after.find("</File>") else { break };
        let block = &after[..end];
        rest = &after[end + 7..];
        let (Some(file_name), Some(url)) = (tag(block, "FileName"), tag(block, "FilePath")) else { continue };
        let Some(size) = tag(block, "Size").and_then(|s| s.parse::<u64>().ok()) else { continue };
        if !file_name.to_ascii_lowercase().ends_with(".esd") || out.iter().any(|e| e.file_name == file_name) {
            continue;
        }
        out.push(EsdFile {
            file_name,
            language_code: tag(block, "LanguageCode").unwrap_or_default(),
            language: tag(block, "Language").unwrap_or_default(),
            edition: tag(block, "Edition").unwrap_or_default(),
            architecture: tag(block, "Architecture").unwrap_or_default(),
            size,
            sha1: tag(block, "Sha1"),
            sha256: tag(block, "Sha256"),
            url,
        });
    }
    if out.is_empty() {
        return Err("The Microsoft catalog didn't list any Windows downloads.".into());
    }
    out.sort_by(|a, b| (&a.language, &a.architecture, &a.edition).cmp(&(&b.language, &b.architecture, &b.edition)));
    Ok(out)
}

/// The host part of an http(s) URL, lower-cased.
pub fn host(url: &str) -> Option<String> {
    let rest = url.strip_prefix("https://").or_else(|| url.strip_prefix("http://"))?;
    let h = rest.split(['/', '?', '#']).next()?;
    let h = h.rsplit('@').next()?.split(':').next()?;
    (!h.is_empty()).then(|| h.to_ascii_lowercase())
}

/// Only Microsoft download hosts (and their subdomains) are allowed.
pub fn host_allowed(url: &str) -> bool {
    host(url).is_some_and(|h| ALLOWED_HOSTS.iter().any(|a| h == *a || h.ends_with(&format!(".{a}"))))
}

/// Prefers HTTPS. Integrity is enforced by the catalog hash either way.
pub fn https(url: &str) -> String {
    match url.strip_prefix("http://") {
        Some(r) => format!("https://{r}"),
        None => url.to_string(),
    }
}

/// A file name safe to create on disk (no paths or odd characters).
pub fn safe_file_name(name: &str) -> Option<String> {
    let ok = !name.is_empty()
        && name.len() <= 200
        && !name.starts_with('.')
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'_' || b == b'-');
    ok.then(|| name.to_string())
}

/// Which images inside a Media Creation Tool ESD are what.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EsdLayout {
    /// "Windows Setup Media": the ISO's files.
    pub media: u32,
    /// Windows PE (becomes boot.wim index 1).
    pub pe: u32,
    /// Windows Setup (becomes boot.wim index 2, bootable).
    pub setup: u32,
    /// Windows editions (become install.esd).
    pub editions: Vec<u32>,
}

pub fn classify(images: &[ImageSummary]) -> Result<EsdLayout, String> {
    let has = |i: &ImageSummary, w: &str| i.name.to_ascii_lowercase().contains(w);
    let media = images
        .iter()
        .find(|i| has(i, "setup media"))
        .map(|i| i.index)
        .ok_or("The download has no setup media image.")?;
    let pe_like: Vec<&ImageSummary> =
        images.iter().filter(|i| i.installation_type.eq_ignore_ascii_case("WindowsPE")).collect();
    let setup =
        pe_like.iter().find(|i| has(i, "setup")).map(|i| i.index).ok_or("The download has no Windows Setup image.")?;
    let pe = pe_like
        .iter()
        .find(|i| i.index != setup && has(i, "pe"))
        .map(|i| i.index)
        .ok_or("The download has no Windows PE image.")?;
    let editions: Vec<u32> = images.iter().filter(|i| i.is_client()).map(|i| i.index).collect();
    if editions.is_empty() {
        return Err("The download contains no Windows editions.".into());
    }
    Ok(EsdLayout { media, pe, setup, editions })
}

#[cfg(test)]
mod tests {
    use super::*;

    const XML: &str = "\u{feff}<?xml version=\"1.0\"?><MCT><Catalogs><Catalog><PublishedMedia><Files>\
<File id=\"1\"><FileName>26100.1742.240906-0331.ge_release_svc_refresh_CLIENTCONSUMER_RET_x64FRE_en-us.esd</FileName>\
<LanguageCode>en-us</LanguageCode><Language>English (United States)</Language><Edition>Professional</Edition>\
<Architecture>x64</Architecture><Size>4123456789</Size><Sha1>0123456789ABCDEF0123456789ABCDEF01234567</Sha1>\
<FilePath>http://dl.delivery.mp.microsoft.com/filestreamingservice/files/abc/26100_x64_en-us.esd</FilePath></File>\
<File id=\"2\"><FileName>26100.1742.240906-0331.ge_release_svc_refresh_CLIENTCONSUMER_RET_x64FRE_en-us.esd</FileName>\
<Size>1</Size><FilePath>http://x/y.esd</FilePath></File>\
<File id=\"3\"><FileName>26100.1742.x_CLIENTCONSUMER_RET_A64FRE_de-de.esd</FileName><LanguageCode>de-de</LanguageCode>\
<Language>Deutsch</Language><Edition>Professional</Edition><Architecture>ARM64</Architecture><Size>3900000000</Size>\
<Sha1>zz</Sha1><FilePath>http://dl.delivery.mp.microsoft.com/files/de.esd</FilePath></File>\
<File id=\"4\"><FileName>broken.esd</FileName></File>\
</Files></PublishedMedia></Catalog></Catalogs></MCT>";

    #[test]
    fn parses_catalog() {
        let f = parse_products(XML).unwrap();
        assert_eq!(f.len(), 2, "duplicate and incomplete entries are dropped");
        let en = f.iter().find(|e| e.language_code == "en-us").unwrap();
        assert_eq!(en.size, 4_123_456_789);
        assert_eq!(en.build(), Some(26100));
        assert_eq!(en.expected_hash().unwrap().0, "SHA1");
        let de = f.iter().find(|e| e.language_code == "de-de").unwrap();
        assert!(de.expected_hash().is_none(), "malformed hash is rejected");
        assert!(parse_products("<MCT/>").is_err());
    }

    #[test]
    fn decodes_and_detects_catalogs() {
        assert!(is_cab(b"MSCF\0\0\0\0"));
        assert!(!is_cab(b"<html>"));
        let utf16: Vec<u8> = [0xFF, 0xFE]
            .into_iter()
            .chain(XML.trim_start_matches('\u{feff}').encode_utf16().flat_map(|u| u.to_le_bytes()))
            .collect();
        let text = decode_text(&utf16);
        assert!(looks_like_catalog(&text));
        assert_eq!(parse_products(&text).unwrap().len(), 2);
        let utf8 = decode_text(XML.as_bytes());
        assert!(looks_like_catalog(&utf8));
        assert!(!looks_like_catalog("<html><body>Access denied</body></html>"));
    }

    #[test]
    fn url_rules() {
        assert!(host_allowed("http://dl.delivery.mp.microsoft.com/a.esd"));
        assert!(host_allowed("https://tlu.dl.delivery.mp.microsoft.com/a.esd"));
        assert!(!host_allowed("https://dl.delivery.mp.microsoft.com.evil.com/a.esd"));
        assert!(!host_allowed("https://evil.com/dl.delivery.mp.microsoft.com"));
        assert!(!host_allowed("https://user@evil.com/"));
        assert!(!host_allowed("ftp://dl.delivery.mp.microsoft.com/a"));
        assert_eq!(https("http://a/b"), "https://a/b");
        assert_eq!(host("https://u:p@Host.Example:443/x"), Some("host.example".into()));
    }

    #[test]
    fn file_names() {
        assert!(safe_file_name("26100.1_x64FRE_en-us.esd").is_some());
        assert!(safe_file_name("..\\evil.esd").is_none());
        assert!(safe_file_name("a/b.esd").is_none());
        assert!(safe_file_name(".hidden").is_none());
    }

    fn img(index: u32, name: &str, ty: &str) -> ImageSummary {
        ImageSummary {
            index,
            name: name.into(),
            description: String::new(),
            edition_id: String::new(),
            installation_type: ty.into(),
            architecture: "x64".into(),
            major: 10,
            minor: 0,
            build: 26100,
            sp_build: 1,
            size_bytes: 1,
            languages: vec![],
        }
    }

    #[test]
    fn classifies_esd() {
        let imgs = vec![
            img(1, "Windows Setup Media", ""),
            img(2, "Microsoft Windows PE (amd64)", "WindowsPE"),
            img(3, "Microsoft Windows Setup (amd64)", "WindowsPE"),
            img(4, "Windows 11 Home", "Client"),
            img(5, "Windows 11 Pro", "Client"),
        ];
        let l = classify(&imgs).unwrap();
        assert_eq!((l.media, l.pe, l.setup), (1, 2, 3));
        assert_eq!(l.editions, vec![4, 5]);
        assert!(classify(&imgs[..3]).is_err());
        assert!(classify(&imgs[1..]).is_err());
    }
}
