/*
Direct dependency rows of the canonical root `Build` entry.

We project dependency edges from direct unconditional `Build::<operation>(builder, ...)`
statements in `build.omg`. Both the package manager (when interpreting literal
source requests for acquisition or source-editing commands) and the compiler
(when binding imports against checked occurrences) share this row grammar so
neither consumer evaluates arbitrary build control flow or falls back across
the Product (`depend` / `depend_as`) and Build (`build_depend` /
`build_depend_as`) scopes.

Conditional `*_when` call names are recognized as dependency vocabulary by
`is_dependency_call_name` so callers can reject them explicitly as unprojected
syntax rather than admitting them as unconditional edges.
*/

use std::fmt;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::ExpressionHandle;
use syntax_trees::statement::{StatementHandle, StatementNode};

use crate::BuildEntrySyntaxProjection;
use crate::direct_build_call::{DirectBuildCallError, direct_build_call_operands};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
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

    pub const fn purpose(self) -> DependencyPurpose {
        match self {
            Self::Depend | Self::DependAs => DependencyPurpose::Product,
            Self::BuildDepend | Self::BuildDependAs => DependencyPurpose::Build,
        }
    }

    pub const fn takes_alias(self) -> bool {
        matches!(self, Self::DependAs | Self::BuildDependAs)
    }
}

pub(crate) const CONDITIONAL_DEPENDENCY_CALL_NAMES: [&str; 4] = [
    "depend_when",
    "depend_as_when",
    "build_depend_when",
    "build_depend_as_when",
];

pub fn is_dependency_call_name(target: &str) -> bool {
    DependencyOperation::classify(target).is_some()
        || CONDITIONAL_DEPENDENCY_CALL_NAMES.contains(&target)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyRow {
    statement: StatementHandle,
    operation: DependencyOperation,
    alias: Option<ExpressionHandle>,
    source: ExpressionHandle,
}

impl DependencyRow {
    pub const fn statement(&self) -> StatementHandle {
        self.statement
    }

    pub const fn operation(&self) -> DependencyOperation {
        self.operation
    }

    pub const fn purpose(&self) -> DependencyPurpose {
        self.operation.purpose()
    }

    pub const fn alias(&self) -> Option<ExpressionHandle> {
        self.alias
    }

    pub const fn source(&self) -> ExpressionHandle {
        self.source
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DependencyRowError {
    WrongReceiver,
    WrongArguments,
}

impl fmt::Display for DependencyRowError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::WrongReceiver => {
                "dependency request must use Build::<operation>(builder, ...), with the root build machine's first parameter"
            }
            Self::WrongArguments => {
                "dependency calls take the builder operand followed by one source argument, or one direct alias literal and one source argument, and accept no generic, evidence, operational, or discard modifiers"
            }
        })
    }
}

impl std::error::Error for DependencyRowError {}

pub fn project_dependency_rows(
    syntax_trees: &SyntaxTrees,
    build: &BuildEntrySyntaxProjection,
) -> Result<Vec<DependencyRow>, DependencyRowError> {
    let entry = syntax_trees.items.state(build.build_entry());
    let mut rows = Vec::new();
    for statement_handle in syntax_trees.items.statements(entry.statements) {
        let StatementNode::Call(call) = syntax_trees.statements.statement(*statement_handle) else {
            continue;
        };
        let Some(operation) = DependencyOperation::classify(call.target.as_str(syntax_trees))
        else {
            continue;
        };
        let operands =
            direct_build_call_operands(syntax_trees, call).map_err(|failure| match failure {
                DirectBuildCallError::WrongReceiver => DependencyRowError::WrongReceiver,
                DirectBuildCallError::WrongArguments => DependencyRowError::WrongArguments,
            })?;
        let (alias, source) = if operation.takes_alias() {
            let [alias, source] = operands else {
                return Err(DependencyRowError::WrongArguments);
            };
            (Some(*alias), *source)
        } else {
            let [source] = operands else {
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
    use syntax_trees::SyntaxTrees;
    use syntax_trees::expression::{ExpressionHandle, ExpressionNode};

    fn rows(source: &str) -> Result<(SyntaxTrees, Vec<DependencyRow>), DependencyRowError> {
        let syntax_trees = psi_compiler::parse_text(source)
            .expect("parse build")
            .syntax;
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
    Build::application(builder, "probe");
    Build::depend(builder, Source::Path { location: "math" });
    Build::depend_as(builder, "other", Source::Path { location: "other" });
    Build::build_depend(builder, Source::Git { repository: "r", revision: "v" });
    Build::build_depend_as(builder, "host", Source::Path { location: "host" });
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
            "Build::depend(helper, Source::Path { location: \"x\" });",
            "builder.depend(Source::Path { location: \"x\" });",
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
            "Build::depend()",
            "Build::depend_as(builder, \"only\")",
            "Build::build_depend(builder, Source::Path { location: \"x\" }, Source::Path { location: \"y\" })",
            "Build::depend_as(builder, \"x\", Source::Path { location: \"x\" }, 1)",
        ];
        for statement in wrong_arguments {
            let source = format!(
                "machine build(builder: &mut Build) {{\n    Build::application(builder, \"probe\");\n    {statement};\n}}\n"
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
    Build::application(builder, "probe");
    Build::depend_when(builder, flag(), Source::Path { location: "x" });
    Build::depend(builder, Source::Path { location: "math" });
}
"#,
        )
        .expect("conditional calls are not rows");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].operation(), DependencyOperation::Depend);
    }

    #[test]
    fn rows_require_the_canonical_build_entry() {
        let syntax_trees = psi_compiler::parse_text(
            "machine helper() {\n    builder.depend(Source::Path { location: \"x\" });\n}\n",
        )
        .expect("parse")
        .syntax;
        assert!(matches!(
            project_build_entry_syntax(&syntax_trees),
            Err(BuildDeclarationError::MissingBuildDeclaration)
        ));
    }
}
