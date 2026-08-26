use crate::identity::{PackageName, WorkspaceMemberPath};
use psi_source_files_to_tokens::Lexer;
use psi_syntax_trees::SyntaxTrees;
use psi_syntax_trees::expression::{ExpressionHandle, ExpressionNode};
use psi_syntax_trees::item::Item;
use psi_syntax_trees::statement::{StatementHandle, StatementNode};
use psi_syntax_trees::types::TypeReferenceNode;
use psi_tokens_to_syntax_trees::parse_syntax_trees;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

const BUILD_FILE_NAME: &str = "build.omg";
const BUILD_MACHINE_NAME: &str = "build";
const BUILD_TYPE_NAME: &str = "Build";
const BUILDER_PARAMETER_NAME: &str = "builder";
const PACKAGE_MACHINE_NAME: &str = "package";
const APPLICATION_MACHINE_NAME: &str = "application";
const MEMBER_MACHINE_NAME: &str = "member";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildDeclaration {
    Package(PackageDeclaration),
    Application(ApplicationDeclaration),
    Workspace(WorkspaceDeclaration),
}

impl BuildDeclaration {
    pub const fn kind(&self) -> BuildDeclarationKind {
        match self {
            Self::Package(_) => BuildDeclarationKind::Package,
            Self::Application(_) => BuildDeclarationKind::Application,
            Self::Workspace(_) => BuildDeclarationKind::Workspace,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildDeclarationKind {
    Package,
    Application,
    Workspace,
}

impl BuildDeclarationKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Package => "package",
            Self::Application => "application",
            Self::Workspace => "workspace",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageDeclaration {
    pub name: PackageName,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicationDeclaration {
    pub name: PackageName,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceDeclaration {
    pub members: Vec<WorkspaceMemberPath>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageDeclarationError {
    MissingBuildFile { path: PathBuf },
    ReadBuildFile { path: PathBuf, message: String },
    InvalidBuildFileEncoding { path: PathBuf },
    Lex { message: String },
    Parse { message: String },
    AuthoredToolchainVocabulary { name: String },
    ScopedBuildMachine { scope: String },
    DuplicateBuildMachines { count: usize },
    InvalidBuildMachine,
    MissingBuildEntry,
    InvalidBuildParameter,
    UnsupportedPackageShape,
    WrongPackageReceiver,
    WrongPackageArguments,
    UnsupportedApplicationShape,
    WrongApplicationReceiver,
    WrongApplicationArguments,
    UnsupportedMemberShape,
    WrongMemberReceiver,
    WrongMemberArguments,
    MissingBuildDeclaration,
    MissingPackageDeclaration,
    ExpectedPackageDeclaration { found: BuildDeclarationKind },
    DuplicatePackageDeclarations { count: usize },
    DuplicateApplicationDeclarations { count: usize },
    MixedBuildDeclarations,
    NameNotStringLiteral,
    NameNotUtf8,
    InvalidPackageName { message: String },
    MemberPathNotStringLiteral,
    MemberPathNotUtf8,
    InvalidWorkspaceMemberPath,
    DuplicateWorkspaceMember { path: String },
}

/// Declaration projection failures for every explicit `build.omg` project kind.
///
/// The package-specific name remains the concrete type for source compatibility
/// with the first declaration reader; new callers should use this general name.
pub type BuildDeclarationError = PackageDeclarationError;

impl fmt::Display for PackageDeclarationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingBuildFile { path } => {
                write!(formatter, "package build file is missing: {}", path.display())
            }
            Self::ReadBuildFile { path, message } => {
                write!(formatter, "cannot read {}: {message}", path.display())
            }
            Self::InvalidBuildFileEncoding { path } => {
                write!(formatter, "{} is not UTF-8 Omega source", path.display())
            }
            Self::Lex { message } => write!(formatter, "cannot lex package build: {message}"),
            Self::Parse { message } => write!(formatter, "cannot parse package build: {message}"),
            Self::AuthoredToolchainVocabulary { name } => write!(
                formatter,
                "package build must not declare toolchain package vocabulary `{name}`"
            ),
            Self::ScopedBuildMachine { scope } => {
                write!(formatter, "package build machine must be free, not `{scope}::build`")
            }
            Self::DuplicateBuildMachines { count } => {
                write!(formatter, "package build declares `build` {count} times")
            }
            Self::InvalidBuildMachine => formatter.write_str(
                "package build must be a bodyful, unscoped, nontarget, nongeneric ordinary machine",
            ),
            Self::MissingBuildEntry => {
                formatter.write_str("package build machine has no callable entry")
            }
            Self::InvalidBuildParameter => formatter.write_str(
                "package build machine's first parameter must be `builder: &mut Build`",
            ),
            Self::UnsupportedPackageShape => formatter.write_str(
                "package declaration must be one direct canonical `builder.package(\"kebab-name\")` statement in the root build entry",
            ),
            Self::WrongPackageReceiver => formatter.write_str(
                "package declaration receiver must be the root build machine's first parameter",
            ),
            Self::WrongPackageArguments => formatter.write_str(
                "`builder.package` must have one direct name literal and accepts no static, evidence, operational, or discard modifiers",
            ),
            Self::UnsupportedApplicationShape => formatter.write_str(
                "application declaration must be one direct canonical `builder.application(\"kebab-name\")` statement in the root build entry",
            ),
            Self::WrongApplicationReceiver => formatter.write_str(
                "application declaration receiver must be the root build machine's first parameter",
            ),
            Self::WrongApplicationArguments => formatter.write_str(
                "`builder.application` must have one direct name literal and accepts no static, evidence, operational, or discard modifiers",
            ),
            Self::UnsupportedMemberShape => formatter.write_str(
                "workspace members must be direct canonical `builder.member(\"relative/path\")` statements in the root build entry",
            ),
            Self::WrongMemberReceiver => formatter.write_str(
                "workspace member declaration receiver must be the root build machine's first parameter",
            ),
            Self::WrongMemberArguments => formatter.write_str(
                "`builder.member` must have one direct path literal and accepts no static, evidence, operational, or discard modifiers",
            ),
            Self::MissingBuildDeclaration => formatter.write_str(
                "build must declare exactly one kind through `builder.package`, `builder.application`, or one or more `builder.member` statements",
            ),
            Self::MissingPackageDeclaration => formatter.write_str(
                "package build must contain one direct `builder.package(\"kebab-name\")` declaration",
            ),
            Self::ExpectedPackageDeclaration { found } => write!(
                formatter,
                "expected a package build declaration, found an explicit {} declaration",
                found.as_str()
            ),
            Self::DuplicatePackageDeclarations { count } => {
                write!(formatter, "package build declares its package name {count} times")
            }
            Self::DuplicateApplicationDeclarations { count } => write!(
                formatter,
                "application build declares its application name {count} times"
            ),
            Self::MixedBuildDeclarations => formatter.write_str(
                "build must declare exactly one kind; package, application, and workspace member declarations cannot be mixed",
            ),
            Self::NameNotStringLiteral => {
                formatter.write_str("package or application name must be a direct string literal")
            }
            Self::NameNotUtf8 => {
                formatter.write_str("package or application name must contain UTF-8 bytes")
            }
            Self::InvalidPackageName { message } => formatter.write_str(message),
            Self::MemberPathNotStringLiteral => {
                formatter.write_str("workspace member path must be a direct string literal")
            }
            Self::MemberPathNotUtf8 => {
                formatter.write_str("workspace member path must contain UTF-8 bytes")
            }
            Self::InvalidWorkspaceMemberPath => formatter.write_str(
                "workspace member path must be a canonical portable relative path without `.` or `..` components",
            ),
            Self::DuplicateWorkspaceMember { path } => {
                write!(formatter, "workspace member path `{path}` is declared more than once")
            }
        }
    }
}

impl std::error::Error for PackageDeclarationError {}

/// Project the package-authored human name from the immutable package root.
///
/// This parses only the root `build.omg`; it does not evaluate build code,
/// imports, constants, helpers, control flow, dependencies, generated files,
/// or build-host services.
pub fn extract_package_declaration(
    package_root: impl AsRef<Path>,
) -> Result<PackageDeclaration, PackageDeclarationError> {
    match extract_build_declaration(package_root) {
        Ok(BuildDeclaration::Package(declaration)) => Ok(declaration),
        Ok(other) => Err(PackageDeclarationError::ExpectedPackageDeclaration {
            found: other.kind(),
        }),
        Err(PackageDeclarationError::MissingBuildDeclaration) => {
            Err(PackageDeclarationError::MissingPackageDeclaration)
        }
        Err(error) => Err(error),
    }
}

/// Project the explicit package, application, or workspace kind from one
/// immutable root `build.omg` without executing build code.
pub fn extract_build_declaration(
    root: impl AsRef<Path>,
) -> Result<BuildDeclaration, BuildDeclarationError> {
    let build_path = root.as_ref().join(BUILD_FILE_NAME);
    let source_bytes = match fs::read(&build_path) {
        Ok(source) => source,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(PackageDeclarationError::MissingBuildFile { path: build_path });
        }
        Err(error) => {
            return Err(PackageDeclarationError::ReadBuildFile {
                path: build_path,
                message: error.to_string(),
            });
        }
    };
    let source = std::str::from_utf8(&source_bytes).map_err(|_| {
        PackageDeclarationError::InvalidBuildFileEncoding {
            path: build_path.clone(),
        }
    })?;
    extract_build_from_source(source)
}

fn extract_build_from_source(source: &str) -> Result<BuildDeclaration, PackageDeclarationError> {
    let tokens = Lexer::new(source)
        .tokenize()
        .map_err(|error| PackageDeclarationError::Lex {
            message: error.message,
        })?;
    let syntax_trees =
        parse_syntax_trees(&tokens).map_err(|error| PackageDeclarationError::Parse {
            message: error.message,
        })?;
    extract_build_from_syntax_trees(&syntax_trees)
}

pub(crate) fn extract_build_from_syntax_trees(
    syntax_trees: &SyntaxTrees,
) -> Result<BuildDeclaration, PackageDeclarationError> {
    reject_authored_toolchain_vocabulary(syntax_trees)?;

    let named_builds = syntax_trees
        .root_items()
        .filter_map(|item| match item {
            Item::Machine(machine)
                if machine_leaf_name(machine.name.as_str()) == BUILD_MACHINE_NAME =>
            {
                Some(machine)
            }
            _ => None,
        })
        .collect::<Vec<_>>();

    if let Some(scoped) = named_builds
        .iter()
        .find(|machine| machine.attached_data.is_some())
    {
        return Err(PackageDeclarationError::ScopedBuildMachine {
            scope: scoped
                .attached_data
                .as_ref()
                .expect("scoped build has an owner")
                .as_str()
                .to_owned(),
        });
    }
    if named_builds.len() > 1 {
        return Err(PackageDeclarationError::DuplicateBuildMachines {
            count: named_builds.len(),
        });
    }

    let Some(build) = named_builds.first() else {
        reject_unprojected_build_declaration_syntax(syntax_trees, &[], &[])?;
        return Err(PackageDeclarationError::MissingBuildDeclaration);
    };
    if build.bodyless
        || build.boundary
        || build.target.is_some()
        || !build.lifetime_parameters.is_empty()
        || !build.type_parameters.is_empty()
    {
        return Err(PackageDeclarationError::InvalidBuildMachine);
    }

    let entry_handle = *syntax_trees
        .items
        .state_handles(build.states)
        .first()
        .ok_or(PackageDeclarationError::MissingBuildEntry)?;
    let entry = syntax_trees.items.state(entry_handle);
    let builder_parameter = syntax_trees
        .items
        .state_parameters(entry.parameters)
        .first()
        .map(|handle| syntax_trees.items.state_parameter(*handle))
        .ok_or(PackageDeclarationError::InvalidBuildParameter)?;
    if builder_parameter.name.as_str() != BUILDER_PARAMETER_NAME
        || builder_parameter.is_const
        || builder_parameter.is_self
    {
        return Err(PackageDeclarationError::InvalidBuildParameter);
    }
    let TypeReferenceNode::Reference {
        referee,
        access,
        lifetime,
    } = syntax_trees
        .type_references
        .type_reference(builder_parameter.type_reference)
    else {
        return Err(PackageDeclarationError::InvalidBuildParameter);
    };
    if lifetime.is_some() || !access.is_exclusive() || !access.is_readable() {
        return Err(PackageDeclarationError::InvalidBuildParameter);
    }
    if !matches!(
        syntax_trees.type_references.type_reference(*referee),
        TypeReferenceNode::Named(name) if name.as_str() == BUILD_TYPE_NAME
    ) {
        return Err(PackageDeclarationError::InvalidBuildParameter);
    }

    let builder_name = builder_parameter.name.as_str();
    let mut packages = Vec::new();
    let mut applications = Vec::new();
    let mut members = Vec::new();
    let mut accepted_statements = Vec::new();
    let mut accepted_literals = Vec::new();
    for statement_handle in syntax_trees.items.statements(entry.statements) {
        let StatementNode::Call(call) = syntax_trees.statements.statement(*statement_handle) else {
            continue;
        };
        let operation = call.target.as_str();
        if !matches!(
            operation,
            PACKAGE_MACHINE_NAME | APPLICATION_MACHINE_NAME | MEMBER_MACHINE_NAME
        ) {
            continue;
        }
        if call.receiver_starts_at_self
            || !matches!(
                syntax_trees.statements.identifier_path_members(call.receiver),
                [receiver] if receiver.as_str() == builder_name
            )
        {
            return Err(wrong_receiver_error(operation));
        }
        if !call.machine_arguments.is_empty()
            || !call.evidence_arguments.is_empty()
            || call.operational_acknowledgement != Default::default()
            || call.discards_result
        {
            return Err(wrong_arguments_error(operation));
        }
        let [literal_handle] = syntax_trees.statements.expression_handles(call.arguments) else {
            return Err(wrong_arguments_error(operation));
        };
        match operation {
            PACKAGE_MACHINE_NAME => packages.push(PackageDeclaration {
                name: project_name_literal(syntax_trees, *literal_handle)?,
            }),
            APPLICATION_MACHINE_NAME => applications.push(ApplicationDeclaration {
                name: project_name_literal(syntax_trees, *literal_handle)?,
            }),
            MEMBER_MACHINE_NAME => {
                members.push(project_member_path_literal(syntax_trees, *literal_handle)?)
            }
            _ => unreachable!("declaration operation was filtered above"),
        }
        accepted_statements.push(*statement_handle);
        accepted_literals.push(*literal_handle);
    }

    if packages.len() > 1 {
        return Err(PackageDeclarationError::DuplicatePackageDeclarations {
            count: packages.len(),
        });
    }
    if applications.len() > 1 {
        return Err(PackageDeclarationError::DuplicateApplicationDeclarations {
            count: applications.len(),
        });
    }
    reject_unprojected_build_declaration_syntax(
        syntax_trees,
        &accepted_statements,
        &accepted_literals,
    )?;
    let declared_kinds = usize::from(!packages.is_empty())
        + usize::from(!applications.is_empty())
        + usize::from(!members.is_empty());
    if declared_kinds > 1 {
        return Err(PackageDeclarationError::MixedBuildDeclarations);
    }
    if let Some(package) = packages.pop() {
        return Ok(BuildDeclaration::Package(package));
    }
    if let Some(application) = applications.pop() {
        return Ok(BuildDeclaration::Application(application));
    }
    for (index, member) in members.iter().enumerate() {
        if members[..index].contains(member) {
            return Err(PackageDeclarationError::DuplicateWorkspaceMember {
                path: member.as_str().to_owned(),
            });
        }
    }
    if !members.is_empty() {
        return Ok(BuildDeclaration::Workspace(WorkspaceDeclaration {
            members,
        }));
    }
    Err(PackageDeclarationError::MissingBuildDeclaration)
}

fn wrong_receiver_error(operation: &str) -> PackageDeclarationError {
    match operation {
        PACKAGE_MACHINE_NAME => PackageDeclarationError::WrongPackageReceiver,
        APPLICATION_MACHINE_NAME => PackageDeclarationError::WrongApplicationReceiver,
        MEMBER_MACHINE_NAME => PackageDeclarationError::WrongMemberReceiver,
        _ => unreachable!("only declaration operations request receiver errors"),
    }
}

fn wrong_arguments_error(operation: &str) -> PackageDeclarationError {
    match operation {
        PACKAGE_MACHINE_NAME => PackageDeclarationError::WrongPackageArguments,
        APPLICATION_MACHINE_NAME => PackageDeclarationError::WrongApplicationArguments,
        MEMBER_MACHINE_NAME => PackageDeclarationError::WrongMemberArguments,
        _ => unreachable!("only declaration operations request argument errors"),
    }
}

fn reject_authored_toolchain_vocabulary(
    syntax_trees: &SyntaxTrees,
) -> Result<(), PackageDeclarationError> {
    for item in syntax_trees.root_items() {
        match package_authored_type_name(item) {
            Some(name) if machine_leaf_name(name) == BUILD_TYPE_NAME => {
                return Err(PackageDeclarationError::AuthoredToolchainVocabulary {
                    name: BUILD_TYPE_NAME.to_owned(),
                });
            }
            _ => {}
        }
        match item {
            Item::Machine(machine)
                if machine
                    .attached_data
                    .as_ref()
                    .is_some_and(|owner| owner.as_str() == BUILD_TYPE_NAME)
                    && matches!(
                        machine_leaf_name(machine.name.as_str()),
                        PACKAGE_MACHINE_NAME | APPLICATION_MACHINE_NAME | MEMBER_MACHINE_NAME
                    ) =>
            {
                return Err(PackageDeclarationError::AuthoredToolchainVocabulary {
                    name: machine.name.as_str().to_owned(),
                });
            }
            _ => {}
        }
    }
    Ok(())
}

fn package_authored_type_name(item: &Item) -> Option<&str> {
    match item {
        Item::Data(data) => Some(data.name.as_str()),
        Item::Domain(domain) => Some(domain.name.as_str()),
        Item::Trait(definition) => Some(definition.name.as_str()),
        Item::WireData(wire) => Some(wire.name.as_str()),
        _ => None,
    }
}

fn machine_leaf_name(name: &str) -> &str {
    name.rsplit("::").next().unwrap_or(name)
}

fn project_name_literal(
    syntax_trees: &SyntaxTrees,
    name_handle: ExpressionHandle,
) -> Result<PackageName, PackageDeclarationError> {
    let ExpressionNode::String(name_bytes) = syntax_trees.expressions.expression(name_handle)
    else {
        return Err(PackageDeclarationError::NameNotStringLiteral);
    };
    let name = std::str::from_utf8(name_bytes).map_err(|_| PackageDeclarationError::NameNotUtf8)?;
    PackageName::parse(name)
        .map_err(|message| PackageDeclarationError::InvalidPackageName { message })
}

fn project_member_path_literal(
    syntax_trees: &SyntaxTrees,
    path_handle: ExpressionHandle,
) -> Result<WorkspaceMemberPath, PackageDeclarationError> {
    let ExpressionNode::String(path_bytes) = syntax_trees.expressions.expression(path_handle)
    else {
        return Err(PackageDeclarationError::MemberPathNotStringLiteral);
    };
    let path =
        std::str::from_utf8(path_bytes).map_err(|_| PackageDeclarationError::MemberPathNotUtf8)?;
    WorkspaceMemberPath::parse(path)
        .map_err(|_| PackageDeclarationError::InvalidWorkspaceMemberPath)
}

fn reject_unprojected_build_declaration_syntax(
    syntax_trees: &SyntaxTrees,
    accepted_statements: &[StatementHandle],
    accepted_literals: &[ExpressionHandle],
) -> Result<(), PackageDeclarationError> {
    for item in syntax_trees.root_items() {
        let Item::Machine(machine) = item else {
            continue;
        };
        for state_handle in syntax_trees.items.state_handles(machine.states) {
            let state = syntax_trees.items.state(*state_handle);
            for statement_handle in syntax_trees.items.statements(state.statements) {
                let StatementNode::Call(call) =
                    syntax_trees.statements.statement(*statement_handle)
                else {
                    continue;
                };
                if matches!(
                    call.target.as_str(),
                    PACKAGE_MACHINE_NAME | APPLICATION_MACHINE_NAME | MEMBER_MACHINE_NAME
                ) && !accepted_statements.contains(statement_handle)
                {
                    return Err(unsupported_shape_error(call.target.as_str()));
                }
            }
        }
    }

    for (_, expression) in syntax_trees.expressions.iter_expressions() {
        if let ExpressionNode::Call(call) = expression
            && matches!(
                call.target.as_str(),
                PACKAGE_MACHINE_NAME | APPLICATION_MACHINE_NAME | MEMBER_MACHINE_NAME
            )
        {
            let arguments = syntax_trees.expressions.expression_handles(call.arguments);
            if !matches!(arguments, [literal] if accepted_literals.contains(literal)) {
                return Err(unsupported_shape_error(call.target.as_str()));
            }
        }
    }
    Ok(())
}

fn unsupported_shape_error(operation: &str) -> PackageDeclarationError {
    match operation {
        PACKAGE_MACHINE_NAME => PackageDeclarationError::UnsupportedPackageShape,
        APPLICATION_MACHINE_NAME => PackageDeclarationError::UnsupportedApplicationShape,
        MEMBER_MACHINE_NAME => PackageDeclarationError::UnsupportedMemberShape,
        _ => unreachable!("only declaration operations request shape errors"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

    struct PackageFixture {
        root: PathBuf,
    }

    impl PackageFixture {
        fn empty() -> Self {
            let nonce = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
            let root = std::env::temp_dir().join(format!(
                "omega-package-declaration-{}-{nonce}",
                std::process::id()
            ));
            fs::create_dir(&root).expect("create package fixture");
            Self { root }
        }

        fn with_source(source: &str) -> Self {
            let fixture = Self::empty();
            fs::write(fixture.root.join(BUILD_FILE_NAME), source).expect("write build.omg");
            fixture
        }

        fn extract(&self) -> Result<PackageDeclaration, PackageDeclarationError> {
            extract_package_declaration(&self.root)
        }

        fn extract_build(&self) -> Result<BuildDeclaration, PackageDeclarationError> {
            extract_build_declaration(&self.root)
        }
    }

    impl Drop for PackageFixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn declaration(name: &str) -> String {
        format!(
            r#"
            machine build(builder: &mut Build) {{
                builder.package("{name}");
            }}
            "#
        )
    }

    #[test]
    fn projects_one_direct_canonical_package_declaration() {
        let fixture = PackageFixture::with_source(&declaration("arithmetic-kernels"));
        assert_eq!(
            fixture.extract().unwrap(),
            PackageDeclaration {
                name: PackageName::parse("arithmetic-kernels").unwrap(),
            }
        );
        assert!(matches!(
            fixture.extract_build().unwrap(),
            BuildDeclaration::Package(_)
        ));
    }

    #[test]
    fn projects_explicit_application_and_workspace_kinds() {
        let application = PackageFixture::with_source(
            r#"machine build(builder: &mut Build) { builder.application("omega-compiler"); }"#,
        );
        assert_eq!(
            application.extract_build().unwrap(),
            BuildDeclaration::Application(ApplicationDeclaration {
                name: PackageName::parse("omega-compiler").unwrap(),
            })
        );
        assert_eq!(
            application.extract().unwrap_err(),
            PackageDeclarationError::ExpectedPackageDeclaration {
                found: BuildDeclarationKind::Application,
            }
        );

        let workspace = PackageFixture::with_source(
            r#"
            machine build(builder: &mut Build) {
                builder.member("omega/language/std");
                builder.member("apps/omega-compiler");
            }
            "#,
        );
        assert_eq!(
            workspace.extract_build().unwrap(),
            BuildDeclaration::Workspace(WorkspaceDeclaration {
                members: vec![
                    WorkspaceMemberPath::parse("omega/language/std").unwrap(),
                    WorkspaceMemberPath::parse("apps/omega-compiler").unwrap(),
                ],
            })
        );
    }

    #[test]
    fn rejects_missing_mixed_and_duplicate_explicit_kinds() {
        let missing = PackageFixture::with_source("machine build(builder: &mut Build) {}");
        assert_eq!(
            missing.extract_build().unwrap_err(),
            PackageDeclarationError::MissingBuildDeclaration
        );

        for source in [
            r#"machine build(builder: &mut Build) { builder.package("one-package"); builder.application("one-app"); }"#,
            r#"machine build(builder: &mut Build) { builder.application("one-app"); builder.member("apps/one"); }"#,
            r#"machine build(builder: &mut Build) { builder.package("one-package"); builder.member("packages/one"); }"#,
        ] {
            let fixture = PackageFixture::with_source(source);
            assert_eq!(
                fixture.extract_build().unwrap_err(),
                PackageDeclarationError::MixedBuildDeclarations
            );
        }

        let duplicate_application = PackageFixture::with_source(
            r#"machine build(builder: &mut Build) { builder.application("one-app"); builder.application("two-app"); }"#,
        );
        assert_eq!(
            duplicate_application.extract_build().unwrap_err(),
            PackageDeclarationError::DuplicateApplicationDeclarations { count: 2 }
        );

        let duplicate_member = PackageFixture::with_source(
            r#"machine build(builder: &mut Build) { builder.member("packages/one"); builder.member("packages/one"); }"#,
        );
        assert_eq!(
            duplicate_member.extract_build().unwrap_err(),
            PackageDeclarationError::DuplicateWorkspaceMember {
                path: "packages/one".to_owned(),
            }
        );
    }

    #[test]
    fn rejects_noncanonical_workspace_members_and_hidden_kind_calls() {
        for path in ["", "/absolute", "packages/../escape", "packages//double"] {
            let fixture = PackageFixture::with_source(&format!(
                r#"machine build(builder: &mut Build) {{ builder.member("{path}"); }}"#
            ));
            assert_eq!(
                fixture.extract_build().unwrap_err(),
                PackageDeclarationError::InvalidWorkspaceMemberPath
            );
        }

        for (source, expected) in [
            (
                r#"machine build(builder: &mut Build) { other.member("packages/one"); }"#,
                PackageDeclarationError::WrongMemberReceiver,
            ),
            (
                r#"machine build(builder: &mut Build) { builder.application(); }"#,
                PackageDeclarationError::WrongApplicationArguments,
            ),
            (
                r#"machine helper(builder: &mut Build) { builder.member("packages/one"); } machine build(builder: &mut Build) {}"#,
                PackageDeclarationError::UnsupportedMemberShape,
            ),
        ] {
            let fixture = PackageFixture::with_source(source);
            assert_eq!(fixture.extract_build().unwrap_err(), expected);
        }
    }

    #[test]
    fn permits_other_build_parameters_and_direct_build_operations() {
        let fixture = PackageFixture::with_source(
            r#"
            machine build(builder: &mut Build, filesystem: &mut Filesystem) {
                builder.package("arithmetic-kernels");
                builder.depend(Source::Path { location: "../exact" });
            }
            "#,
        );
        assert_eq!(
            fixture.extract().unwrap().name,
            PackageName::parse("arithmetic-kernels").unwrap()
        );
    }

    #[test]
    fn rejects_missing_unreadable_non_utf8_unlexable_and_unparsable_files() {
        let missing = PackageFixture::empty();
        assert!(matches!(
            missing.extract(),
            Err(PackageDeclarationError::MissingBuildFile { .. })
        ));

        let unreadable = PackageFixture::empty();
        fs::create_dir(unreadable.root.join(BUILD_FILE_NAME)).expect("create build directory");
        assert!(matches!(
            unreadable.extract(),
            Err(PackageDeclarationError::ReadBuildFile { .. })
        ));

        let invalid_encoding = PackageFixture::empty();
        fs::write(invalid_encoding.root.join(BUILD_FILE_NAME), [0xff]).expect("write bad UTF-8");
        assert!(matches!(
            invalid_encoding.extract(),
            Err(PackageDeclarationError::InvalidBuildFileEncoding { .. })
        ));

        let unlexable = PackageFixture::with_source("machine build(builder: &mut Build) { ` }");
        assert!(matches!(
            unlexable.extract(),
            Err(PackageDeclarationError::Lex { .. })
        ));
        let unparsable = PackageFixture::with_source("machine build(builder: &mut Build) {");
        assert!(matches!(
            unparsable.extract(),
            Err(PackageDeclarationError::Parse { .. })
        ));
    }

    #[test]
    fn rejects_missing_and_duplicate_package_calls() {
        for source in [
            "machine build(builder: &mut Build) {}",
            r#"const PACKAGE: Package = Package { name: "retired-shape" }; machine build(builder: &mut Build) {}"#,
            r#"machine helper(builder: &mut Build) {}"#,
        ] {
            let fixture = PackageFixture::with_source(source);
            assert!(matches!(
                fixture.extract(),
                Err(PackageDeclarationError::MissingPackageDeclaration)
            ));
        }

        let duplicate = PackageFixture::with_source(
            r#"
            machine build(builder: &mut Build) {
                builder.package("first-package");
                builder.package("second-package");
            }
            "#,
        );
        assert_eq!(
            duplicate.extract().unwrap_err(),
            PackageDeclarationError::DuplicatePackageDeclarations { count: 2 }
        );
    }

    #[test]
    fn rejects_duplicate_and_scoped_build_machines() {
        let duplicate = PackageFixture::with_source(
            r#"
            machine build(builder: &mut Build) { builder.package("first-package"); }
            machine build(builder: &mut Build) { builder.package("second-package"); }
            "#,
        );
        assert_eq!(
            duplicate.extract().unwrap_err(),
            PackageDeclarationError::DuplicateBuildMachines { count: 2 }
        );

        let scoped = PackageFixture::with_source(
            r#"machine Owner::build(builder: &mut Build) { builder.package("scoped-package"); }"#,
        );
        assert!(matches!(
            scoped.extract(),
            Err(PackageDeclarationError::ScopedBuildMachine { .. })
        ));
    }

    #[test]
    fn rejects_nonordinary_build_machine_forms() {
        for source in [
            "boundary machine build(builder: &mut Build);",
            "machine build<T>(builder: &mut Build) {}",
        ] {
            let fixture = PackageFixture::with_source(source);
            assert!(matches!(
                fixture.extract(),
                Err(PackageDeclarationError::InvalidBuildMachine)
            ));
        }
    }

    #[test]
    fn rejects_noncanonical_first_build_parameter() {
        for source in [
            "machine build() {}",
            "machine build(builder: Build) {}",
            "machine build(builder: &Build) {}",
            "machine build(builder: &write Build) {}",
            "machine build(builder: &mut Builder) {}",
            "machine build(build: &mut Build) {}",
        ] {
            let fixture = PackageFixture::with_source(source);
            assert!(matches!(
                fixture.extract(),
                Err(PackageDeclarationError::InvalidBuildParameter)
            ));
        }
    }

    #[test]
    fn rejects_authored_build_and_package_vocabulary() {
        for source in [
            r#"data Build {} machine build(builder: &mut Build) { builder.package("spoofed-package"); }"#,
            r#"domain u64::Build; machine build(builder: &mut Build) { builder.package("spoofed-package"); }"#,
            r#"trait Build {} machine build(builder: &mut Build) { builder.package("spoofed-package"); }"#,
            r#"machine Build::package(name: &[u8]) {} machine build(builder: &mut Build) { builder.package("spoofed-package"); }"#,
            r#"machine Build::application(name: &[u8]) {} machine build(builder: &mut Build) { builder.application("spoofed-app"); }"#,
            r#"machine Build::member(path: &[u8]) {} machine build(builder: &mut Build) { builder.member("spoofed/member"); }"#,
        ] {
            let fixture = PackageFixture::with_source(source);
            let result = fixture.extract();
            assert!(
                matches!(
                    result,
                    Err(PackageDeclarationError::AuthoredToolchainVocabulary { .. })
                ),
                "unexpected result for {source:?}: {result:?}"
            );
        }
    }

    #[test]
    fn rejects_wrong_package_receiver_and_arguments() {
        let wrong_receiver = PackageFixture::with_source(
            r#"machine build(builder: &mut Build) { other.package("wrong-receiver"); }"#,
        );
        assert_eq!(
            wrong_receiver.extract().unwrap_err(),
            PackageDeclarationError::WrongPackageReceiver
        );

        for source in [
            r#"machine build(builder: &mut Build) { builder.package(); }"#,
            r#"machine build(builder: &mut Build) { builder.package("one", "two"); }"#,
        ] {
            let fixture = PackageFixture::with_source(source);
            assert_eq!(
                fixture.extract().unwrap_err(),
                PackageDeclarationError::WrongPackageArguments
            );
        }
    }

    #[test]
    fn rejects_helper_nested_and_expression_package_calls() {
        let helper = PackageFixture::with_source(
            r#"
            machine declare(builder: &mut Build) {
                builder.package("hidden-package");
            }
            machine build(builder: &mut Build) { declare(builder); }
            "#,
        );
        assert_eq!(
            helper.extract().unwrap_err(),
            PackageDeclarationError::UnsupportedPackageShape
        );

        let nested_state = PackageFixture::with_source(
            r#"
            machine build(builder: &mut Build) {
                state later(builder: &mut Build) {
                    builder.package("conditional-package");
                }
            }
            "#,
        );
        assert_eq!(
            nested_state.extract().unwrap_err(),
            PackageDeclarationError::UnsupportedPackageShape
        );

        let expression = PackageFixture::with_source(
            r#"
            machine build(builder: &mut Build) {
                consume(builder.package("expression-package"));
            }
            "#,
        );
        assert_eq!(
            expression.extract().unwrap_err(),
            PackageDeclarationError::UnsupportedPackageShape
        );
    }

    #[test]
    fn rejects_package_syntax_without_an_authoritative_build_machine() {
        let fixture = PackageFixture::with_source(
            r#"
            machine helper(builder: &mut Build) {
                builder.package("hidden-package");
            }
            "#,
        );
        assert_eq!(
            fixture.extract().unwrap_err(),
            PackageDeclarationError::UnsupportedPackageShape
        );
    }

    #[test]
    fn rejects_nonliteral_non_utf8_and_noncanonical_names() {
        let nonliteral = PackageFixture::with_source(
            "machine build(builder: &mut Build) { builder.package(package_name); }",
        );
        assert_eq!(
            nonliteral.extract().unwrap_err(),
            PackageDeclarationError::NameNotStringLiteral
        );

        let non_utf8 = PackageFixture::with_source(
            r#"machine build(builder: &mut Build) { builder.package("\x80package"); }"#,
        );
        assert_eq!(
            non_utf8.extract().unwrap_err(),
            PackageDeclarationError::NameNotUtf8
        );

        for name in [
            "Arithmetic-Kernels",
            "arithmetic_kernels",
            "arithmetic--kernels",
            "123-tools",
        ] {
            let fixture = PackageFixture::with_source(&declaration(name));
            assert!(matches!(
                fixture.extract(),
                Err(PackageDeclarationError::InvalidPackageName { .. })
            ));
        }
    }

    #[test]
    fn package_operation_name_is_case_sensitive() {
        let fixture = PackageFixture::with_source(
            r#"machine build(builder: &mut Build) { builder.Package("wrong-case"); }"#,
        );
        assert_eq!(
            fixture.extract().unwrap_err(),
            PackageDeclarationError::MissingPackageDeclaration
        );
    }
}
