use crate::declarations::dependencies::edit::{
    BUILD_MACHINE_NAME, BUILDER_PARAMETER_NAME, BuildDependencyEditError,
};
use crate::declarations::dependencies::read::DependencyProjectionError;
use psi_source_files_to_tokens::Lexer;
use psi_syntax_trees::item::Item;
use psi_tokens::TokenStream;
use psi_tokens_to_syntax_trees::parse_syntax_trees;

#[derive(Debug)]
pub(super) struct BuildLayout {
    machine_indent: String,
    body_open_end: usize,
    body_close_start: usize,
    dependency_rows: Vec<DependencyRow>,
}

impl BuildLayout {
    pub(super) fn dependency_rows(&self) -> &[DependencyRow] {
        &self.dependency_rows
    }

    pub(super) fn insert_statement(&self, source: &str, statement: &str) -> Option<String> {
        let newline = if source.contains("\r\n") {
            "\r\n"
        } else {
            "\n"
        };
        let interior = &source[self.body_open_end..self.body_close_start];
        let mut replacement = source.to_owned();
        if interior.trim().is_empty() {
            let inner_indent = format!("{}    ", self.machine_indent);
            replacement.replace_range(
                self.body_open_end..self.body_close_start,
                &format!(
                    "{newline}{inner_indent}{statement}{newline}{}",
                    self.machine_indent
                ),
            );
            return Some(replacement);
        }
        let close_line_start = source[..self.body_close_start]
            .rfind('\n')
            .map_or(0, |index| index + 1);
        let close_prefix = &source[close_line_start..self.body_close_start];
        if !close_prefix.chars().all(char::is_whitespace) {
            return None;
        }
        replacement.insert_str(
            close_line_start,
            &format!("{}    {statement}{newline}", close_prefix),
        );
        Some(replacement)
    }
}

#[derive(Debug)]
pub(super) struct DependencyRow {
    pub(super) start: usize,
    pub(super) end: usize,
    pub(super) contains_comment: bool,
}

/// `None` means no build machine. `Some(None)` means a valid but noncanonical
/// build signature that must be edited manually.
pub(super) fn discover_build_layout(
    source: &str,
) -> Result<Option<Option<BuildLayout>>, BuildDependencyEditError> {
    let tokens = Lexer::new(source).tokenize().map_err(|error| {
        BuildDependencyEditError::InvalidBuild(DependencyProjectionError::Lex {
            message: error.message,
        })
    })?;
    let syntax = parse_syntax_trees(&tokens).map_err(|error| {
        BuildDependencyEditError::InvalidBuild(DependencyProjectionError::Parse {
            message: error.message,
        })
    })?;
    let Some(build) = syntax.root_items().find_map(|item| match item {
        Item::Machine(machine) if machine.name.as_str() == BUILD_MACHINE_NAME => Some(machine),
        _ => None,
    }) else {
        return Ok(None);
    };
    let semantic = semantic_indices(&tokens);
    let name_span = build.name.source_span().span;
    let Some(name_position) = semantic.iter().position(|index| {
        let token = &tokens[*index];
        token.span == name_span && token.lexeme == BUILD_MACHINE_NAME
    }) else {
        return Ok(Some(None));
    };
    let expected_before = name_position
        .checked_sub(1)
        .and_then(|position| semantic.get(position))
        .is_some_and(|index| tokens[*index].lexeme == "machine");
    let expected_after = ["(", "builder", ":", "&", "mut", "Build", ")", "{"];
    if !expected_before
        || expected_after.iter().enumerate().any(|(offset, expected)| {
            semantic
                .get(name_position + 1 + offset)
                .is_none_or(|index| tokens[*index].lexeme != *expected)
        })
    {
        return Ok(Some(None));
    }
    let machine_token = semantic[name_position - 1];
    let body_open = semantic[name_position + expected_after.len()];
    let Some(body_close) = matching_brace(&tokens, body_open) else {
        return Ok(Some(None));
    };
    let machine_indent = line_indent(source, tokens[machine_token].span.start)
        .unwrap_or_default()
        .to_owned();
    let dependency_rows = dependency_rows(&tokens, body_open, body_close);
    Ok(Some(Some(BuildLayout {
        machine_indent,
        body_open_end: tokens[body_open].span.end,
        body_close_start: tokens[body_close].span.start,
        dependency_rows,
    })))
}

fn semantic_indices(tokens: &TokenStream<'_>) -> Vec<usize> {
    tokens
        .iter()
        .enumerate()
        .filter_map(|(index, token)| (!token.is_non_semantic()).then_some(index))
        .collect()
}

fn matching_brace(tokens: &TokenStream<'_>, open: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (index, token) in tokens.iter().enumerate().skip(open) {
        match token.lexeme.try_as_str() {
            Some("{") => depth = depth.checked_add(1)?,
            Some("}") => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
}

fn dependency_rows(
    tokens: &TokenStream<'_>,
    body_open: usize,
    body_close: usize,
) -> Vec<DependencyRow> {
    let semantic = semantic_indices(tokens)
        .into_iter()
        .filter(|index| *index > body_open && *index < body_close)
        .collect::<Vec<_>>();
    let mut rows = Vec::new();
    let mut brace_depth = 0usize;
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    let mut position = 0usize;
    while position < semantic.len() {
        let index = semantic[position];
        let token = &tokens[index];
        if brace_depth == 0
            && paren_depth == 0
            && bracket_depth == 0
            && token.lexeme == BUILDER_PARAMETER_NAME
            && semantic
                .get(position + 1)
                .is_some_and(|next| tokens[*next].lexeme == ".")
            && semantic.get(position + 2).is_some_and(|next| {
                matches!(
                    tokens[*next].lexeme.try_as_str(),
                    Some("depend" | "depend_as")
                )
            })
            && semantic
                .get(position + 3)
                .is_some_and(|next| tokens[*next].lexeme == "(")
            && let Some(end_position) = dependency_row_end(tokens, &semantic, position + 3)
        {
            let end_index = semantic[end_position];
            rows.push(DependencyRow {
                start: token.span.start,
                end: tokens[end_index].span.end,
                contains_comment: tokens[index..=end_index]
                    .iter()
                    .any(|token| token.comment().is_some()),
            });
            position = end_position + 1;
            continue;
        }
        match token.lexeme.try_as_str() {
            Some("{") => brace_depth += 1,
            Some("}") => brace_depth = brace_depth.saturating_sub(1),
            Some("(") => paren_depth += 1,
            Some(")") => paren_depth = paren_depth.saturating_sub(1),
            Some("[") => bracket_depth += 1,
            Some("]") => bracket_depth = bracket_depth.saturating_sub(1),
            _ => {}
        }
        position += 1;
    }
    rows
}

fn dependency_row_end(
    tokens: &TokenStream<'_>,
    semantic: &[usize],
    open_position: usize,
) -> Option<usize> {
    let mut depth = 0usize;
    for position in open_position..semantic.len() {
        match tokens[semantic[position]].lexeme.try_as_str() {
            Some("(") => depth = depth.checked_add(1)?,
            Some(")") => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    let semicolon = position + 1;
                    return semantic
                        .get(semicolon)
                        .filter(|index| tokens[**index].lexeme == ";")
                        .map(|_| semicolon);
                }
            }
            _ => {}
        }
    }
    None
}

fn line_indent(source: &str, offset: usize) -> Option<&str> {
    let line_start = source[..offset].rfind('\n').map_or(0, |index| index + 1);
    let prefix = &source[line_start..offset];
    prefix.chars().all(char::is_whitespace).then_some(prefix)
}
