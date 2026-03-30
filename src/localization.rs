// Localization system using fluent-rs (fluent-bundle)
//
// FTL files are embedded into the binary at compile time via include_str!.
// At startup, LocalizationService parses all FTL sources and pre-computes
// translations into AHashMaps for O(1) per-request lookups.
//
// Language resolution order (highest priority first):
//   1. User's saved preference (from DB), unless it is "browser"
//   2. Browser negotiation via the Accept-Language request header
//   3. Server-wide default from config.locale
//   4. Hardcoded fallback: "en"

use fluent_bundle::{FluentBundle, FluentResource};
use unic_langid::LanguageIdentifier;

// ---------------------------------------------------------------------------
// Embedded locale files – compiled into the binary
// ---------------------------------------------------------------------------

const LOCALE_EN: &str = include_str!("../locales/en/main.ftl");
const LOCALE_CS: &str = include_str!("../locales/cs/main.ftl");
const LOCALE_DE: &str = include_str!("../locales/de/main.ftl");
const LOCALE_FR: &str = include_str!("../locales/fr/main.ftl");
const LOCALE_ES: &str = include_str!("../locales/es/main.ftl");
const LOCALE_PL: &str = include_str!("../locales/pl/main.ftl");

/// All embedded locales as (code, ftl_source) pairs.
static EMBEDDED_LOCALES: &[(&str, &str)] = &[
    ("en", LOCALE_EN),
    ("cs", LOCALE_CS),
    ("de", LOCALE_DE),
    ("fr", LOCALE_FR),
    ("es", LOCALE_ES),
    ("pl", LOCALE_PL),
];

/// Human-readable display name for a language code.
pub fn lang_display_name(code: &str) -> &'static str {
    match code {
        "en" => "English",
        "cs" => "Čeština",
        "de" => "Deutsch",
        "fr" => "Français",
        "es" => "Español",
        "pl" => "Polski",
        _ => "Unknown",
    }
}

// ---------------------------------------------------------------------------
// LangInfo – metadata about one available language
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct LangInfo {
    pub code: String,
    pub name: &'static str,
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Parse an FTL source string using fluent-bundle and extract all simple
/// message values into a flat AHashMap.  Complex patterns that fail to format
/// are silently skipped (they will fall through to the English fallback).
fn parse_ftl_to_map(lang_code: &str, ftl_source: &str) -> AHashMap<String, String> {
    let lang_id: LanguageIdentifier = lang_code
        .parse()
        .unwrap_or_else(|_| "en".parse().expect("'en' is always a valid lang id"));

    // Build a temporary FluentBundle just for this startup parse.
    let mut bundle: FluentBundle<FluentResource> = FluentBundle::new(vec![lang_id]);

    let resource = match FluentResource::try_new(ftl_source.to_string()) {
        Ok(r) => r,
        Err((r, _errs)) => r, // use partial resource even if there were parse errors
    };
    // add_resource errors only occur on duplicate IDs; ignore them.
    let _ = bundle.add_resource(resource);

    // Collect all (key, formatted_value) pairs by scanning the FTL source for
    // "key = ..." lines.  We use the source scan to discover key names because
    // FluentBundle does not expose an iterator over loaded message IDs.
    let mut map = AHashMap::new();

    for line in ftl_source.lines() {
        let line = line.trim_end();
        // Skip blank lines, comments, and indented continuation lines.
        if line.is_empty()
            || line.starts_with('#')
            || line.starts_with(' ')
            || line.starts_with('\t')
        {
            continue;
        }
        if let Some((raw_key, _)) = line.split_once(" = ") {
            let key = raw_key.trim();
            if key.is_empty() {
                continue;
            }
            if let Some(msg) = bundle.get_message(key) {
                if let Some(pattern) = msg.value() {
                    let mut errors = vec![];
                    let value = bundle
                        .format_pattern(pattern, None, &mut errors)
                        .into_owned();
                    if errors.is_empty() {
                        map.insert(key.to_string(), value);
                    }
                }
            }
        }
    }

    map
}

/// Parse an `Accept-Language` header value and return ordered language codes
/// with their quality weights.  E.g. `"en-US,en;q=0.9,de;q=0.8"` →
/// `[("en", 1.0), ("en", 0.9), ("de", 0.8)]` (with prefix normalisation).
fn parse_accept_language(header: &str) -> Vec<(String, f32)> {
    let mut pairs: Vec<(String, f32)> = header
        .split(',')
        .filter_map(|part| {
            let part = part.trim();
            let mut segments = part.splitn(2, ';');
            let raw_lang = segments.next()?.trim();
            if raw_lang.is_empty() {
                return None;
            }
            // Keep only the primary and extended subtag (strip script/region for
            // matching), e.g. "en-US" → "en".  Store both the full tag and the
            // primary so we can try both.
            let primary = raw_lang
                .split('-')
                .next()
                .unwrap_or(raw_lang)
                .to_ascii_lowercase();
            let quality: f32 = segments
                .next()
                .and_then(|q| q.trim().strip_prefix("q="))
                .and_then(|v| v.parse().ok())
                .unwrap_or(1.0);
            Some((primary, quality))
        })
        .collect();

    // Sort by descending quality.
    pairs.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    pairs
}

// ---------------------------------------------------------------------------
// LocalizationService
// ---------------------------------------------------------------------------

/// Holds pre-computed translations for all embedded languages and provides
/// language resolution and lookup.  Wrap in `Arc` and share via Axum Extension.
pub struct LocalizationService {
    /// lang_code → (key → translated_string)
    translations: AHashMap<String, AHashMap<String, String>>,
    /// Ordered list of available languages for display in the settings UI.
    pub available_langs: Vec<LangInfo>,
    /// Fallback when no better match can be found.
    fallback_lang: String,
}

impl LocalizationService {
    /// Build the service by parsing all embedded FTL sources.
    pub fn new() -> Arc<Self> {
        let mut translations: AHashMap<String, AHashMap<String, String>> = AHashMap::new();
        let mut available_langs: Vec<LangInfo> = Vec::new();

        for (code, source) in EMBEDDED_LOCALES {
            let map = parse_ftl_to_map(code, source);
            translations.insert(code.to_string(), map);
            available_langs.push(LangInfo {
                code: code.to_string(),
                name: lang_display_name(code),
            });
        }

        // Sort alphabetically by code for stable UI display.
        available_langs.sort_by(|a, b| a.code.cmp(&b.code));

        Arc::new(LocalizationService {
            translations,
            available_langs,
            fallback_lang: "en".to_string(),
        })
    }

    /// Look up a translation key for the given language, falling back to
    /// English, and finally returning the raw key if nothing else matches.
    pub fn translate(&self, lang: &str, key: &str) -> String {
        if let Some(map) = self.translations.get(lang) {
            if let Some(val) = map.get(key) {
                return val.clone();
            }
        }
        // English fallback
        if lang != "en" {
            if let Some(map) = self.translations.get("en") {
                if let Some(val) = map.get(key) {
                    return val.clone();
                }
            }
        }
        // Last resort – return the key itself so missing strings are visible.
        key.to_string()
    }

    /// Negotiate the best available language from an `Accept-Language` header.
    pub fn negotiate_from_accept(&self, accept_language: &str) -> String {
        for (lang, _quality) in parse_accept_language(accept_language) {
            if self.translations.contains_key(lang.as_str()) {
                return lang;
            }
        }
        self.fallback_lang.clone()
    }

    /// Resolve the language for a single request.
    ///
    /// * `user_preferred`  – value from DB: `"browser"` means use browser
    ///                        detection, any other value is a language code.
    /// * `accept_language` – raw `Accept-Language` header (may be `None`).
    /// * `config_fallback` – server-wide locale from `config.json`.
    pub fn resolve_language(
        &self,
        user_preferred: Option<&str>,
        accept_language: Option<&str>,
        config_fallback: &str,
    ) -> String {
        // 1. User override (unless they chose "browser")
        if let Some(pref) = user_preferred {
            if pref != "browser" && !pref.is_empty() && self.translations.contains_key(pref) {
                return pref.to_string();
            }
        }

        // 2. Browser negotiation via Accept-Language header
        if let Some(al) = accept_language {
            if !al.trim().is_empty() {
                return self.negotiate_from_accept(al);
            }
        }

        // 3. Server-wide config locale
        if self.translations.contains_key(config_fallback) {
            return config_fallback.to_string();
        }

        // 4. Hardcoded fallback
        self.fallback_lang.clone()
    }

    /// Returns true if `code` is a known language code.
    pub fn is_valid_lang(&self, code: &str) -> bool {
        self.translations.contains_key(code)
    }
}

// ---------------------------------------------------------------------------
// RequestLocale – per-request handle passed to templates
// ---------------------------------------------------------------------------

/// Lightweight per-request wrapper that holds the resolved language and a
/// shared reference to the service.  Cloning is cheap (Arc + String clone).
#[derive(Clone)]
pub struct RequestLocale {
    pub lang: String,
    service: Arc<LocalizationService>,
}

impl RequestLocale {
    pub fn new(lang: String, service: Arc<LocalizationService>) -> Self {
        RequestLocale { lang, service }
    }

    /// Translate a message key using the resolved language for this request.
    /// Falls back to English and then to the key itself.
    pub fn t(&self, key: &str) -> String {
        self.service.translate(&self.lang, key)
    }
}
