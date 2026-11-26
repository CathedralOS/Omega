// Source layout discovery and insertion boundaries for conservative `build.omg` dependency edits.
// `discover_build_layout` returns `None` when no `build` machine exists and `Some(None)` when a valid `build` machine has a noncanonical signature or body layout requiring manual editing.
// Dependency declarations belong to the implicit entry before its first transition or nested state; `entry_insertion_start` locates the last top-level semicolon preceding that boundary so adding a dependency never splits a transition or makes the row conditional.

use crate::declarations::dependencies::edit::BUILD_MACHINE_NAME;
use crate::declarations::editable_build::{project_build_entry_syntax, project_dependency_rows};
use syntax_trees::SyntaxTrees;
use syntax_trees::item::Item;
use syntax_trees::statement::StatementNode;
use tokens::{PunctuationKind, TokenKind, TokenStream};

#[derive(Debug)]
pub(super) struct BuildLayout {
    machine_indent: String,
    body_open_end: usize,
    body_close_start: usize,
    entry_end_start: usize,
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
        let insertion_line_start = source[..self.entry_end_start]
            .rfind('\n')
            .map_or(0, |index| index + 1);
        let insertion_prefix = &source[insertion_line_start..self.entry_end_start];
        if !insertion_prefix.chars().all(char::is_whitespace) {
            return None;
        }
        let extra_indent = if self.entry_end_start == self.body_close_start {
            "    "
        } else {
            ""
        };
        let indent = format!("{insertion_prefix}{extra_indent}");
        replacement.insert_str(
            insertion_line_start,
            &format!(
                "{indent}{}{newline}",
                indent_continuation_lines(statement, &indent, newline)
            ),
        );
        Some(replacement)
    }

    // A replacement keeps the replaced row's first-line indent for every line.
    pub(super) fn replace_row(source: &str, row: &DependencyRow, statement: &str) -> String {
        let newline = if source.contains("\r\n") {
            "\r\n"
        } else {
            "\n"
        };
        let indent = line_indent(source, row.start).unwrap_or_default();
        let mut replacement = source.to_owned();
        replacement.replace_range(
            row.start..row.end,
            &indent_continuation_lines(statement, indent, newline),
        );
        replacement
    }
}

fn indent_continuation_lines(statement: &str, indent: &str, newline: &str) -> String {
    statement.replace('\n', &format!("{newline}{indent}"))
}

#[derive(Debug)]
pub(super) struct DependencyRow {
    pub(super) start: usize,
    pub(super) end: usize,
    pub(super) contains_comment: bool,
}

pub(super) fn discover_build_layout(
    parsed: &source_syntax::ParsedText<'_>,
    receiver: &str,
) -> Option<Option<BuildLayout>> {
    let source_syntax::ParsedText { tokens, syntax } = parsed;
    let source = tokens.source_text();
    let build = syntax.root_items().find_map(|item| match item {
        Item::Machine(machine) if machine.name.as_str(syntax) == BUILD_MACHINE_NAME => {
            Some(machine)
        }
        _ => None,
    })?;
    let semantic = semantic_indices(tokens);
    let name_span = build.name.source_span().span;
    let Some(name_position) = semantic.iter().position(|index| {
        let token = &tokens[*index];
        token.span == name_span && tokens.spelling(token) == BUILD_MACHINE_NAME
    }) else {
        return Some(None);
    };
    let expected_before = name_position
        .checked_sub(1)
        .and_then(|position| semantic.get(position))
        .is_some_and(|index| tokens.spelling(&tokens[*index]) == "machine");
    let expected_after = ["(", receiver, ":", "@", "mutable", "Build", ")", "{"];
    if !expected_before
        || expected_after.iter().enumerate().any(|(offset, expected)| {
            semantic
                .get(name_position + 1 + offset)
                .is_none_or(|index| tokens.spelling(&tokens[*index]) != *expected)
        })
    {
        return Some(None);
    }
    let machine_token = semantic[name_position - 1];
    let body_open = semantic[name_position + expected_after.len()];
    let Some(body_close) = matching_brace(tokens, body_open) else {
        return Some(None);
    };
    let machine_indent = line_indent(source, tokens[machine_token].span.start)
        .unwrap_or_default()
        .to_owned();
    let Some(dependency_rows) = dependency_rows(syntax, tokens, body_open, body_close) else {
        return Some(None);
    };
    let states = syntax.items.state_handles(build.states);
    let Some(entry) = states.first().map(|handle| syntax.items.state(*handle)) else {
        return Some(None);
    };
    let entry_boundary = syntax
        .items
        .statements(entry.statements)
        .iter()
        .find_map(|handle| match syntax.statements.statement(*handle) {
            StatementNode::Dispatch(handle) => {
                Some(syntax.statements.dispatch(*handle).source_span.span.start)
            }
            StatementNode::Transition(transition) => Some(transition.source_span.span.start),
            _ => None,
        })
        .or_else(|| {
            states.get(1).map(|handle| {
                let state = syntax.items.state(*handle);
                state.name.source_span().span.start
            })
        });
    let entry_end_start = if let Some(bound) = entry_boundary {
        let Some(start) = entry_insertion_start(tokens, body_open, body_close, bound) else {
            return Some(None);
        };
        start
    } else {
        tokens[body_close].span.start
    };
    Some(Some(BuildLayout {
        machine_indent,
        body_open_end: tokens[body_open].span.end,
        body_close_start: tokens[body_close].span.start,
        entry_end_start,
        dependency_rows,
    }))
}

fn entry_insertion_start(
    tokens: &TokenStream<'_>,
    body_open: usize,
    body_close: usize,
    bound: usize,
) -> Option<usize> {
    let mut depth = 0usize;
    let mut start = None;
    for token in tokens[body_open + 1..body_close]
        .iter()
        .filter(|token| !token.is_trivia())
    {
        start.get_or_insert(token.span.start);
        if token.span.start >= bound {
            return start;
        }
        match token.kind {
            TokenKind::Punctuation(
                PunctuationKind::LeftBrace
                | PunctuationKind::LeftParen
                | PunctuationKind::LeftBracket,
            ) => depth = depth.checked_add(1)?,
            TokenKind::Punctuation(
                PunctuationKind::RightBrace
                | PunctuationKind::RightParen
                | PunctuationKind::RightBracket,
            ) => {
                depth = depth.checked_sub(1)?;
            }
            TokenKind::Punctuation(PunctuationKind::Semicolon) if depth == 0 => start = None,
            _ => {}
        }
    }
    None
}

fn semantic_indices(tokens: &TokenStream<'_>) -> Vec<usize> {
    tokens
        .iter()
        .enumerate()
        .filter_map(|(index, token)| (!token.is_trivia()).then_some(index))
        .collect()
}

fn matching_brace(tokens: &TokenStream<'_>, open: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (index, token) in tokens.iter().enumerate().skip(open) {
        match token.punctuation() {
            Some(PunctuationKind::LeftBrace) => depth = depth.checked_add(1)?,
            Some(PunctuationKind::RightBrace) => {
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

// The projector owns row membership. The tokens supply byte and comment ranges for the edit.
fn dependency_rows(
    syntax: &SyntaxTrees,
    tokens: &TokenStream<'_>,
    body_open: usize,
    body_close: usize,
) -> Option<Vec<DependencyRow>> {
    let build = project_build_entry_syntax(syntax).ok()?;
    let rows = project_dependency_rows(syntax, &build).ok()?;
    let statements = top_level_statements(tokens, body_open, body_close);
    let token_range = |handle| {
        let anchor = match syntax.statements.statement(handle) {
            StatementNode::LocalData(local) => local.name.source_span().span,
            StatementNode::Call(call) => call.call(syntax).target.source_span().span,
            _ => return None,
        };
        statements.iter().position(|&(first, last)| {
            tokens[first].span.start <= anchor.start && anchor.end <= tokens[last].span.end
        })
    };
    rows.iter()
        .map(|row| {
            let handles = row.statements();
            let start = token_range(*handles.first()?)?;
            // An editable group covers adjacent, complete statements.
            for (offset, handle) in handles.iter().enumerate() {
                if token_range(*handle)? != start + offset {
                    return None;
                }
            }
            let first = statements[start].0;
            let last = statements[start + handles.len() - 1].1;
            // A preceding block may not end with a semicolon. Refuse to include its text in an edit.
            let anchor = match syntax.statements.statement(handles[0]) {
                StatementNode::LocalData(local) => (local.name.source_span().span.start, "let"),
                StatementNode::Call(call) if call.discards_result => {
                    (call.call(syntax).target.source_span().span.start, "_")
                }
                _ => return None,
            };
            let actual_start = (first..=last)
                .take_while(|&index| tokens[index].span.start < anchor.0)
                .filter(|&index| tokens.spelling(&tokens[index]) == anchor.1)
                .last()?;
            if actual_start != first {
                return None;
            }
            Some(DependencyRow {
                start: tokens[first].span.start,
                end: tokens[last].span.end,
                contains_comment: tokens[first..=last]
                    .iter()
                    .any(|token| token.comment().is_some()),
            })
        })
        .collect()
}

// Inclusive token ranges of the body's depth-0 statements, each ending at its
// semicolon. Nested blocks, calls and literals stay inside their statement.
fn top_level_statements(
    tokens: &TokenStream<'_>,
    body_open: usize,
    body_close: usize,
) -> Vec<(usize, usize)> {
    let mut statements = Vec::new();
    let mut depth = 0usize;
    let mut start = None;
    for index in semantic_indices(tokens)
        .into_iter()
        .filter(|index| *index > body_open && *index < body_close)
    {
        if depth == 0 {
            start.get_or_insert(index);
        }
        match tokens[index].punctuation() {
            Some(
                PunctuationKind::LeftBrace
                | PunctuationKind::LeftParen
                | PunctuationKind::LeftBracket,
            ) => depth += 1,
            Some(
                PunctuationKind::RightBrace
                | PunctuationKind::RightParen
                | PunctuationKind::RightBracket,
            ) => depth = depth.saturating_sub(1),
            Some(PunctuationKind::Semicolon) if depth == 0 => {
                if let Some(first) = start.take() {
                    statements.push((first, index));
                }
            }
            _ => {}
        }
    }
    statements
}

fn line_indent(source: &str, offset: usize) -> Option<&str> {
    let line_start = source[..offset].rfind('\n').map_or(0, |index| index + 1);
    let prefix = &source[line_start..offset];
    prefix.chars().all(char::is_whitespace).then_some(prefix)
}

#[cfg(test)]
mod tests {
    use super::discover_build_layout;

    #[test]
    fn quoted_delimiters_leave_the_build_body_and_dependency_row_intact() {
        let source = r#"use {omega}/build/dependency.omg;
machine build(builder: @mutable Build) {
    Build::application(builder, "}");
    _ = Build::dependency(builder, |dependency|::from_source(Source::Path { location: "(" }));
}
"#;
        let layout = discover_build_layout(&source_syntax::parse_text(source).unwrap(), "builder")
            .unwrap()
            .unwrap();
        assert_eq!(layout.body_close_start, source.rfind('}').unwrap());
        let [row] = layout.dependency_rows() else {
            panic!("one dependency row");
        };
        assert_eq!(
            &source[row.start..row.end],
            r#"_ = Build::dependency(builder, |dependency|::from_source(Source::Path { location: "(" }));"#
        );
    }
}
