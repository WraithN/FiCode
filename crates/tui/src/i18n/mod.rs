use std::sync::atomic::{AtomicBool, Ordering};

rust_i18n::i18n!("locales", fallback = "en");

static LANG_SET: AtomicBool = AtomicBool::new(false);

pub fn set_language(lang: &str) {
    rust_i18n::set_locale(lang);
    LANG_SET.store(true, Ordering::Relaxed);
}

pub fn current_language() -> String {
    if LANG_SET.load(Ordering::Relaxed) {
        rust_i18n::locale().to_string()
    } else {
        std::env::var("LANG")
            .unwrap_or_default()
            .split('.')
            .next()
            .unwrap_or("en")
            .split('_')
            .next()
            .unwrap_or("en")
            .to_string()
    }
}
