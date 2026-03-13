use super::OutputSections;

pub fn render(sections: &OutputSections) -> String {
    let mut out = String::new();
    if !sections.metadata.is_empty() {
        out.push_str("## Project Metadata\n\n");
        out.push_str(&sections.metadata);
        out.push_str("\n\n");
    }
    if !sections.directory_tree.is_empty() {
        out.push_str("## Directory Tree\n\n```\n");
        out.push_str(&sections.directory_tree);
        out.push_str("\n```\n\n");
    }
    if !sections.module_map.is_empty() {
        out.push_str("## Module / Component Map\n\n");
        out.push_str(&sections.module_map);
        out.push_str("\n\n");
    }
    if !sections.dependency_graph.is_empty() {
        out.push_str("## Dependency Graph\n\n");
        out.push_str(&sections.dependency_graph);
        out.push_str("\n\n");
    }
    if !sections.key_files.is_empty() {
        out.push_str("## Key Files\n\n");
        out.push_str(&sections.key_files);
        out.push_str("\n\n");
    }
    if !sections.signatures.is_empty() {
        out.push_str("## Function / Type Signatures\n\n");
        out.push_str(&sections.signatures);
        out.push_str("\n\n");
    }
    if !sections.git_context.is_empty() {
        out.push_str("## Git Context\n\n");
        out.push_str(&sections.git_context);
        out.push_str("\n\n");
    }
    out
}

pub fn render_single_section(title: &str, content: &str) -> String {
    format!("## {title}\n\n{content}\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_single_section() {
        let content = "### src/main.rs\n- pub Function: `main`\n";
        let output = render_single_section("Module / Component Map", content);
        assert!(output.starts_with("## Module / Component Map"));
        assert!(output.contains("pub Function"));
    }

    #[test]
    fn test_render_all_sections() {
        let sections = OutputSections {
            metadata: "name: test".to_string(),
            directory_tree: "src/".to_string(),
            module_map: "mod a".to_string(),
            dependency_graph: "a -> b".to_string(),
            key_files: "main.rs".to_string(),
            signatures: "fn main()".to_string(),
            git_context: "branch: main".to_string(),
        };
        let output = render(&sections);
        assert!(output.contains("## Project Metadata"));
        assert!(output.contains("## Directory Tree"));
        assert!(output.contains("## Module / Component Map"));
        assert!(output.contains("## Dependency Graph"));
        assert!(output.contains("## Key Files"));
        assert!(output.contains("## Function / Type Signatures"));
        assert!(output.contains("## Git Context"));
    }

    #[test]
    fn test_render_includes_sections() {
        let sections = OutputSections {
            metadata: "Language: Rust (100%)".into(),
            directory_tree: "src/\n  main.rs".into(),
            module_map: String::new(),
            dependency_graph: String::new(),
            key_files: String::new(),
            signatures: String::new(),
            git_context: String::new(),
        };
        let output = render(&sections);
        assert!(output.contains("## Project Metadata"));
        assert!(output.contains("Language: Rust"));
        assert!(output.contains("## Directory Tree"));
        assert!(!output.contains("## Module")); // empty = omitted
    }
}
