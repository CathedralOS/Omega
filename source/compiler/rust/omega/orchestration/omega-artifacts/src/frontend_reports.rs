//! Source-load and syntax-tree artifact presentation.

use psi_diagnostics::Diagnostic;

use super::{
    ArtifactWriter, AstArtifact, AstFileArtifact, SourceFileArtifact, SourceLoadArtifact,
    format_bytes,
};

impl ArtifactWriter {
    pub fn write_sources(&self, source_artifact: &SourceLoadArtifact) -> Result<(), Diagnostic> {
        let mut output = String::new();

        let total_bytes = source_artifact
            .files
            .iter()
            .map(|file| file.byte_count)
            .sum::<usize>();
        let total_lines = source_artifact
            .files
            .iter()
            .map(|file| file.line_count)
            .sum::<usize>();
        let total_non_empty_lines = source_artifact
            .files
            .iter()
            .map(|file| file.non_empty_line_count)
            .sum::<usize>();

        output.push_str("# Omega Source Load\n\n");
        output.push_str("## Totals\n");
        output.push_str(&format!("files: {}\n", source_artifact.files.len()));
        output.push_str(&format!("items: {}\n", source_artifact.item_count));
        output.push_str(&format!("bytes: {}\n", format_bytes(total_bytes as u64)));
        output.push_str(&format!("lines: {}\n", total_lines));
        output.push_str(&format!("non-empty lines: {}\n\n", total_non_empty_lines));

        output.push_str("## Files\n");
        output.push_str(&format!(
            "{:<4} {:>8} {:>7} {:>7} {:>7} {:>11} {}\n",
            "id", "bytes", "lines", "code", "items", "item range", "path"
        ));
        output.push_str(&format!(
            "{:<4} {:>8} {:>7} {:>7} {:>7} {:>11} {}\n",
            "--", "-----", "-----", "----", "-----", "----------", "----"
        ));

        for file in &source_artifact.files {
            write_source_file_artifact(&mut output, file);
        }

        self.write_html_report("01_sources.html", "sources", &output)
    }

    pub fn write_ast(&self, ast_artifact: &AstArtifact) -> Result<(), Diagnostic> {
        let mut output = String::new();

        output.push_str("# Omega AST\n\n");
        output.push_str(&format!("files: {}\n", ast_artifact.file_count));
        output.push_str(&format!("items: {}\n\n", ast_artifact.item_count));

        output.push_str("## Identity Storage\n");
        output.push_str(&format!(
            "owned identifier strings: {}\n",
            ast_artifact.identity.owned_identifier_strings
        ));
        output.push_str(&format!(
            "identifiers: {}\n",
            ast_artifact.identity.identifiers
        ));
        output.push_str(&format!(
            "source identifiers: {}\n",
            ast_artifact.identity.source_identifiers
        ));
        output.push_str(&format!(
            "generated identifiers: {}\n",
            ast_artifact.identity.generated_identifiers
        ));
        output.push_str(&format!(
            "path members: {}\n",
            ast_artifact.identity.path_members
        ));
        output.push_str(&format!(
            "string literals: {}\n",
            ast_artifact.identity.string_literals
        ));
        output.push_str(&format!(
            "float literals: {}\n",
            ast_artifact.identity.float_literals
        ));
        output.push_str(&format!(
            "source float literals: {}\n",
            ast_artifact.identity.source_float_literals
        ));
        output.push_str(&format!(
            "generated float literals: {}\n\n",
            ast_artifact.identity.generated_float_literals
        ));

        for file in &ast_artifact.files {
            write_ast_file_artifact(&mut output, file);
        }

        self.write_html_report("02_ast.html", "ast", &output)
    }
}

fn write_source_file_artifact(output: &mut String, file: &SourceFileArtifact) {
    let item_range = if file.item_count == 0 {
        String::from("-")
    } else {
        format!("{}..{}", file.first_item, file.first_item + file.item_count)
    };

    output.push_str(&format!(
        "{:<4} {:>8} {:>7} {:>7} {:>7} {:>11} {}\n",
        file.id,
        format_bytes(file.byte_count as u64),
        file.line_count,
        file.non_empty_line_count,
        file.item_count,
        item_range,
        file.path.display()
    ));
}

fn write_ast_file_artifact(output: &mut String, file: &AstFileArtifact) {
    output.push_str(&format!("## {}\n", file.path.display()));
    output.push_str(&format!(
        "tables: statements {} transition_targets {} expressions {} type_references {} type_constraints {}\n",
        file.statement_count,
        file.transition_target_count,
        file.expression_count,
        file.type_reference_count,
        file.type_constraint_count
    ));

    if !file.item_range_valid {
        output.push_str("invalid item range\n\n");
        return;
    }

    if file.item_summaries.is_empty() {
        output.push_str("items: none\n\n");
        return;
    }

    for (index, summary) in file.item_summaries.iter().enumerate() {
        output.push_str(&format!(
            "- item {}: {}\n",
            file.first_item + index,
            summary
        ));
    }

    output.push('\n');
}
