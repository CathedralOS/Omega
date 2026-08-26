use crate::dependency_projection::{
    DependencyProjectionError, DependencySourceRequest, extract_from_source,
};
use psi_source_files_to_tokens::Lexer;
use psi_syntax_trees::item::Item;
use psi_tokens::TokenStream;
use psi_tokens_to_syntax_trees::parse_syntax_trees;
use sha2::{Digest, Sha256};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

const BUILD_FILE_NAME: &str = "build.omg";
const BUILD_MACHINE_NAME: &str = "build";
const BUILDER_PARAMETER_NAME: &str = "builder";

/// A conservative, non-mutating plan for changing one `build.omg` dependency.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildDependencyEditPlan {
    /// The exact requested row is already present.
    Unchanged,
    /// The source has a canonical edit point and may be replaced atomically
    /// after checking `expected_sha256` again.
    Automatic(BuildFileReplacement),
    /// The source is valid Omega but its layout or intent requires a person or
    /// reviewing agent to place the generated row.
    Manual(BuildDependencyManualPatch),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildFileReplacement {
    build_path: PathBuf,
    expected_sha256: [u8; 32],
    replacement_source: String,
}

impl BuildFileReplacement {
    pub fn build_path(&self) -> &Path {
        &self.build_path
    }

    pub fn expected_sha256(&self) -> &[u8; 32] {
        &self.expected_sha256
    }

    pub fn replacement_source(&self) -> &str {
        &self.replacement_source
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildDependencyManualPatch {
    build_path: PathBuf,
    expected_sha256: [u8; 32],
    reason: BuildDependencyManualReason,
    current_statement: Option<String>,
    proposed_statement: String,
}

impl BuildDependencyManualPatch {
    pub fn build_path(&self) -> &Path {
        &self.build_path
    }

    pub fn expected_sha256(&self) -> &[u8; 32] {
        &self.expected_sha256
    }

    pub fn reason(&self) -> BuildDependencyManualReason {
        self.reason
    }

    /// Canonical, compiler-generated text for the accepted row, when this is a
    /// replacement. This is never copied from package source.
    pub fn current_statement(&self) -> Option<&str> {
        self.current_statement.as_deref()
    }

    /// Canonical, compiler-generated text for the requested row. Every
    /// caller-controlled string is escaped as an Omega literal.
    pub fn proposed_statement(&self) -> &str {
        &self.proposed_statement
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildDependencyManualReason {
    NonCanonicalBuildSignature,
    NonCanonicalBuildBodyLayout,
    NonCanonicalDependencyRows,
    DependencyRowContainsComment,
    AcceptedRequestMissing,
    AcceptedRequestAmbiguous,
    CandidateAlreadyPresent,
    GeneratedEditRejected,
}

impl fmt::Display for BuildDependencyManualReason {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::NonCanonicalBuildSignature => {
                "the build entry is not the canonical `machine build(builder: &mut Build)` form"
            }
            Self::NonCanonicalBuildBodyLayout => {
                "the build entry closing brace has a noncanonical inline layout"
            }
            Self::NonCanonicalDependencyRows => {
                "the parsed dependency rows cannot be mapped uniquely to direct source statements"
            }
            Self::DependencyRowContainsComment => {
                "the accepted dependency row contains a comment that an automatic rewrite would discard"
            }
            Self::AcceptedRequestMissing => {
                "the accepted dependency row is not present in the current build projection"
            }
            Self::AcceptedRequestAmbiguous => {
                "the accepted dependency row occurs more than once in the current build projection"
            }
            Self::CandidateAlreadyPresent => {
                "the candidate dependency row is already present separately from the accepted row"
            }
            Self::GeneratedEditRejected => {
                "the generated source did not project to the exact requested dependency rows"
            }
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildDependencyEditError {
    ReadBuildFile { path: PathBuf, message: String },
    InvalidBuildFileEncoding { path: PathBuf },
    InvalidBuild(DependencyProjectionError),
}

impl fmt::Display for BuildDependencyEditError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReadBuildFile { path, message } => {
                write!(formatter, "cannot read {}: {message}", path.display())
            }
            Self::InvalidBuildFileEncoding { path } => {
                write!(formatter, "{} is not UTF-8 Omega source", path.display())
            }
            Self::InvalidBuild(error) => {
                write!(formatter, "cannot edit invalid package build: {error}")
            }
        }
    }
}

impl std::error::Error for BuildDependencyEditError {}

pub fn plan_dependency_addition(
    package_root: impl AsRef<Path>,
    request: &DependencySourceRequest,
) -> Result<BuildDependencyEditPlan, BuildDependencyEditError> {
    let (build_path, source) = read_build_source(package_root.as_ref())?;
    plan_addition_from_source(build_path, source, request)
}

pub fn plan_dependency_replacement(
    package_root: impl AsRef<Path>,
    accepted: &DependencySourceRequest,
    candidate: &DependencySourceRequest,
) -> Result<BuildDependencyEditPlan, BuildDependencyEditError> {
    let (build_path, source) = read_build_source(package_root.as_ref())?;
    plan_replacement_from_source(build_path, source, accepted, candidate)
}

/// Render one ordinary Omega statement. Caller-controlled strings cannot add
/// syntax, lines, comments, or review prose because they remain escaped bytes
/// inside string literals.
pub fn canonical_dependency_statement(request: &DependencySourceRequest) -> String {
    let (operation, alias) = match request.explicit_alias() {
        Some(alias) => (
            "depend_as",
            format!(
                "{}, ",
                psi_source::display_literal_bytes(alias.as_str().as_bytes())
            ),
        ),
        None => ("depend", String::new()),
    };
    let source = match request {
        DependencySourceRequest::Path { location, .. } => format!(
            "Source::Path {{ location: {} }}",
            psi_source::display_literal_bytes(location.as_bytes())
        ),
        DependencySourceRequest::Git {
            repository,
            revision,
            ..
        } => format!(
            "Source::Git {{ repository: {}, revision: {} }}",
            psi_source::display_literal_bytes(repository.as_bytes()),
            psi_source::display_literal_bytes(revision.as_bytes())
        ),
    };
    format!("{BUILDER_PARAMETER_NAME}.{operation}({alias}{source});")
}

fn read_build_source(package_root: &Path) -> Result<(PathBuf, String), BuildDependencyEditError> {
    let build_path = package_root.join(BUILD_FILE_NAME);
    let bytes = fs::read(&build_path).map_err(|error| BuildDependencyEditError::ReadBuildFile {
        path: build_path.clone(),
        message: error.to_string(),
    })?;
    let source = String::from_utf8(bytes).map_err(|_| {
        BuildDependencyEditError::InvalidBuildFileEncoding {
            path: build_path.clone(),
        }
    })?;
    Ok((build_path, source))
}

fn plan_addition_from_source(
    build_path: PathBuf,
    source: String,
    request: &DependencySourceRequest,
) -> Result<BuildDependencyEditPlan, BuildDependencyEditError> {
    let requests = extract_from_source(&source).map_err(BuildDependencyEditError::InvalidBuild)?;
    if requests.contains(request) {
        return Ok(BuildDependencyEditPlan::Unchanged);
    }
    let digest = source_digest(&source);
    let statement = canonical_dependency_statement(request);
    let Some(layout) = build_layout(&source)? else {
        let replacement = append_build_machine(&source, &statement);
        return validated_automatic_addition(
            build_path,
            digest,
            replacement,
            requests,
            request,
            statement,
        );
    };
    let Some(layout) = layout else {
        return Ok(manual_patch(
            build_path,
            digest,
            BuildDependencyManualReason::NonCanonicalBuildSignature,
            None,
            statement,
        ));
    };
    let Some(replacement) = insert_statement(&source, &layout, &statement) else {
        return Ok(manual_patch(
            build_path,
            digest,
            BuildDependencyManualReason::NonCanonicalBuildBodyLayout,
            None,
            statement,
        ));
    };
    validated_automatic_addition(
        build_path,
        digest,
        replacement,
        requests,
        request,
        statement,
    )
}

fn validated_automatic_addition(
    build_path: PathBuf,
    digest: [u8; 32],
    replacement: String,
    mut expected: Vec<DependencySourceRequest>,
    request: &DependencySourceRequest,
    statement: String,
) -> Result<BuildDependencyEditPlan, BuildDependencyEditError> {
    expected.push(request.clone());
    if extract_from_source(&replacement).ok().as_ref() != Some(&expected) {
        return Ok(manual_patch(
            build_path,
            digest,
            BuildDependencyManualReason::GeneratedEditRejected,
            None,
            statement,
        ));
    }
    Ok(BuildDependencyEditPlan::Automatic(BuildFileReplacement {
        build_path,
        expected_sha256: digest,
        replacement_source: replacement,
    }))
}

fn plan_replacement_from_source(
    build_path: PathBuf,
    source: String,
    accepted: &DependencySourceRequest,
    candidate: &DependencySourceRequest,
) -> Result<BuildDependencyEditPlan, BuildDependencyEditError> {
    let requests = extract_from_source(&source).map_err(BuildDependencyEditError::InvalidBuild)?;
    if accepted == candidate {
        return Ok(if requests.contains(candidate) {
            BuildDependencyEditPlan::Unchanged
        } else {
            manual_patch(
                build_path,
                source_digest(&source),
                BuildDependencyManualReason::AcceptedRequestMissing,
                Some(canonical_dependency_statement(accepted)),
                canonical_dependency_statement(candidate),
            )
        });
    }
    let digest = source_digest(&source);
    let current_statement = canonical_dependency_statement(accepted);
    let proposed_statement = canonical_dependency_statement(candidate);
    let accepted_indices = requests
        .iter()
        .enumerate()
        .filter_map(|(index, request)| (request == accepted).then_some(index))
        .collect::<Vec<_>>();
    let [accepted_index] = accepted_indices.as_slice() else {
        let reason = if accepted_indices.is_empty() {
            BuildDependencyManualReason::AcceptedRequestMissing
        } else {
            BuildDependencyManualReason::AcceptedRequestAmbiguous
        };
        return Ok(manual_patch(
            build_path,
            digest,
            reason,
            Some(current_statement),
            proposed_statement,
        ));
    };
    if requests.iter().any(|request| request == candidate) {
        return Ok(manual_patch(
            build_path,
            digest,
            BuildDependencyManualReason::CandidateAlreadyPresent,
            Some(current_statement),
            proposed_statement,
        ));
    }
    let Some(layout) = build_layout(&source)? else {
        return Ok(manual_patch(
            build_path,
            digest,
            BuildDependencyManualReason::AcceptedRequestMissing,
            Some(current_statement),
            proposed_statement,
        ));
    };
    let Some(layout) = layout else {
        return Ok(manual_patch(
            build_path,
            digest,
            BuildDependencyManualReason::NonCanonicalBuildSignature,
            Some(current_statement),
            proposed_statement,
        ));
    };
    if layout.dependency_rows.len() != requests.len() {
        return Ok(manual_patch(
            build_path,
            digest,
            BuildDependencyManualReason::NonCanonicalDependencyRows,
            Some(current_statement),
            proposed_statement,
        ));
    }
    let row = &layout.dependency_rows[*accepted_index];
    if row.contains_comment {
        return Ok(manual_patch(
            build_path,
            digest,
            BuildDependencyManualReason::DependencyRowContainsComment,
            Some(current_statement),
            proposed_statement,
        ));
    }
    let mut replacement = source.clone();
    replacement.replace_range(row.start..row.end, &proposed_statement);
    let mut expected = requests;
    expected[*accepted_index] = candidate.clone();
    if extract_from_source(&replacement).ok().as_ref() != Some(&expected) {
        return Ok(manual_patch(
            build_path,
            digest,
            BuildDependencyManualReason::GeneratedEditRejected,
            Some(current_statement),
            proposed_statement,
        ));
    }
    Ok(BuildDependencyEditPlan::Automatic(BuildFileReplacement {
        build_path,
        expected_sha256: digest,
        replacement_source: replacement,
    }))
}

fn manual_patch(
    build_path: PathBuf,
    expected_sha256: [u8; 32],
    reason: BuildDependencyManualReason,
    current_statement: Option<String>,
    proposed_statement: String,
) -> BuildDependencyEditPlan {
    BuildDependencyEditPlan::Manual(BuildDependencyManualPatch {
        build_path,
        expected_sha256,
        reason,
        current_statement,
        proposed_statement,
    })
}

fn source_digest(source: &str) -> [u8; 32] {
    Sha256::digest(source.as_bytes()).into()
}

#[derive(Debug)]
struct BuildLayout {
    machine_indent: String,
    body_open_end: usize,
    body_close_start: usize,
    dependency_rows: Vec<DependencyRow>,
}

#[derive(Debug)]
struct DependencyRow {
    start: usize,
    end: usize,
    contains_comment: bool,
}

/// `None` means no build machine. `Some(None)` means a valid but noncanonical
/// build signature that must be edited manually.
fn build_layout(source: &str) -> Result<Option<Option<BuildLayout>>, BuildDependencyEditError> {
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
    let semantic = semantic_indices(tokens);
    let semantic = semantic
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
        {
            if let Some(end_position) = dependency_row_end(tokens, &semantic, position + 3) {
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

fn insert_statement(source: &str, layout: &BuildLayout, statement: &str) -> Option<String> {
    let newline = if source.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let interior = &source[layout.body_open_end..layout.body_close_start];
    let mut replacement = source.to_owned();
    if interior.trim().is_empty() {
        let inner_indent = format!("{}    ", layout.machine_indent);
        replacement.replace_range(
            layout.body_open_end..layout.body_close_start,
            &format!(
                "{newline}{inner_indent}{statement}{newline}{}",
                layout.machine_indent
            ),
        );
        return Some(replacement);
    }
    let close_line_start = source[..layout.body_close_start]
        .rfind('\n')
        .map_or(0, |index| index + 1);
    let close_prefix = &source[close_line_start..layout.body_close_start];
    if !close_prefix.chars().all(char::is_whitespace) {
        return None;
    }
    replacement.insert_str(
        close_line_start,
        &format!("{}    {statement}{newline}", close_prefix),
    );
    Some(replacement)
}

fn append_build_machine(source: &str, statement: &str) -> String {
    let newline = if source.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let separator = if source.is_empty() {
        String::new()
    } else if source.ends_with(newline) {
        newline.to_owned()
    } else {
        format!("{newline}{newline}")
    };
    format!(
        "{source}{separator}machine build(builder: &mut Build) {{{newline}    {statement}{newline}}}{newline}"
    )
}

fn line_indent(source: &str, offset: usize) -> Option<&str> {
    let line_start = source[..offset].rfind('\n').map_or(0, |index| index + 1);
    let prefix = &source[line_start..offset];
    prefix.chars().all(char::is_whitespace).then_some(prefix)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::AliasName;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

    fn path(location: &str) -> DependencySourceRequest {
        DependencySourceRequest::Path {
            explicit_alias: None,
            location: location.to_owned(),
        }
    }

    fn git(repository: &str, revision: &str) -> DependencySourceRequest {
        DependencySourceRequest::Git {
            explicit_alias: None,
            repository: repository.to_owned(),
            revision: revision.to_owned(),
        }
    }

    fn automatic(plan: BuildDependencyEditPlan) -> BuildFileReplacement {
        let BuildDependencyEditPlan::Automatic(replacement) = plan else {
            panic!("expected automatic edit: {plan:?}");
        };
        replacement
    }

    fn fixture_root() -> PathBuf {
        let sequence = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "omega-package-dependency-edit-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("create fixture");
        root
    }

    #[test]
    fn adds_to_empty_canonical_build_without_mutating_input() {
        let source = "machine build(builder: &mut Build) {}\n".to_owned();
        let replacement = automatic(
            plan_addition_from_source(PathBuf::from("build.omg"), source.clone(), &path("../math"))
                .expect("plan addition"),
        );

        assert_eq!(source, "machine build(builder: &mut Build) {}\n");
        assert_eq!(
            extract_from_source(replacement.replacement_source()).expect("project replacement"),
            vec![path("../math")]
        );
        assert!(
            replacement
                .replacement_source()
                .contains("    builder.depend(Source::Path { location: \"../math\" });")
        );
    }

    #[test]
    fn creates_a_build_machine_when_the_valid_file_has_none() {
        let source = "target windows_x64 { }\n".to_owned();
        let replacement = automatic(
            plan_addition_from_source(PathBuf::from("build.omg"), source.clone(), &path("vendor"))
                .expect("plan addition"),
        );

        assert!(replacement.replacement_source().starts_with(&source));
        assert_eq!(
            extract_from_source(replacement.replacement_source()).expect("project replacement"),
            vec![path("vendor")]
        );
    }

    #[test]
    fn appends_after_existing_build_work_and_preserves_it() {
        let source = r#"machine build(builder: &mut Build) {
    builder.target(Target::Host);
}
"#
        .to_owned();
        let replacement = automatic(
            plan_addition_from_source(PathBuf::from("build.omg"), source, &path("vendor"))
                .expect("plan addition"),
        );

        assert!(
            replacement
                .replacement_source()
                .contains("    builder.target(Target::Host);\n    builder.depend")
        );
    }

    #[test]
    fn noncanonical_signature_yields_generated_manual_patch() {
        let source = "machine build(builder: &mut Build, profile: u32) {}\n".to_owned();
        let plan = plan_addition_from_source(PathBuf::from("build.omg"), source, &path("vendor"))
            .expect("plan addition");
        let BuildDependencyEditPlan::Manual(patch) = plan else {
            panic!("expected manual patch");
        };

        assert_eq!(
            patch.reason(),
            BuildDependencyManualReason::NonCanonicalBuildSignature
        );
        assert_eq!(
            patch.proposed_statement(),
            "builder.depend(Source::Path { location: \"vendor\" });"
        );
    }

    #[test]
    fn replaces_a_semantically_canonical_row_without_relying_on_formatting() {
        let accepted = git("https://example.test/repo.git", "old");
        let candidate = git("https://example.test/repo.git", "new");
        let source = r#"machine build(builder: &mut Build) {
    builder.depend(
        Source::Git {
            revision: "old",
            repository: "https://example.test/repo.git"
        }
    );
}
"#
        .to_owned();
        let replacement = automatic(
            plan_replacement_from_source(PathBuf::from("build.omg"), source, &accepted, &candidate)
                .expect("plan replacement"),
        );

        assert_eq!(
            extract_from_source(replacement.replacement_source()).expect("project replacement"),
            vec![candidate]
        );
    }

    #[test]
    fn comments_inside_a_replaced_row_force_manual_placement() {
        let accepted = path("vendor");
        let candidate = path("vendor-next");
        let source = r#"machine build(builder: &mut Build) {
    builder.depend(/* retained intent */ Source::Path { location: "vendor" });
}
"#
        .to_owned();
        let plan =
            plan_replacement_from_source(PathBuf::from("build.omg"), source, &accepted, &candidate)
                .expect("plan replacement");
        let BuildDependencyEditPlan::Manual(patch) = plan else {
            panic!("expected manual patch");
        };

        assert_eq!(
            patch.reason(),
            BuildDependencyManualReason::DependencyRowContainsComment
        );
    }

    #[test]
    fn generated_rows_escape_all_caller_controlled_strings() {
        let request = DependencySourceRequest::Git {
            explicit_alias: Some(AliasName::parse("safe_alias").expect("alias")),
            repository: "https://example.test/\"repo\n// injected".to_owned(),
            revision: "main\rnext".to_owned(),
        };
        let statement = canonical_dependency_statement(&request);
        let source = format!("machine build(builder: &mut Build) {{\n    {statement}\n}}\n");

        assert!(!statement.contains("\n// injected"));
        assert_eq!(
            extract_from_source(&source).expect("escaped statement parses"),
            vec![request]
        );
    }

    #[test]
    fn exact_existing_request_is_unchanged() {
        let request = path("vendor");
        let source = format!(
            "machine build(builder: &mut Build) {{\n    {}\n}}\n",
            canonical_dependency_statement(&request)
        );

        assert_eq!(
            plan_addition_from_source(PathBuf::from("build.omg"), source, &request)
                .expect("plan addition"),
            BuildDependencyEditPlan::Unchanged
        );
    }

    #[test]
    fn public_file_planner_binds_the_expected_digest_without_writing() {
        let root = fixture_root();
        let build_path = root.join(BUILD_FILE_NAME);
        let source = "machine build(builder: &mut Build) {}\n";
        fs::write(&build_path, source).expect("write fixture build");

        let replacement = automatic(
            plan_dependency_addition(&root, &path("vendor")).expect("plan file addition"),
        );

        assert_eq!(
            fs::read_to_string(&build_path).expect("read fixture"),
            source
        );
        assert_eq!(replacement.build_path(), build_path);
        assert_eq!(replacement.expected_sha256(), &source_digest(source));
        fs::remove_dir_all(root).expect("remove fixture");
    }
}
