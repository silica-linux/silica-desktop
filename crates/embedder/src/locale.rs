//! The system languages for Flutter (`FlutterEngineUpdateLocales`): from `LANGUAGE` (a list,
//! `it:en`), otherwise the first of `LC_ALL`, `LC_MESSAGES`, `LANG`. `C` and `POSIX` mean
//! English. Apps resolve them against their translations (English source, Italian first).

/// A language: code and optional country (`it`, `IT`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Locale {
    pub language: String,
    pub country: Option<String>,
}

/// One POSIX locale name (`it_IT.UTF-8@euro`) as a `Locale`; None for an empty or odd name.
fn parse_one(name: &str) -> Option<Locale> {
    let base = name.split(['.', '@']).next()?.trim();
    if base.is_empty() {
        return None;
    }
    if base == "C" || base == "POSIX" {
        return Some(Locale {
            language: "en".to_owned(),
            country: Some("US".to_owned()),
        });
    }
    let mut parts = base.split(['_', '-']);
    let language = parts.next()?.to_ascii_lowercase();
    if language.is_empty()
        || language.len() > 8
        || !language.chars().all(|c| c.is_ascii_alphabetic())
    {
        return None;
    }
    let country = parts
        .next()
        .filter(|c| !c.is_empty() && c.len() <= 3 && c.chars().all(|c| c.is_ascii_alphanumeric()))
        .map(str::to_ascii_uppercase);
    Some(Locale { language, country })
}

/// The preferred languages from the environment, most preferred first, without repeats.
pub fn from_env(get: impl Fn(&str) -> Option<String>) -> Vec<Locale> {
    let mut out: Vec<Locale> = Vec::new();
    fn push(out: &mut Vec<Locale>, locale: Locale) {
        if !out.contains(&locale) {
            out.push(locale);
        }
    }
    let single = ["LC_ALL", "LC_MESSAGES", "LANG"]
        .into_iter()
        .find_map(|key| get(key).filter(|v| !v.is_empty()));
    // LANGUAGE is ignored when the locale is C (GNU gettext does the same).
    let c_locale = single.as_deref().is_some_and(|v| v == "C" || v == "POSIX");
    if !c_locale && let Some(list) = get("LANGUAGE") {
        for name in list.split(':').take(16) {
            if let Some(locale) = parse_one(name) {
                push(&mut out, locale);
            }
        }
    }
    if let Some(name) = single
        && let Some(locale) = parse_one(&name)
    {
        push(&mut out, locale);
    }
    if out.is_empty() {
        push(
            &mut out,
            Locale {
                language: "en".to_owned(),
                country: Some("US".to_owned()),
            },
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |key| {
            pairs
                .iter()
                .find(|(k, _)| *k == key)
                .map(|(_, v)| (*v).to_owned())
        }
    }

    fn l(language: &str, country: Option<&str>) -> Locale {
        Locale {
            language: language.to_owned(),
            country: country.map(str::to_owned),
        }
    }

    #[test]
    fn lang_and_language_lists() {
        assert_eq!(
            from_env(env(&[("LANG", "it_IT.UTF-8")])),
            vec![l("it", Some("IT"))]
        );
        assert_eq!(
            from_env(env(&[("LANG", "it_IT.UTF-8"), ("LANGUAGE", "it:en_GB")])),
            vec![l("it", None), l("en", Some("GB")), l("it", Some("IT"))]
        );
        assert_eq!(
            from_env(env(&[("LC_ALL", "de_DE@euro"), ("LANG", "it_IT.UTF-8")])),
            vec![l("de", Some("DE"))]
        );
    }

    #[test]
    fn c_locale_and_hostile_values() {
        assert_eq!(
            from_env(env(&[("LANG", "C"), ("LANGUAGE", "it")])),
            vec![l("en", Some("US"))]
        );
        assert_eq!(from_env(env(&[])), vec![l("en", Some("US"))]);
        assert_eq!(
            from_env(env(&[("LANG", "../../etc"), ("LANGUAGE", "::x1y:")])),
            vec![l("en", Some("US"))]
        );
    }
}
