pub mod language;
pub mod languages;

use language::LanguageSupport;
use std::collections::HashMap;

pub struct LanguageRegistry {
    languages: HashMap<String, Box<dyn LanguageSupport>>,
}

impl Default for LanguageRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl LanguageRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            languages: HashMap::new(),
        };
        registry.register_defaults();
        registry
    }

    fn register_defaults(&mut self) {
        #[cfg(feature = "lang-rust")]
        self.register(Box::new(languages::rust::RustLanguage));

        #[cfg(feature = "lang-typescript")]
        self.register(Box::new(languages::typescript::TypeScriptLanguage));

        #[cfg(feature = "lang-javascript")]
        self.register(Box::new(languages::javascript::JavaScriptLanguage));

        #[cfg(feature = "lang-python")]
        self.register(Box::new(languages::python::PythonLanguage));

        #[cfg(feature = "lang-java")]
        self.register(Box::new(languages::java::JavaLanguage));

        #[cfg(feature = "lang-go")]
        self.register(Box::new(languages::go::GoLanguage));

        #[cfg(feature = "lang-c")]
        self.register(Box::new(languages::c::CLanguage));

        #[cfg(feature = "lang-cpp")]
        self.register(Box::new(languages::cpp::CppLanguage));

        #[cfg(feature = "lang-ruby")]
        self.register(Box::new(languages::ruby::RubyLanguage));

        #[cfg(feature = "lang-csharp")]
        self.register(Box::new(languages::csharp::CSharpLanguage));

        #[cfg(feature = "lang-swift")]
        self.register(Box::new(languages::swift::SwiftLanguage));

        #[cfg(feature = "lang-kotlin")]
        self.register(Box::new(languages::kotlin::KotlinLanguage));
    }

    pub fn register(&mut self, lang: Box<dyn LanguageSupport>) {
        self.languages.insert(lang.name().to_string(), lang);
    }

    pub fn get(&self, name: &str) -> Option<&dyn LanguageSupport> {
        self.languages.get(name).map(|l| l.as_ref())
    }

    pub fn supported_languages(&self) -> Vec<&str> {
        self.languages.keys().map(|k| k.as_str()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_creates_registry() {
        let registry = LanguageRegistry::default();
        assert!(registry.get("rust").is_some());
    }

    #[test]
    fn test_supported_languages_returns_all() {
        let registry = LanguageRegistry::new();
        let langs = registry.supported_languages();
        assert!(
            langs.len() >= 12,
            "expected at least 12 languages, got {}",
            langs.len()
        );
        assert!(langs.contains(&"rust"));
        assert!(langs.contains(&"python"));
    }

    #[test]
    fn test_ts_language_all_registered() {
        // Exercises ts_language() on every registered language, which is otherwise
        // uncovered because unit tests use make_parser() directly.
        let registry = LanguageRegistry::new();
        for lang_name in registry.supported_languages() {
            let lang = registry
                .get(lang_name)
                .expect("registered language missing");
            let ts_lang = lang.ts_language();
            let mut parser = tree_sitter::Parser::new();
            parser
                .set_language(&ts_lang)
                .unwrap_or_else(|e| panic!("failed to set language for {}: {}", lang_name, e));
        }
    }

    #[test]
    fn test_get_nonexistent_language() {
        let registry = LanguageRegistry::new();
        assert!(registry.get("brainfuck").is_none());
    }
}
