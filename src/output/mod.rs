pub mod json;
pub mod markdown;
pub mod xml;

use crate::cli::OutputFormat;

#[derive(Debug, Clone)]
pub struct OutputSections {
    pub metadata: String,
    pub directory_tree: String,
    pub module_map: String,
    pub dependency_graph: String,
    pub key_files: String,
    pub signatures: String,
    pub git_context: String,
}

pub fn render_single_section(title: &str, content: &str, format: &OutputFormat) -> String {
    match format {
        OutputFormat::Markdown => markdown::render_single_section(title, content),
        OutputFormat::Xml => xml::render_single_section(title, content),
        OutputFormat::Json => json::render_single_section(title, content),
    }
}

pub fn render(sections: &OutputSections, format: &OutputFormat) -> String {
    match format {
        OutputFormat::Markdown => markdown::render(sections),
        OutputFormat::Xml => xml::render(sections),
        OutputFormat::Json => json::render(sections),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_sections() -> OutputSections {
        OutputSections {
            metadata: "name: test".to_string(),
            directory_tree: "src/".to_string(),
            module_map: String::new(),
            dependency_graph: String::new(),
            key_files: String::new(),
            signatures: String::new(),
            git_context: String::new(),
        }
    }

    #[test]
    fn test_render_dispatches_markdown() {
        let output = render(&make_sections(), &OutputFormat::Markdown);
        assert!(output.contains("# ") || output.contains("##"));
    }

    #[test]
    fn test_render_dispatches_xml() {
        let output = render(&make_sections(), &OutputFormat::Xml);
        assert!(output.contains("<cxpak>"));
    }

    #[test]
    fn test_render_dispatches_json() {
        let output = render(&make_sections(), &OutputFormat::Json);
        assert!(output.contains("\"metadata\""));
    }

    #[test]
    fn test_render_single_section_all_formats() {
        let md = render_single_section("Test", "content", &OutputFormat::Markdown);
        assert!(md.contains("content"));

        let xml = render_single_section("Test", "content", &OutputFormat::Xml);
        assert!(xml.contains("<cxpak>"));

        let json = render_single_section("Test", "content", &OutputFormat::Json);
        assert!(json.contains("content"));
    }
}
