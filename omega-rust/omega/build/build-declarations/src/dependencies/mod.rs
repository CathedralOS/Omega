//! Dependency rows of the canonical root `Build` entry.
//!
//! Dependency edges are declared as direct unconditional
//! `builder.<operation>(...)` statements in `build.omg`. That grammar is
//! part of the one build-role grammar this crate owns: the package manager
//! interprets each retained row's literals into source requests for
//! acquisition, and the compiler consumes the same checked occurrences when
//! it binds imports. Neither consumer evaluates build code, imports,
//! helpers, or control flow, and neither may fall back to the other scope.

use std::fmt;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::ExpressionHandle;
use syntax_trees::statement::{StatementHandle, StatementNode};

use crate::BuildEntrySyntaxProjection;

/// Which authorized context one direct dependency edge serves.
///
/// `depend`/`depend_as` rows authorize product imports. `build_depend`/
/// `build_depend_as` rows authorize the host build context. The two scopes
/// are distinct: an alias or a package may appear in both, and neither scope
/// falls back to the other.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DependencyPurpose {
    Product,
    Build,
}

impl DependencyPurpose {
    pub const ALL: [Self; 2] = [Self::Product, Self::Build];

    pub const fn is_product(self) -> bool {
        matches!(self, Self::Product)
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Product => "product",
            Self::Build => "build",
        }
    }
}

/// One unconditional `Build` dependency operation that projects an edge.
///
/// The operation name selects both the authorized scope and the canonical
/// argument shape: `depend` and `build_depend` take one source argument;
/// `depend_as` and `build_depend_as` take a direct alias literal followed
/// by one source argument.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DependencyOperation {
    Depend,
    DependAs,
    BuildDepend,
    BuildDependAs,
}

impl DependencyOperation {
    pub const ALL: [Self; 4] = [
        Self::Depend,
        Self::DependAs,
        Self::BuildDepend,
        Self::BuildDependAs,
    ];

    /// Classify a `build.omg` call target as an edge-projecting operation.
    ///
    /// Conditional `*_when` names are recognized dependency vocabulary but
    /// never project a row; use [`is_dependency_call_name`] to cover the
    /// full family when rejecting unprojected syntax.
    pub fn classify(target: &str) -> Option<Self> {
        Some(match target {
            "depend" => Self::Depend,
            "depend_as" => Self::DependAs,
            "build_depend" => Self::BuildDepend,
            "build_depend_as" => Self::BuildDependAs,
            _ => return None,
        })
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Depend => "depend",
            Self::DependAs => "depend_as",
            Self::BuildDepend => "build_depend",
            Self::BuildDependAs => "build_depend_as",
        }
    }

    /// The authorized context the operation's row serves.
    pub const fn purpose(self) -> DependencyPurpose {
        match self {
            Self::Depend | Self::DependAs => DependencyPurpose::Product,
            Self::BuildDepend | Self::BuildDependAs => DependencyPurpose::Build,
        }
    }

    /// Whether the row's first argument is a direct alias literal.
    pub const fn takes_alias(self) -> bool {
        matches!(self, Self::DependAs | Self::BuildDependAs)
    }
}

/// The conditional `*_when` dependency call names.
///
/// These are recognized `Build` vocabulary so consumers can reject them
/// deliberately. A conditional call can never project an unconditional
/// edge, so [`project_dependency_rows`] leaves them unclassified rather
/// than admitting them as rows or letting them pass silently.
pub const CONDITIONAL_DEPENDENCY_CALL_NAMES: [&str; 4] = [
    "depend_when",
    "depend_as_when",
    "build_depend_when",
    "build_depend_as_when",
];

/// Whether `target` names any recognized dependency call, including the
/// conditional forms that never project an edge.
pub fn is_dependency_call_name(target: &str) -> bool {
    DependencyOperation::classify(target).is_some()
        || CONDITIONAL_DEPENDENCY_CALL_NAMES.contains(&target)
}

/// One checked dependency occurrence: a direct unconditional
/// `builder.<operation>(...)` statement in the root build entry.
///
/// The row retains its statement and raw argument handles so a consumer can
/// tie authored syntax to acquired edges. Interpreting the alias and source
/// literals is consumer policy, not grammar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyRow {
    statement: StatementHandle,
    operation: DependencyOperation,
    alias: Option<ExpressionHandle>,
    source: ExpressionHandle,
}

impl DependencyRow {
    /// The statement carrying this row, in authored order.
    pub const fn statement(&self) -> StatementHandle {
        self.statement
    }

    pub const fn operation(&self) -> DependencyOperation {
        self.operation
    }

    /// The authorized context this row's edge serves.
    pub const fn purpose(&self) -> DependencyPurpose {
        self.operation.purpose()
    }

    /// The direct alias literal for `depend_as`/`build_depend_as` rows.
    pub const fn alias(&self) -> Option<ExpressionHandle> {
        self.alias
    }

    /// The source argument expression, still raw syntax.
    pub const fn source(&self) -> ExpressionHandle {
        self.source
    }
}

/// How one direct `builder.<operation>(...)` statement departed from the
/// dependency row grammar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DependencyRowError {
    /// The receiver was not the root build entry's `builder` parameter —
    /// including one merely prefixed by `self`.
    WrongReceiver,
    /// The call's canonical argument shape was violated: `depend` and
    /// `build_depend` take exactly one source argument, `depend_as` and
    /// `build_depend_as` take one direct alias literal followed by one
    /// source argument, and no form accepts static, evidence, operational,
    /// or discard modifiers.
    WrongArguments,
}

impl fmt::Display for DependencyRowError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::WrongReceiver => {
                "dependency request receiver must be the root build machine's first parameter"
            }
            Self::WrongArguments => {
                "dependency calls take one source argument, or one direct alias literal followed by one source argument, and accept no static, evidence, operational, or discard modifiers"
            }
        })
    }
}

impl std::error::Error for DependencyRowError {}

/// Project the direct dependency rows of a root build entry previously
/// validated by [`crate::project_build_entry_syntax`].
///
/// Every direct call statement naming an edge-projecting operation on the
/// `builder` receiver becomes one retained row, in authored order. A
/// receiver that is not `builder` — including one merely prefixed by
/// `self` — or a noncanonical argument shape rejects the whole projection.
/// Conditional `*_when` calls are recognized vocabulary but carry no
/// unconditional purpose, so they are not rows: consumers reject them as
/// unprojected syntax through [`is_dependency_call_name`].
pub fn project_dependency_rows(
    syntax_trees: &SyntaxTrees,
    build: &BuildEntrySyntaxProjection,
) -> Result<Vec<DependencyRow>, DependencyRowError> {
    let entry = syntax_trees.items.state(build.build_entry());
    let builder_name = syntax_trees
        .items
        .state_parameter(build.builder_parameter())
        .name
        .as_str();
    let mut rows = Vec::new();
    for statement_handle in syntax_trees.items.statements(entry.statements) {
        let StatementNode::Call(call) = syntax_trees.statements.statement(*statement_handle) else {
            continue;
        };
        let Some(operation) = DependencyOperation::classify(call.target.as_str()) else {
            continue;
        };
        if call.receiver_starts_at_self
            || !matches!(
                syntax_trees
                    .statements
                    .identifier_path_members(call.receiver),
                [receiver] if receiver.as_str() == builder_name
            )
        {
            return Err(DependencyRowError::WrongReceiver);
        }
        if !call.machine_arguments.is_empty()
            || !call.evidence_arguments.is_empty()
            || call.operational_acknowledgement != Default::default()
            || call.discards_result
        {
            return Err(DependencyRowError::WrongArguments);
        }
        let arguments = syntax_trees.statements.expression_handles(call.arguments);
        let (alias, source) = if operation.takes_alias() {
            let [alias, source] = arguments else {
                return Err(DependencyRowError::WrongArguments);
            };
            (Some(*alias), *source)
        } else {
            let [source] = arguments else {
                return Err(DependencyRowError::WrongArguments);
            };
            (None, *source)
        };
        rows.push(DependencyRow {
            statement: *statement_handle,
            operation,
            alias,
            source,
        });
    }
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::{
        CONDITIONAL_DEPENDENCY_CALL_NAMES, DependencyOperation, DependencyPurpose, DependencyRow,
        DependencyRowError, is_dependency_call_name, project_dependency_rows,
    };
    use crate::{BuildDeclarationError, project_build_entry_syntax};
    use source_files_to_tokens::Lexer;
    use syntax_trees::SyntaxTrees;
    use syntax_trees::expression::{ExpressionHandle, ExpressionNode};
    use tokens_to_syntax_trees::parse_syntax_trees;

    fn rows(source: &str) -> Result<(SyntaxTrees, Vec<DependencyRow>), DependencyRowError> {
        let tokens = Lexer::new(source).tokenize().expect("lex build");
        let syntax_trees = parse_syntax_trees(&tokens).expect("parse build");
        let build = project_build_entry_syntax(&syntax_trees).expect("valid build entry");
        project_dependency_rows(&syntax_trees, &build).map(|rows| (syntax_trees, rows))
    }

    fn string_literal(trees: &SyntaxTrees, handle: ExpressionHandle) -> String {
        let ExpressionNode::String(bytes) = trees.expressions.expression(handle) else {
            panic!("expected a direct string literal")
        };
        std::str::from_utf8(bytes).expect("utf8 literal").to_owned()
    }

    #[test]
    fn rows_retain_their_purpose_and_literal_occurrences() {
        let (trees, rows) = rows(
            r#"machine build(builder: &mut Build) {
    builder.application("probe");
    builder.depend(Source::Path { location: "math" });
    builder.depend_as("other", Source::Path { location: "other" });
    builder.build_depend(Source::Git { repository: "r", revision: "v" });
    builder.build_depend_as("host", Source::Path { location: "host" });
}
"#,
        )
        .expect("project rows");

        assert_eq!(
            rows.iter().map(DependencyRow::purpose).collect::<Vec<_>>(),
            [
                DependencyPurpose::Product,
                DependencyPurpose::Product,
                DependencyPurpose::Build,
                DependencyPurpose::Build,
            ]
        );
        assert_eq!(rows[1].operation(), DependencyOperation::DependAs);
        assert_eq!(rows[3].operation(), DependencyOperation::BuildDependAs);
        assert_eq!(rows[0].alias(), None);
        assert_eq!(
            string_literal(&trees, rows[1].alias().expect("alias row")),
            "other"
        );
        assert_eq!(
            string_literal(&trees, rows[3].alias().expect("build alias row")),
            "host"
        );
        for row in &rows {
            assert!(matches!(
                trees.expressions.expression(row.source()),
                ExpressionNode::StructLiteral(_)
            ));
        }
    }

    #[test]
    fn a_wrong_receiver_or_argument_shape_rejects_the_row() {
        let wrong_receiver = [
            "builder.application(\"probe\");\n    self.depend(Source::Path { location: \"x\" });",
            "builder.application(\"probe\");\n    helper.depend(Source::Path { location: \"x\" });",
        ];
        for statements in wrong_receiver {
            let source = format!("machine build(builder: &mut Build) {{\n{statements}\n}}\n");
            assert_eq!(
                rows(&source).map(|pair| pair.1),
                Err(DependencyRowError::WrongReceiver),
                "{source}"
            );
        }
        let wrong_arguments = [
            "builder.depend()",
            "builder.depend_as(\"only\")",
            "builder.build_depend(Source::Path { location: \"x\" }, Source::Path { location: \"y\" })",
            "builder.depend_as(\"x\", Source::Path { location: \"x\" }, 1)",
        ];
        for statement in wrong_arguments {
            let source = format!(
                "machine build(builder: &mut Build) {{\n    builder.application(\"probe\");\n    {statement};\n}}\n"
            );
            assert_eq!(
                rows(&source).map(|pair| pair.1),
                Err(DependencyRowError::WrongArguments),
                "{source}"
            );
        }
    }

    #[test]
    fn conditional_dependency_calls_are_recognized_but_never_rows() {
        for target in CONDITIONAL_DEPENDENCY_CALL_NAMES {
            assert!(is_dependency_call_name(target), "{target}");
            assert_eq!(DependencyOperation::classify(target), None, "{target}");
        }
        for operation in DependencyOperation::ALL {
            assert!(is_dependency_call_name(operation.name()));
            assert_eq!(
                DependencyOperation::classify(operation.name()),
                Some(operation)
            );
        }
        let (.., rows) = rows(
            r#"machine build(builder: &mut Build) {
    builder.application("probe");
    builder.depend_when(flag(), Source::Path { location: "x" });
    builder.depend(Source::Path { location: "math" });
}
"#,
        )
        .expect("conditional calls are not rows");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].operation(), DependencyOperation::Depend);
    }

    #[test]
    fn rows_require_the_canonical_build_entry() {
        let tokens = Lexer::new(
            "machine helper() {\n    builder.depend(Source::Path { location: \"x\" });\n}\n",
        )
        .tokenize()
        .expect("lex");
        let syntax_trees = parse_syntax_trees(&tokens).expect("parse");
        assert!(matches!(
            project_build_entry_syntax(&syntax_trees),
            Err(BuildDeclarationError::MissingBuildDeclaration)
        ));
    }
}
