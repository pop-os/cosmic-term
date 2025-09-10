// SPDX-License-Identifier: GPL-3.0-only

use std::{str::FromStr, sync::LazyLock};

use i18n_embed::{
    DefaultLocalizer, LanguageLoader, Localizer,
    fluent::{FluentLanguageLoader, fluent_language_loader},
};
use icu_collator::{Collator, CollatorOptions, Numeric};
use icu_provider::DataLocale;
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "i18n/"]
struct Localizations;

pub static LANGUAGE_LOADER: LazyLock<FluentLanguageLoader> = LazyLock::new(|| {
    let loader: FluentLanguageLoader = fluent_language_loader!();

    loader
        .load_fallback_language(&Localizations)
        .expect("Error while loading fallback language");

    loader
});

pub static LANGUAGE_SORTER: LazyLock<Collator> = LazyLock::new(|| {
    let mut options = CollatorOptions::new();
    options.numeric = Some(Numeric::On);

    DataLocale::from_str(&LANGUAGE_LOADER.current_language().to_string())
            .or_else(|_| DataLocale::from_str(&LANGUAGE_LOADER.fallback_language().to_string()))
            .ok()
            .and_then(|locale| Collator::try_new(&locale, options).ok())
            .or_else(|| {
                let locale = DataLocale::from_str("en-US").expect("en-US is a valid BCP-47 tag");
                Collator::try_new(&locale, options).ok()
            })
            .expect("Creating a collator from the system's current language, the fallback language, or American English should succeed")
});

#[macro_export]
macro_rules! fl {
    ($message_id:literal) => {{
        i18n_embed_fl::fl!($crate::localize::LANGUAGE_LOADER, $message_id)
    }};

    ($message_id:literal, $($args:expr),*) => {{
        i18n_embed_fl::fl!($crate::localize::LANGUAGE_LOADER, $message_id, $($args), *)
    }};
}

// Get the `Localizer` to be used for localizing this library.
pub fn localizer() -> Box<dyn Localizer> {
    Box::from(DefaultLocalizer::new(&*LANGUAGE_LOADER, &Localizations))
}

pub fn localize() {
    let localizer = localizer();
    let requested_languages = i18n_embed::DesktopLanguageRequester::requested_languages();

    if let Err(error) = localizer.select(&requested_languages) {
        eprintln!("Error while loading language for App List {error}");
    }
}
