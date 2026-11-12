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
let math: DependencyRegistration = Build::dependency(builder, dependency::from_source(Source::Path { location: "math" }));

let mut host: DependencyDescription = dependency::from_source(Source::Path { location: "host" });
dependency::with_alias(&mut host, "host");
let host_registration: DependencyRegistration = Build::build_dependency(builder, host);
```

A registration returns a `DependencyRegistration`; its statement binds it, as
tooling writes, or discards it with `_ =`. An unaliased row is one
registration of a fresh description. An
aliased row is exactly three consecutive statements: a mutable description
local, its one alias configuration, and its registration. Every other use of
the dependency vocabulary (a conditional registration, a helper, a reused or
re-aliased description) is ordinary Build code the editor does not rewrite;
callers reject it as an unprojected shape rather than guess its meaning.
*/

use std::fmt;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{ExpressionHandle, ExpressionNode};
use syntax_trees::statement::{StatementHandle, StatementNode, TableCall};

use crate::BuildEntrySyntaxProjection;
use crate::build_declaration::BUILDER_PARAMETER_NAME;
use crate::direct_build_call::{DirectBuildCallError, direct_build_call_operands};

/// Owner of the ordinary dependency-description helpers.
pub const DEPENDENCY_HELPER_OWNER: &str = "dependency";
/// `dependency::from_source(source) -> DependencyDescription`.
pub const FROM_SOURCE_HELPER_NAME: &str = "from_source";
/// `dependency::with_alias(&mut description, alias)`.
pub const WITH_ALIAS_HELPER_NAME: &str = "with_alias";
/// The configured, unregistered description type.
pub const DEPENDENCY_DESCRIPTION_TYPE_NAME: &str = "DependencyDescription";

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

    /// The `Build` operation that registers a dependency for this purpose.
    pub const fn operation(self) -> DependencyOperation {
        match self {
            Self::Product => DependencyOperation::Dependency,
            Self::Build => DependencyOperation::BuildDependency,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DependencyOperation {
    Dependency,
    BuildDependency,
}

impl DependencyOperation {
    pub const ALL: [Self; 2] = [Self::Dependency, Self::BuildDependency];

    pub fn classify(target: &str) -> Option<Self> {
        Some(match target {
            "dependency" => Self::Dependency,
            "build_dependency" => Self::BuildDependency,
            _ => return None,
        })
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Dependency => "dependency",
            Self::BuildDependency => "build_dependency",
        }
    }

    pub const fn purpose(self) -> DependencyPurpose {
        match self {
            Self::Dependency => DependencyPurpose::Product,
            Self::BuildDependency => DependencyPurpose::Build,
        }
    }
}

/// Whether a call names dependency registration or description vocabulary:
/// a `Build::<operation>` registration or a `dependency::<helper>` call.
pub fn is_dependency_call(owner: Option<&str>, target: &str) -> bool {
    match owner {
        Some(crate::BUILD_TYPE_NAME) => DependencyOperation::classify(target).is_some(),
        Some(DEPENDENCY_HELPER_OWNER) => {
            matches!(target, FROM_SOURCE_HELPER_NAME | WITH_ALIAS_HELPER_NAME)
        }
        _ => false,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyRow {
    statements: Vec<StatementHandle>,
    operation: DependencyOperation,
    alias: Option<ExpressionHandle>,
    source: ExpressionHandle,
    calls: Vec<Vec<ExpressionHandle>>,
}

impl DependencyRow {
    /// The row's statements in authored order; the registration is last.
    pub fn statements(&self) -> &[StatementHandle] {
        &self.statements
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

    /// Argument rosters of every vocabulary call this row accepted, so a
    /// caller scanning all call expressions can tell projected occurrences
    /// from unprojected ones.
    pub fn accepted_call_arguments(&self) -> &[Vec<ExpressionHandle>] {
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
                "dependency registration must use Build::<operation>(builder, ...), with the root build machine's first parameter"
            }
            Self::WrongArguments => {
                "dependency registration takes the builder operand and one description, written `_ = Build::dependency(builder, dependency::from_source(source));` or, with an alias, a `let mut name: DependencyDescription = dependency::from_source(source);` local, one `dependency::with_alias(&mut name, \"alias\");` and `_ = Build::dependency(builder, name);`, with no generic, evidence or operational modifiers"
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
    let statements = syntax_trees.items.statements(entry.statements);
    let mut rows = Vec::new();
    let mut position = 0;
    while position < statements.len() {
        let handle = statements[position];
        if let Some(registration) = registration(syntax_trees, handle)? {
            let (source, from_source) = fresh_description(syntax_trees, registration.description)
                .ok_or(DependencyRowError::WrongArguments)?;
            rows.push(DependencyRow {
                statements: vec![handle],
                operation: registration.operation,
                alias: None,
                source,
                calls: vec![registration.arguments, from_source],
            });
        } else if let StatementNode::LocalData(local) = syntax_trees.statements.statement(handle)
            && let Some((source, from_source)) =
                fresh_description(syntax_trees, local.initial_value)
        {
            let row = aliased_row(
                syntax_trees,
                &statements[position..],
                local,
                source,
                from_source,
            )?;
            position += row.statements.len();
            rows.push(row);
            continue;
        }
        position += 1;
    }
    Ok(rows)
}

fn arguments(syntax_trees: &SyntaxTrees, call: &TableCall) -> Vec<ExpressionHandle> {
    syntax_trees
        .statements
        .expression_handles(call.arguments)
        .to_vec()
}

struct Registration {
    operation: DependencyOperation,
    description: ExpressionHandle,
    arguments: Vec<ExpressionHandle>,
}

// A registration returns its non-unit `DependencyRegistration`, so its direct
// statement either binds it (`let name: DependencyRegistration = ...;`, the
// spelling tooling writes) or discards it explicitly (`_ = ...;`).
fn registration(
    syntax_trees: &SyntaxTrees,
    statement: StatementHandle,
) -> Result<Option<Registration>, DependencyRowError> {
    match syntax_trees.statements.statement(statement) {
        StatementNode::Call(call) => {
            let owner = match syntax_trees
                .statements
                .identifier_path_members(call.receiver)
            {
                [owner] => Some(owner.as_str(syntax_trees)),
                _ => None,
            };
            if owner.is_some_and(|owner| owner != crate::BUILD_TYPE_NAME) {
                return Ok(None);
            }
            let Some(operation) = DependencyOperation::classify(call.target.as_str(syntax_trees))
            else {
                return Ok(None);
            };
            if !call.discards_result {
                return Err(DependencyRowError::WrongArguments);
            }
            let undiscarded = TableCall {
                discards_result: false,
                ..call.clone()
            };
            let operands =
                direct_build_call_operands(syntax_trees, &undiscarded).map_err(|failure| {
                    match failure {
                        DirectBuildCallError::WrongReceiver => DependencyRowError::WrongReceiver,
                        DirectBuildCallError::WrongArguments => DependencyRowError::WrongArguments,
                    }
                })?;
            let [description] = operands else {
                return Err(DependencyRowError::WrongArguments);
            };
            Ok(Some(Registration {
                operation,
                description: *description,
                arguments: arguments(syntax_trees, call),
            }))
        }
        StatementNode::LocalData(local) => {
            let ExpressionNode::Call(call) =
                syntax_trees.expressions.expression(local.initial_value)
            else {
                return Ok(None);
            };
            let Some(operation) = DependencyOperation::classify(call.target.as_str(syntax_trees))
            else {
                return Ok(None);
            };
            if expression_owner(syntax_trees, call.receiver) != Some(crate::BUILD_TYPE_NAME) {
                return Ok(None);
            }
            if !call.target_is_static
                || !call.machine_arguments.is_empty()
                || !call.evidence_arguments.is_empty()
                || call.operational_acknowledgement != Default::default()
            {
                return Err(DependencyRowError::WrongArguments);
            }
            let operands = syntax_trees.expressions.expression_handles(call.arguments);
            let [builder, description] = operands else {
                return Err(DependencyRowError::WrongArguments);
            };
            if single_name(syntax_trees, *builder) != Some(BUILDER_PARAMETER_NAME) {
                return Err(DependencyRowError::WrongReceiver);
            }
            Ok(Some(Registration {
                operation,
                description: *description,
                arguments: operands.to_vec(),
            }))
        }
        _ => Ok(None),
    }
}

// `dependency::from_source(source)` with exactly one source operand.
fn fresh_description(
    syntax_trees: &SyntaxTrees,
    expression: ExpressionHandle,
) -> Option<(ExpressionHandle, Vec<ExpressionHandle>)> {
    let ExpressionNode::Call(call) = syntax_trees.expressions.expression(expression) else {
        return None;
    };
    if call.target.as_str(syntax_trees) != FROM_SOURCE_HELPER_NAME
        || expression_owner(syntax_trees, call.receiver) != Some(DEPENDENCY_HELPER_OWNER)
        || !call.target_is_static
        || !call.machine_arguments.is_empty()
        || !call.evidence_arguments.is_empty()
        || call.operational_acknowledgement != Default::default()
    {
        return None;
    }
    match syntax_trees.expressions.expression_handles(call.arguments) {
        [source] => Some((*source, vec![*source])),
        _ => None,
    }
}

fn expression_owner(syntax_trees: &SyntaxTrees, receiver: ExpressionHandle) -> Option<&str> {
    if !receiver.is_valid() {
        return None;
    }
    let ExpressionNode::Name(path) = syntax_trees.expressions.expression(receiver) else {
        return None;
    };
    match syntax_trees.expressions.identifier_path_members(*path) {
        [owner] => Some(owner.as_str(syntax_trees)),
        _ => None,
    }
}

fn single_name(syntax_trees: &SyntaxTrees, expression: ExpressionHandle) -> Option<&str> {
    expression_owner(syntax_trees, expression)
}

fn aliased_row(
    syntax_trees: &SyntaxTrees,
    statements: &[StatementHandle],
    local: &syntax_trees::statement::TableLocalData,
    source: ExpressionHandle,
    from_source: Vec<ExpressionHandle>,
) -> Result<DependencyRow, DependencyRowError> {
    let name = local.name.as_str(syntax_trees);
    let [declaration, configuration, registration, ..] = statements else {
        return Err(DependencyRowError::WrongArguments);
    };
    if !local.is_mutable {
        return Err(DependencyRowError::WrongArguments);
    }
    let StatementNode::Call(configure) = syntax_trees.statements.statement(*configuration) else {
        return Err(DependencyRowError::WrongArguments);
    };
    if configure.target.as_str(syntax_trees) != WITH_ALIAS_HELPER_NAME
        || !matches!(syntax_trees.statements.identifier_path_members(configure.receiver), [owner] if owner.as_str(syntax_trees) == DEPENDENCY_HELPER_OWNER)
        || !configure.target_is_static
        || !configure.machine_arguments.is_empty()
        || !configure.evidence_arguments.is_empty()
        || configure.operational_acknowledgement != Default::default()
        || configure.discards_result
    {
        return Err(DependencyRowError::WrongArguments);
    }
    let [borrowed, alias] = syntax_trees
        .statements
        .expression_handles(configure.arguments)
    else {
        return Err(DependencyRowError::WrongArguments);
    };
    let ExpressionNode::Borrow(borrow) = syntax_trees.expressions.expression(*borrowed) else {
        return Err(DependencyRowError::WrongArguments);
    };
    if !borrow.access.is_exclusive() || single_name(syntax_trees, borrow.target) != Some(name) {
        return Err(DependencyRowError::WrongArguments);
    }
    let Some(register) = self::registration(syntax_trees, *registration)? else {
        return Err(DependencyRowError::WrongArguments);
    };
    if single_name(syntax_trees, register.description) != Some(name) {
        return Err(DependencyRowError::WrongArguments);
    }
    Ok(DependencyRow {
        statements: vec![*declaration, *configuration, *registration],
        operation: register.operation,
        alias: Some(*alias),
        source,
        calls: vec![
            from_source,
            arguments(syntax_trees, configure),
            register.arguments,
        ],
    })
}

#[cfg(test)]
mod tests {
    use super::{
        DependencyOperation, DependencyPurpose, DependencyRow, DependencyRowError,
        is_dependency_call, project_dependency_rows,
    };
    use crate::{BuildDeclarationError, project_build_entry_syntax};
    use syntax_trees::SyntaxTrees;
    use syntax_trees::expression::{ExpressionHandle, ExpressionNode};

    fn rows(source: &str) -> Result<(SyntaxTrees, Vec<DependencyRow>), DependencyRowError> {
        let syntax_trees = psi_compiler::parse_text(source)
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
    fn rows_retain_their_purpose_alias_and_literal_occurrences() {
        let (trees, rows) = rows(
            r#"machine build(builder: &mut Build) {
    Build::application(builder, "probe");
    _ = Build::dependency(builder, dependency::from_source(Source::Path { location: "math" }));
    let mut other: DependencyDescription = dependency::from_source(Source::Path { location: "other" });
    dependency::with_alias(&mut other, "other");
    let other_registration: DependencyRegistration = Build::dependency(builder, other);
    let git: DependencyRegistration = Build::build_dependency(builder, dependency::from_source(Source::Git { repository: "r", revision: "v" }));
    let mut host: DependencyDescription = dependency::from_source(Source::Path { location: "host" });
    dependency::with_alias(&mut host, "host");
    _ = Build::build_dependency(builder, host);
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
        assert_eq!(rows[1].operation(), DependencyOperation::Dependency);
        assert_eq!(rows[3].operation(), DependencyOperation::BuildDependency);
        assert_eq!(rows[0].alias(), None);
        assert_eq!(rows[0].statements().len(), 1);
        assert_eq!(rows[1].statements().len(), 3);
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
            "_ = Build::dependency(helper, dependency::from_source(Source::Path { location: \"x\" }));",
            "let r: DependencyRegistration = Build::dependency(helper, dependency::from_source(Source::Path { location: \"x\" }));",
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
            // The registration result is neither discarded nor bound.
            "Build::dependency(builder, dependency::from_source(Source::Path { location: \"x\" }))",
            "_ = Build::dependency(builder)",
            "_ = Build::build_dependency(builder, dependency::from_source(Source::Path { location: \"x\" }), 1)",
            "_ = Build::dependency(builder, Source::Path { location: \"x\" })",
            // An alias configuration must follow its description directly.
            "let mut x: DependencyDescription = dependency::from_source(Source::Path { location: \"x\" });\n    _ = Build::dependency(builder, x)",
            "let x: DependencyDescription = dependency::from_source(Source::Path { location: \"x\" });\n    dependency::with_alias(&mut x, \"x\");\n    _ = Build::dependency(builder, x)",
            "let mut x: DependencyDescription = dependency::from_source(Source::Path { location: \"x\" });\n    dependency::with_alias(&mut x, \"x\");\n    _ = Build::dependency(builder, y)",
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
    fn vocabulary_is_recognized_only_under_its_owner() {
        for operation in DependencyOperation::ALL {
            assert!(is_dependency_call(Some("Build"), operation.name()));
            assert!(!is_dependency_call(None, operation.name()));
            assert!(!is_dependency_call(Some("Graph"), operation.name()));
            assert_eq!(
                DependencyOperation::classify(operation.name()),
                Some(operation)
            );
        }
        assert!(is_dependency_call(Some("dependency"), "from_source"));
        assert!(is_dependency_call(Some("dependency"), "with_alias"));
        assert!(!is_dependency_call(Some("Build"), "from_source"));
        for retired in ["depend", "depend_as", "build_depend", "build_depend_as"] {
            assert_eq!(DependencyOperation::classify(retired), None, "{retired}");
        }
    }

    #[test]
    fn rows_require_the_canonical_build_entry() {
        let syntax_trees = psi_compiler::parse_text(
            "machine helper() {\n    _ = Build::dependency(builder, dependency::from_source(Source::Path { location: \"x\" }));\n}\n",
        )
        .expect("parse")
        .syntax;
        assert!(matches!(
            project_build_entry_syntax(&syntax_trees),
            Err(BuildDeclarationError::MissingBuildDeclaration)
        ));
    }
}
