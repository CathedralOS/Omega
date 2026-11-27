/*
Direct dependency rows of the canonical root `Build` entry.

Package tooling edits `build.omg` when it persists a dependency change, so it
must read and rewrite the literal registrations an author wrote. The
interpreted Build result, not this projection, decides the requested graph:
acquisition follows the `BuildRequest::Dependency` rows an activation actually
issues. This module only recognizes the canonical editable spellings of the
registration journey from
[scoped execution](wiki/spec/build/scoped_execution.md#dependency-declarations-and-binding):

```omega
let math: DependencyRegistration = Build::dependency(builder, Source::Path { location: "math" });

let mutable host: DependencyDescription = |dependency|::from_source(Source::Path { location: "host" });
|dependency|::with_alias(@mutable host, "host");
let host_registration: DependencyRegistration = Build::dependency(builder, host);
```

A registration returns a `DependencyRegistration`; its statement binds it, as
tooling writes, or discards it with `_ =`. An unaliased product registration
can pass its Source literal directly or wrap it in a fresh description. An
aliased row is exactly three consecutive statements: a mutable description
local, its one alias configuration, and its registration. Every other use of
the dependency vocabulary (a conditional registration, a helper, a reused or
re-aliased description) is ordinary Build code the editor does not rewrite;
callers reject it as an unprojected shape rather than guess its meaning.
*/

use std::fmt;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{ExpressionHandle, ExpressionNode};
use syntax_trees::statement::{StatementHandle, StatementNode};

use super::BuildEntrySyntaxProjection;
use super::direct_build_call::{DirectBuildCallError, direct_build_call_operands};
use super::{DEPENDENCY_REGISTRATION_NAME, expression_owner};

pub const FROM_SOURCE_HELPER_NAME: &str = "from_source";
/// `|dependency|::with_alias(@mutable description, alias)`.
pub const WITH_ALIAS_HELPER_NAME: &str = "with_alias";
/// The configured, unregistered description type.
pub const DEPENDENCY_DESCRIPTION_TYPE_NAME: &str = "DependencyDescription";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyRow {
    alias: Option<ExpressionHandle>,
    source: ExpressionHandle,
    calls: Vec<ExpressionHandle>,
}

impl DependencyRow {
    pub const fn alias(&self) -> Option<ExpressionHandle> {
        self.alias
    }

    pub const fn source(&self) -> ExpressionHandle {
        self.source
    }

    /// The exact authored calls accepted by this row.
    pub fn accepted_calls(&self) -> &[ExpressionHandle] {
        &self.calls
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
                "dependency registration must call Build::dependency with the root build machine's first parameter"
            }
            Self::WrongArguments => {
                "a directly editable product registration takes a literal Source, as in `_ = Build::dependency(builder, source);`, or a fresh dependency description or, with an alias, a `let mutable name: DependencyDescription = |dependency|::from_source(source);` local, one `|dependency|::with_alias(@mutable name, \"alias\");` and `_ = Build::dependency(builder, name);`, with no generic, evidence or operational modifiers"
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
    let receiver = build.receiver_name(syntax_trees);
    let statements = syntax_trees.items.statements(entry.statements);
    let mut rows = Vec::new();
    let mut position = 0;
    while position < statements.len() {
        let handle = statements[position];
        if let Some(registration) = registration(syntax_trees, handle, receiver)? {
            let mut calls = vec![registration.expression];
            let source = if let Some(source) = fresh_description(syntax_trees, registration.operand)
            {
                calls.push(registration.operand);
                source
            } else if matches!(
                syntax_trees.expressions.expression(registration.operand),
                ExpressionNode::StructLiteral(_)
            ) {
                registration.operand
            } else {
                return Err(DependencyRowError::WrongArguments);
            };
            rows.push(DependencyRow {
                alias: None,
                source,
                calls,
            });
        } else if let StatementNode::LocalData(local) = syntax_trees.statements.statement(handle)
            && let Some(source) = fresh_description(syntax_trees, local.initial_value)
        {
            let group = statements[position..]
                .first_chunk::<3>()
                .ok_or(DependencyRowError::WrongArguments)?;
            let row = aliased_row(syntax_trees, group, local, source, receiver)?;
            position += group.len();
            rows.push(row);
            continue;
        }
        position += 1;
    }
    Ok(rows)
}

struct Registration {
    operand: ExpressionHandle,
    expression: ExpressionHandle,
}

// A registration returns its non-unit `DependencyRegistration`, so its direct
// statement either binds it (`let name: DependencyRegistration = ...;`, the
// spelling tooling writes) or discards it explicitly (`_ = ...;`).
fn registration(
    syntax_trees: &SyntaxTrees,
    statement: StatementHandle,
    receiver: &str,
) -> Result<Option<Registration>, DependencyRowError> {
    let (expression, discarded) = match syntax_trees.statements.statement(statement) {
        StatementNode::Call(statement) => (statement.expression, Some(statement.discards_result)),
        StatementNode::LocalData(local) if local.initial_value.is_valid() => {
            (local.initial_value, None)
        }
        _ => return Ok(None),
    };
    let ExpressionNode::Call(call) = syntax_trees.expressions.expression(expression) else {
        return Ok(None);
    };
    if call.target.as_str(syntax_trees) != DEPENDENCY_REGISTRATION_NAME {
        return Ok(None);
    }
    let owner = expression_owner(syntax_trees, call.receiver);
    if owner != Some(package_compilation::declarations::BUILD_MODULE_NAME)
        && (owner.is_some() || discarded.is_none())
    {
        return Ok(None);
    }
    if discarded == Some(false) || (discarded.is_none() && !call.target_is_static) {
        return Err(DependencyRowError::WrongArguments);
    }
    let operands =
        direct_build_call_operands(syntax_trees, call, receiver).map_err(|error| match error {
            DirectBuildCallError::WrongReceiver => DependencyRowError::WrongReceiver,
            DirectBuildCallError::WrongArguments => DependencyRowError::WrongArguments,
        })?;
    let [operand] = operands else {
        return Err(DependencyRowError::WrongArguments);
    };
    Ok(Some(Registration {
        operand: *operand,
        expression,
    }))
}

// `|dependency|::from_source(source)` with exactly one source operand.
fn fresh_description(
    syntax_trees: &SyntaxTrees,
    expression: ExpressionHandle,
) -> Option<ExpressionHandle> {
    let ExpressionNode::Call(call) = syntax_trees.expressions.expression(expression) else {
        return None;
    };
    if super::dependency_import::helper(syntax_trees, call.target.as_str(syntax_trees))
        != Some(FROM_SOURCE_HELPER_NAME)
        || call.receiver.is_valid()
        || !call.target_is_static
        || !call.machine_arguments.is_empty()
        || !call.evidence_arguments.is_empty()
        || call.operational_acknowledgement != Default::default()
    {
        return None;
    }
    match syntax_trees.expressions.expression_handles(call.arguments) {
        [source] => Some(*source),
        _ => None,
    }
}

fn aliased_row(
    syntax_trees: &SyntaxTrees,
    statements: &[StatementHandle; 3],
    local: &syntax_trees::statement::TableLocalData,
    source: ExpressionHandle,
    receiver: &str,
) -> Result<DependencyRow, DependencyRowError> {
    let name = local.name.as_str(syntax_trees);
    let [_, configuration, registration] = statements;
    if !local.is_mutable {
        return Err(DependencyRowError::WrongArguments);
    }
    let StatementNode::Call(statement) = syntax_trees.statements.statement(*configuration) else {
        return Err(DependencyRowError::WrongArguments);
    };
    let configure = statement.call(syntax_trees);
    if super::dependency_import::helper(syntax_trees, configure.target.as_str(syntax_trees))
        != Some(WITH_ALIAS_HELPER_NAME)
        || configure.receiver.is_valid()
        || !configure.target_is_static
        || !configure.machine_arguments.is_empty()
        || !configure.evidence_arguments.is_empty()
        || configure.operational_acknowledgement != Default::default()
        || statement.discards_result
    {
        return Err(DependencyRowError::WrongArguments);
    }
    let [borrowed, alias] = syntax_trees
        .expressions
        .expression_handles(configure.arguments)
    else {
        return Err(DependencyRowError::WrongArguments);
    };
    let ExpressionNode::Borrow(borrow) = syntax_trees.expressions.expression(*borrowed) else {
        return Err(DependencyRowError::WrongArguments);
    };
    if !borrow.access.is_exclusive() || expression_owner(syntax_trees, borrow.target) != Some(name)
    {
        return Err(DependencyRowError::WrongArguments);
    }
    let Some(register) = self::registration(syntax_trees, *registration, receiver)? else {
        return Err(DependencyRowError::WrongArguments);
    };
    if expression_owner(syntax_trees, register.operand) != Some(name) {
        return Err(DependencyRowError::WrongArguments);
    }
    Ok(DependencyRow {
        alias: Some(*alias),
        source,
        calls: vec![
            local.initial_value,
            statement.expression,
            register.expression,
        ],
    })
}

#[cfg(test)]
mod tests {
    use super::super::project_build_entry_syntax;
    use super::{DependencyRow, DependencyRowError, project_dependency_rows};
    use crate::declarations::dependencies::read::DependencyProjectionError;
    use syntax_trees::SyntaxTrees;
    use syntax_trees::expression::{ExpressionHandle, ExpressionNode};

    fn rows(source: &str) -> Result<(SyntaxTrees, Vec<DependencyRow>), DependencyRowError> {
        let syntax_trees = source_syntax::parse_text(source)
            .unwrap_or_else(|error| panic!("parse build {source}: {error:?}"))
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
    fn rows_retain_their_alias_and_literal_occurrences() {
        let (trees, rows) = rows(
            r#"use {omega}/build/dependency.omg;
machine build(builder: @mutable BuildPlan) {
    builder.product = BuildProduct::Application { name: "probe" };
    _ = Build::dependency(builder, Source::Path { location: "math" });
    let mutable other: DependencyDescription = |dependency|::from_source(Source::Path { location: "other" });
    |dependency|::with_alias(@mutable other, "other");
    let other_registration: DependencyRegistration = Build::dependency(builder, other);
    let git: DependencyRegistration = Build::dependency(builder, |dependency|::from_source(Source::Git { repository: "r", revision: "v" }));
    let mutable host: DependencyDescription = |dependency|::from_source(Source::Path { location: "host" });
    |dependency|::with_alias(@mutable host, "host");
    _ = Build::dependency(builder, host);
}
"#,
        )
        .expect("project rows");

        assert_eq!(rows.len(), 4);
        assert_eq!(rows[0].alias(), None);
        assert_eq!(
            string_literal(&trees, rows[1].alias().expect("alias row")),
            "other"
        );
        assert_eq!(
            string_literal(&trees, rows[3].alias().expect("alias row")),
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
            "_ = Build::dependency(helper, |dependency|::from_source(Source::Path { location: \"x\" }));",
            "let r: DependencyRegistration = Build::dependency(helper, |dependency|::from_source(Source::Path { location: \"x\" }));",
        ];
        for statements in wrong_receiver {
            let source = format!(
                "use {{omega}}/build/dependency.omg;\nmachine build(builder: @mutable BuildPlan) {{\n{statements}\n}}\n"
            );
            assert_eq!(
                rows(&source).map(|pair| pair.1),
                Err(DependencyRowError::WrongReceiver),
                "{source}"
            );
        }
        let wrong_arguments = [
            // The registration result is neither discarded nor bound.
            "Build::dependency(builder, |dependency|::from_source(Source::Path { location: \"x\" }))",
            "_ = Build::dependency(builder)",
            "_ = Build::dependency(builder, |dependency|::from_source(Source::Path { location: \"x\" }), 1)",
            "_ = Build::dependency(builder, Source::Path { location: \"x\" }, ProgramLaunch::Inherit::<Packer>())",
            // An alias configuration must follow its description directly.
            "let mutable x: DependencyDescription = |dependency|::from_source(Source::Path { location: \"x\" });\n    _ = Build::dependency(builder, x)",
            "let x: DependencyDescription = |dependency|::from_source(Source::Path { location: \"x\" });\n    |dependency|::with_alias(@mutable x, \"x\");\n    _ = Build::dependency(builder, x)",
            "let mutable x: DependencyDescription = |dependency|::from_source(Source::Path { location: \"x\" });\n    |dependency|::with_alias(@mutable x, \"x\");\n    _ = Build::dependency(builder, y)",
        ];
        for statement in wrong_arguments {
            let source = format!(
                "use {{omega}}/build/dependency.omg;\nmachine build(builder: @mutable BuildPlan) {{\n    builder.product = BuildProduct::Application {{ name: \"probe\" }};\n    {statement};\n}}\n"
            );
            assert_eq!(
                rows(&source).map(|pair| pair.1),
                Err(DependencyRowError::WrongArguments),
                "{source}"
            );
        }
    }

    #[test]
    fn rows_require_the_canonical_build_entry() {
        let syntax_trees = source_syntax::parse_text(
            "machine helper() {\n    _ = Build::dependency(builder, |dependency|::from_source(Source::Path { location: \"x\" }));\n}\n",
        )
        .expect("parse")
        .syntax;
        assert!(matches!(
            project_build_entry_syntax(&syntax_trees),
            Err(DependencyProjectionError::MissingBuildMachine)
        ));
    }
}
