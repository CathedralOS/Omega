// Source layout discovery and insertion boundaries for conservative `build.omg` dependency edits.
// `discover_build_layout` returns `Ok(None)` when no `build` machine exists and `Ok(Some(None))` when a valid `build` machine has a noncanonical signature or body layout requiring manual editing.
// Dependency declarations belong to the implicit entry before its first transition or nested state; `entry_insertion_start` locates the last top-level semicolon preceding that boundary so adding a dependency never splits a transition or makes the row conditional.

use crate::declarations::dependencies::edit::{
    BUILD_MACHINE_NAME, BUILDER_PARAMETER_NAME, BuildDependencyEditError,
};
use crate::declarations::dependencies::read::DependencyProjectionError;
use build_declarations::{
    BUILD_TYPE_NAME, DEPENDENCY_DESCRIPTION_TYPE_NAME, DEPENDENCY_HELPER_OWNER,
    DependencyOperation, FROM_SOURCE_HELPER_NAME,
};
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
    source: &str,
) -> Result<Option<Option<BuildLayout>>, BuildDependencyEditError> {
    let psi_compiler::ParsedText { tokens, syntax } =
        psi_compiler::parse_text(source).map_err(|error| {
            BuildDependencyEditError::InvalidBuild(match error {
                psi_compiler::SourceParseError::Lex { message } => {
                    DependencyProjectionError::Lex { message }
                }
                psi_compiler::SourceParseError::Parse { message } => {
                    DependencyProjectionError::Parse { message }
                }
            })
        })?;
    let Some(build) = syntax.root_items().find_map(|item| match item {
        Item::Machine(machine) if machine.name.as_str(&syntax) == BUILD_MACHINE_NAME => {
            Some(machine)
        }
        _ => None,
    }) else {
        return Ok(None);
    };
    let semantic = semantic_indices(&tokens);
    let name_span = build.name.source_span().span;
    let Some(name_position) = semantic.iter().position(|index| {
        let token = &tokens[*index];
        token.span == name_span && tokens.text(token) == BUILD_MACHINE_NAME
    }) else {
        return Ok(Some(None));
    };
    let expected_before = name_position
        .checked_sub(1)
        .and_then(|position| semantic.get(position))
        .is_some_and(|index| tokens.text(&tokens[*index]) == "machine");
    let expected_after = ["(", "builder", ":", "&", "mut", "Build", ")", "{"];
    if !expected_before
        || expected_after.iter().enumerate().any(|(offset, expected)| {
            semantic
                .get(name_position + 1 + offset)
                .is_none_or(|index| tokens.text(&tokens[*index]) != *expected)
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
    let states = syntax.items.state_handles(build.states);
    let Some(entry) = states.first().map(|handle| syntax.items.state(*handle)) else {
        return Ok(Some(None));
    };
    let entry_boundary = syntax
        .items
        .statements(entry.statements)
        .iter()
        .find_map(|handle| match syntax.statements.statement(*handle) {
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
        let Some(start) = entry_insertion_start(&tokens, body_open, body_close, bound) else {
            return Ok(Some(None));
        };
        start
    } else {
        tokens[body_close].span.start
    };
    Ok(Some(Some(BuildLayout {
        machine_indent,
        body_open_end: tokens[body_open].span.end,
        body_close_start: tokens[body_close].span.start,
        entry_end_start,
        dependency_rows,
    })))
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
        .filter(|token| !token.is_non_semantic())
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
        .filter_map(|(index, token)| (!token.is_non_semantic()).then_some(index))
        .collect()
}

fn matching_brace(tokens: &TokenStream<'_>, open: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (index, token) in tokens.iter().enumerate().skip(open) {
        match tokens.text(token).try_as_str() {
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

// Rows are the canonical top-level registration spellings the shared row
// grammar projects: one bound or discarded `Build::<operation>(...)` statement, or the
// three-statement aliased description journey starting at its `let mut`.
fn dependency_rows(
    tokens: &TokenStream<'_>,
    body_open: usize,
    body_close: usize,
) -> Vec<DependencyRow> {
    let statements = top_level_statements(tokens, body_open, body_close);
    let discarded = DependencyOperation::ALL.map(|operation| {
        format!(
            "_={BUILD_TYPE_NAME}::{}({BUILDER_PARAMETER_NAME},",
            operation.name()
        )
    });
    let bound = DependencyOperation::ALL.map(|operation| {
        format!(
            ":DependencyRegistration={BUILD_TYPE_NAME}::{}({BUILDER_PARAMETER_NAME},",
            operation.name()
        )
    });
    let description_marker = format!(
        ":{DEPENDENCY_DESCRIPTION_TYPE_NAME}={DEPENDENCY_HELPER_OWNER}::{FROM_SOURCE_HELPER_NAME}("
    );
    let is_registration = |text: &str| {
        discarded
            .iter()
            .any(|prefix| text.starts_with(prefix.as_str()))
            || (text.starts_with("let")
                && !text.starts_with("letmut")
                && bound.iter().any(|marker| text.contains(marker.as_str())))
    };
    let mut rows = Vec::new();
    let mut position = 0;
    while position < statements.len() {
        let (first, _) = statements[position];
        let text = compact_text(tokens, statements[position]);
        let length = if is_registration(&text) {
            1
        } else if text.starts_with("letmut") && text.contains(description_marker.as_str()) {
            3
        } else {
            position += 1;
            continue;
        };
        let Some(&(_, last)) = statements.get(position + length - 1) else {
            break;
        };
        rows.push(DependencyRow {
            start: tokens[first].span.start,
            end: tokens[last].span.end,
            contains_comment: tokens[first..=last]
                .iter()
                .any(|token| token.comment().is_some()),
        });
        position += length;
    }
    rows
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
        match tokens.text(&tokens[index]).try_as_str() {
            Some("{" | "(" | "[") => depth += 1,
            Some("}" | ")" | "]") => depth = depth.saturating_sub(1),
            Some(";") if depth == 0 => {
                if let Some(first) = start.take() {
                    statements.push((first, index));
                }
            }
            _ => {}
        }
    }
    statements
}

fn compact_text(tokens: &TokenStream<'_>, (first, last): (usize, usize)) -> String {
    (first..=last)
        .filter(|index| !tokens[*index].is_non_semantic())
        .filter_map(|index| tokens.text(&tokens[index]).try_as_str())
        .collect()
}

fn line_indent(source: &str, offset: usize) -> Option<&str> {
    let line_start = source[..offset].rfind('\n').map_or(0, |index| index + 1);
    let prefix = &source[line_start..offset];
    prefix.chars().all(char::is_whitespace).then_some(prefix)
}
