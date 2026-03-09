use super::OutputSections;

pub fn render(sections: &OutputSections) -> String {
    let mut out = String::from("<cxpak>\n");
    emit_section(&mut out, "metadata", &sections.metadata);
    emit_section(&mut out, "directory-tree", &sections.directory_tree);
    emit_section(&mut out, "module-map", &sections.module_map);
    emit_section(&mut out, "dependency-graph", &sections.dependency_graph);
    emit_section(&mut out, "key-files", &sections.key_files);
    emit_section(&mut out, "signatures", &sections.signatures);
    emit_section(&mut out, "git-context", &sections.git_context);
    out.push_str("</cxpak>\n");
    out
}

pub fn render_single_section(title: &str, content: &str) -> String {
    let tag = title.to_lowercase().replace([' ', '/'], "-");
    let mut out = String::from("<cxpak>\n");
    emit_section(&mut out, &tag, content);
    out.push_str("</cxpak>\n");
    out
}

fn emit_section(out: &mut String, tag: &str, content: &str) {
    if !content.is_empty() {
        out.push_str(&format!("  <{tag}>\n"));
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("<!-- ") && trimmed.ends_with(" -->") {
                // Omission pointer — emit as XML element instead of escaped comment
                let inner = &trimmed[5..trimmed.len() - 4];
                out.push_str(&format!(
                    "    <detail-ref>{}</detail-ref>\n",
                    escape_xml(inner)
                ));
            } else {
                out.push_str(&format!("    {}\n", escape_xml(line)));
            }
        }
        out.push_str(&format!("  </{tag}>\n"));
    }
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
