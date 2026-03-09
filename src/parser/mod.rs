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
