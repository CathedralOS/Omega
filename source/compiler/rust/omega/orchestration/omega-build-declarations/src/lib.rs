#![forbid(unsafe_code)]

//! Compiler-neutral syntactic projection of the authoritative project role in
//! an Omega `build.omg`.
//!
//! Projection parses syntax but never evaluates build code, follows imports,
//! or grants build-host authority. Package management and the compiler can
//! therefore consume one role grammar without depending on each other.

use psi_source_files_to_tokens::Lexer;
use psi_syntax_trees::SyntaxTrees;
use psi_syntax_trees::expression::{ExpressionHandle, ExpressionNode};
use psi_syntax_trees::item::{Item, StateHandle, StateParameterHandle};
use psi_syntax_trees::statement::{StatementHandle, StatementNode};
use psi_syntax_trees::types::TypeReferenceNode;
use psi_tokens_to_syntax_trees::parse_syntax_trees;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

pub const BUILD_FILE_NAME: &str = "build.omg";
const BUILD_MACHINE_NAME: &str = "build";
const BUILD_TYPE_NAME: &str = "Build";
const BUILDER_PARAMETER_NAME: &str = "builder";
const PACKAGE_MACHINE_NAME: &str = "package";
const APPLICATION_MACHINE_NAME: &str = "application";
const MEMBER_MACHINE_NAME: &str = "member";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProjectName(String);

impl ProjectName {
    pub fn parse(value: impl Into<String>) -> Result<Self, String> {
        let value = value.into();
        if is_kebab_case(&value) {
            Ok(Self(value))
        } else {
            Err(format!(
                "package identity `{value}` must start with a lowercase letter and use kebab-case lowercase words"
            ))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WorkspaceMemberPath(String);

impl WorkspaceMemberPath {
    pub fn parse(value: impl Into<String>) -> Result<Self, InvalidWorkspaceMemberPath> {
        let value = value.into();
        if value.is_empty()
            || value.starts_with('/')
            || value.ends_with('/')
            || value.contains('\\')
            || value.bytes().any(|byte| byte.is_ascii_control())
            || value.split('/').any(|component| {
                component.is_empty()
                    || matches!(component, "." | "..")
                    || !component.bytes().all(is_portable_path_byte)
            })
        {
            return Err(InvalidWorkspaceMemberPath);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidWorkspaceMemberPath;

impl fmt::Display for InvalidWorkspaceMemberPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(
            "workspace member path must be a canonical portable relative path without `.` or `..` components",
        )
    }
}

impl std::error::Error for InvalidWorkspaceMemberPath {}

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
    pub name: ProjectName,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicationDeclaration {
    pub name: ProjectName,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceDeclaration {
    pub members: Vec<WorkspaceMemberPath>,
}

/// A validated role plus the exact syntax entry that established it.
///
/// Consumers projecting adjacent build syntax must use this handle instead of
/// rediscovering the build machine or reproducing its signature checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildDeclarationSyntaxProjection {
    declaration: BuildDeclaration,
    build: BuildEntrySyntaxProjection,
}

/// The structurally validated root build entry, before requiring a project
/// role declaration from its body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildEntrySyntaxProjection {
    build_entry: StateHandle,
    builder_parameter: StateParameterHandle,
}

impl BuildDeclarationSyntaxProjection {
    pub const fn declaration(&self) -> &BuildDeclaration {
        &self.declaration
    }

    pub fn into_declaration(self) -> BuildDeclaration {
        self.declaration
    }

    pub const fn build_entry(&self) -> StateHandle {
        self.build.build_entry
    }

    pub const fn builder_parameter(&self) -> StateParameterHandle {
        self.build.builder_parameter
    }
}

impl BuildEntrySyntaxProjection {
    pub const fn build_entry(&self) -> StateHandle {
        self.build_entry
    }

    pub const fn builder_parameter(&self) -> StateParameterHandle {
        self.builder_parameter
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildDeclarationError {
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

impl fmt::Display for BuildDeclarationError {
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

impl std::error::Error for BuildDeclarationError {}

/// Read and project the authoritative declaration from a root `build.omg`.
pub fn extract_build_declaration(
    root: impl AsRef<Path>,
) -> Result<BuildDeclaration, BuildDeclarationError> {
    let build_path = root.as_ref().join(BUILD_FILE_NAME);
    let source_bytes = match fs::read(&build_path) {
        Ok(source) => source,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(BuildDeclarationError::MissingBuildFile { path: build_path });
        }
        Err(error) => {
            return Err(BuildDeclarationError::ReadBuildFile {
                path: build_path,
                message: error.to_string(),
            });
        }
    };
    let source = std::str::from_utf8(&source_bytes).map_err(|_| {
        BuildDeclarationError::InvalidBuildFileEncoding {
            path: build_path.clone(),
        }
    })?;
    project_build_declaration_from_source(source)
}

/// Parse source and project its authoritative role without evaluating it.
pub fn project_build_declaration_from_source(
    source: &str,
) -> Result<BuildDeclaration, BuildDeclarationError> {
    let tokens = Lexer::new(source)
        .tokenize()
        .map_err(|error| BuildDeclarationError::Lex {
            message: error.message,
        })?;
    let syntax_trees =
        parse_syntax_trees(&tokens).map_err(|error| BuildDeclarationError::Parse {
            message: error.message,
        })?;
    project_build_declaration_from_syntax_trees(&syntax_trees)
}

/// Project only the role value from an already-parsed syntax tree.
pub fn project_build_declaration_from_syntax_trees(
    syntax_trees: &SyntaxTrees,
) -> Result<BuildDeclaration, BuildDeclarationError> {
    project_build_declaration_syntax(syntax_trees)
        .map(BuildDeclarationSyntaxProjection::into_declaration)
}

/// Validate and locate the root build entry without requiring a role call.
///
/// This is the common seam for adjacent syntactic projectors that must retain
/// their own historical error ordering before the role is required.
pub fn project_build_entry_syntax(
    syntax_trees: &SyntaxTrees,
) -> Result<BuildEntrySyntaxProjection, BuildDeclarationError> {
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
        return Err(BuildDeclarationError::ScopedBuildMachine {
            scope: scoped
                .attached_data
                .as_ref()
                .expect("scoped build has an owner")
                .as_str()
                .to_owned(),
        });
    }
    if named_builds.len() > 1 {
        return Err(BuildDeclarationError::DuplicateBuildMachines {
            count: named_builds.len(),
        });
    }

    let Some(build) = named_builds.first() else {
        return Err(BuildDeclarationError::MissingBuildDeclaration);
    };
    if build.bodyless
        || build.boundary
        || build.target.is_some()
        || !build.lifetime_parameters.is_empty()
        || !build.type_parameters.is_empty()
    {
        return Err(BuildDeclarationError::InvalidBuildMachine);
    }

    let build_entry = *syntax_trees
        .items
        .state_handles(build.states)
        .first()
        .ok_or(BuildDeclarationError::MissingBuildEntry)?;
    let entry = syntax_trees.items.state(build_entry);
    let builder_parameter = *syntax_trees
        .items
        .state_parameters(entry.parameters)
        .first()
        .ok_or(BuildDeclarationError::InvalidBuildParameter)?;
    let builder = syntax_trees.items.state_parameter(builder_parameter);
    if builder.name.as_str() != BUILDER_PARAMETER_NAME || builder.is_const || builder.is_self {
        return Err(BuildDeclarationError::InvalidBuildParameter);
    }
    let TypeReferenceNode::Reference {
        referee,
        access,
        lifetime,
    } = syntax_trees
        .type_references
        .type_reference(builder.type_reference)
    else {
        return Err(BuildDeclarationError::InvalidBuildParameter);
    };
    if lifetime.is_some() || !access.is_exclusive() || !access.is_readable() {
        return Err(BuildDeclarationError::InvalidBuildParameter);
    }
    if !matches!(
        syntax_trees.type_references.type_reference(*referee),
        TypeReferenceNode::Named(name) if name.as_str() == BUILD_TYPE_NAME
    ) {
        return Err(BuildDeclarationError::InvalidBuildParameter);
    }

    Ok(BuildEntrySyntaxProjection {
        build_entry,
        builder_parameter,
    })
}

/// Project the role and retain the exact validated root build entry.
pub fn project_build_declaration_syntax(
    syntax_trees: &SyntaxTrees,
) -> Result<BuildDeclarationSyntaxProjection, BuildDeclarationError> {
    let build = match project_build_entry_syntax(syntax_trees) {
        Ok(build) => build,
        Err(BuildDeclarationError::MissingBuildDeclaration) => {
            reject_unprojected_build_declaration_syntax(syntax_trees, &[], &[])?;
            return Err(BuildDeclarationError::MissingBuildDeclaration);
        }
        Err(error) => return Err(error),
    };
    project_build_declaration_in_entry(syntax_trees, build)
}

/// Project the role from a root entry previously validated by this crate.
///
/// The opaque entry value prevents adjacent projectors from substituting an
/// arbitrary state while allowing them to preserve diagnostic ordering.
pub fn project_build_declaration_in_entry(
    syntax_trees: &SyntaxTrees,
    build: BuildEntrySyntaxProjection,
) -> Result<BuildDeclarationSyntaxProjection, BuildDeclarationError> {
    let entry = syntax_trees.items.state(build.build_entry());
    let builder = syntax_trees
        .items
        .state_parameter(build.builder_parameter());

    let builder_name = builder.name.as_str();
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
        return Err(BuildDeclarationError::DuplicatePackageDeclarations {
            count: packages.len(),
        });
    }
    if applications.len() > 1 {
        return Err(BuildDeclarationError::DuplicateApplicationDeclarations {
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
        return Err(BuildDeclarationError::MixedBuildDeclarations);
    }
    let declaration = if let Some(package) = packages.pop() {
        BuildDeclaration::Package(package)
    } else if let Some(application) = applications.pop() {
        BuildDeclaration::Application(application)
    } else {
        for (index, member) in members.iter().enumerate() {
            if members[..index].contains(member) {
                return Err(BuildDeclarationError::DuplicateWorkspaceMember {
                    path: member.as_str().to_owned(),
                });
            }
        }
        if members.is_empty() {
            return Err(BuildDeclarationError::MissingBuildDeclaration);
        }
        BuildDeclaration::Workspace(WorkspaceDeclaration { members })
    };

    Ok(BuildDeclarationSyntaxProjection { declaration, build })
}

fn wrong_receiver_error(operation: &str) -> BuildDeclarationError {
    match operation {
        PACKAGE_MACHINE_NAME => BuildDeclarationError::WrongPackageReceiver,
        APPLICATION_MACHINE_NAME => BuildDeclarationError::WrongApplicationReceiver,
        MEMBER_MACHINE_NAME => BuildDeclarationError::WrongMemberReceiver,
        _ => unreachable!("only declaration operations request receiver errors"),
    }
}

fn wrong_arguments_error(operation: &str) -> BuildDeclarationError {
    match operation {
        PACKAGE_MACHINE_NAME => BuildDeclarationError::WrongPackageArguments,
        APPLICATION_MACHINE_NAME => BuildDeclarationError::WrongApplicationArguments,
        MEMBER_MACHINE_NAME => BuildDeclarationError::WrongMemberArguments,
        _ => unreachable!("only declaration operations request argument errors"),
    }
}

fn reject_authored_toolchain_vocabulary(
    syntax_trees: &SyntaxTrees,
) -> Result<(), BuildDeclarationError> {
    for item in syntax_trees.root_items() {
        match package_authored_type_name(item) {
            Some(name) if machine_leaf_name(name) == BUILD_TYPE_NAME => {
                return Err(BuildDeclarationError::AuthoredToolchainVocabulary {
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
                return Err(BuildDeclarationError::AuthoredToolchainVocabulary {
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
) -> Result<ProjectName, BuildDeclarationError> {
    let ExpressionNode::String(name_bytes) = syntax_trees.expressions.expression(name_handle)
    else {
        return Err(BuildDeclarationError::NameNotStringLiteral);
    };
    let name = std::str::from_utf8(name_bytes).map_err(|_| BuildDeclarationError::NameNotUtf8)?;
    ProjectName::parse(name)
        .map_err(|message| BuildDeclarationError::InvalidPackageName { message })
}

fn project_member_path_literal(
    syntax_trees: &SyntaxTrees,
    path_handle: ExpressionHandle,
) -> Result<WorkspaceMemberPath, BuildDeclarationError> {
    let ExpressionNode::String(path_bytes) = syntax_trees.expressions.expression(path_handle)
    else {
        return Err(BuildDeclarationError::MemberPathNotStringLiteral);
    };
    let path =
        std::str::from_utf8(path_bytes).map_err(|_| BuildDeclarationError::MemberPathNotUtf8)?;
    WorkspaceMemberPath::parse(path).map_err(|_| BuildDeclarationError::InvalidWorkspaceMemberPath)
}

fn reject_unprojected_build_declaration_syntax(
    syntax_trees: &SyntaxTrees,
    accepted_statements: &[StatementHandle],
    accepted_literals: &[ExpressionHandle],
) -> Result<(), BuildDeclarationError> {
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

fn unsupported_shape_error(operation: &str) -> BuildDeclarationError {
    match operation {
        PACKAGE_MACHINE_NAME => BuildDeclarationError::UnsupportedPackageShape,
        APPLICATION_MACHINE_NAME => BuildDeclarationError::UnsupportedApplicationShape,
        MEMBER_MACHINE_NAME => BuildDeclarationError::UnsupportedMemberShape,
        _ => unreachable!("only declaration operations request shape errors"),
    }
}

fn is_kebab_case(value: &str) -> bool {
    if !value.as_bytes().first().is_some_and(u8::is_ascii_lowercase) || value.ends_with('-') {
        return false;
    }
    let mut previous_separator = false;
    for byte in value.bytes() {
        if byte == b'-' {
            if previous_separator {
                return false;
            }
            previous_separator = true;
        } else {
            previous_separator = false;
            if !byte.is_ascii_lowercase() && !byte.is_ascii_digit() {
                return false;
            }
        }
    }
    true
}

fn is_portable_path_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(source: &str) -> Result<BuildDeclaration, BuildDeclarationError> {
        project_build_declaration_from_source(source)
    }

    #[test]
    fn source_and_syntax_apis_project_the_same_authoritative_role() {
        let source = r#"
            machine build(builder: &mut Build, filesystem: &mut Filesystem) {
                builder.application("omega-compiler");
            }
        "#;
        let tokens = Lexer::new(source).tokenize().expect("lex fixture");
        let trees = parse_syntax_trees(&tokens).expect("parse fixture");
        let syntax = project_build_declaration_syntax(&trees).expect("project syntax");

        assert_eq!(
            syntax.declaration(),
            &project_build_declaration_from_source(source).expect("project source")
        );
        assert!(matches!(
            syntax.declaration(),
            BuildDeclaration::Application(application)
                if application.name.as_str() == "omega-compiler"
        ));
        assert_eq!(
            trees
                .items
                .state_parameter(syntax.builder_parameter())
                .name
                .as_str(),
            "builder"
        );
        assert_eq!(
            trees
                .items
                .statements(trees.items.state(syntax.build_entry()).statements)
                .len(),
            1
        );
    }

    #[test]
    fn projects_package_application_and_workspace_declarations() {
        assert!(matches!(
            project(r#"machine build(builder: &mut Build) { builder.package("exact-math"); }"#),
            Ok(BuildDeclaration::Package(PackageDeclaration { name }))
                if name.as_str() == "exact-math"
        ));
        assert!(matches!(
            project(r#"machine build(builder: &mut Build) { builder.application("omega"); }"#),
            Ok(BuildDeclaration::Application(ApplicationDeclaration { name }))
                if name.as_str() == "omega"
        ));
        assert_eq!(
            project(
                r#"machine build(builder: &mut Build) { builder.member("omega/std"); builder.member("source/compiler"); }"#,
            ),
            Ok(BuildDeclaration::Workspace(WorkspaceDeclaration {
                members: vec![
                    WorkspaceMemberPath::parse("omega/std").unwrap(),
                    WorkspaceMemberPath::parse("source/compiler").unwrap(),
                ],
            }))
        );
    }

    #[test]
    fn only_the_first_parameter_is_reserved_for_the_builder() {
        assert!(
            project(
                r#"
            machine build(
                builder: &mut Build,
                filesystem: &mut Filesystem,
                console: &mut Console
            ) {
                builder.package("service-backed-build");
                filesystem.prepare();
            }
            "#,
            )
            .is_ok()
        );

        for source in [
            "machine build() {}",
            "machine build(builder: Build) {}",
            "machine build(builder: &Build) {}",
            "machine build(builder: &write Build) {}",
            "machine build(builder: &mut Builder) {}",
            "machine build(build: &mut Build) {}",
            "machine build(filesystem: &mut Filesystem, builder: &mut Build) {}",
        ] {
            assert_eq!(
                project(source),
                Err(BuildDeclarationError::InvalidBuildParameter)
            );
        }
    }

    #[test]
    fn rejects_mixed_duplicate_and_hidden_role_syntax() {
        assert_eq!(
            project(
                r#"machine build(builder: &mut Build) { builder.package("one"); builder.application("two"); }"#,
            ),
            Err(BuildDeclarationError::MixedBuildDeclarations)
        );
        assert_eq!(
            project(
                r#"machine build(builder: &mut Build) { builder.application("one"); builder.application("two"); }"#,
            ),
            Err(BuildDeclarationError::DuplicateApplicationDeclarations { count: 2 })
        );
        assert_eq!(
            project(
                r#"machine helper(builder: &mut Build) { builder.member("hidden"); } machine build(builder: &mut Build) {}"#,
            ),
            Err(BuildDeclarationError::UnsupportedMemberShape)
        );
    }

    #[test]
    fn rejects_wrong_role_receivers_arguments_and_literals() {
        for (source, expected) in [
            (
                r#"machine build(builder: &mut Build) { other.package("wrong"); }"#,
                BuildDeclarationError::WrongPackageReceiver,
            ),
            (
                r#"machine build(builder: &mut Build) { builder.application(); }"#,
                BuildDeclarationError::WrongApplicationArguments,
            ),
            (
                r#"machine build(builder: &mut Build) { builder.member("one", "two"); }"#,
                BuildDeclarationError::WrongMemberArguments,
            ),
            (
                r#"machine build(builder: &mut Build) { builder.package(package_name); }"#,
                BuildDeclarationError::NameNotStringLiteral,
            ),
            (
                r#"machine build(builder: &mut Build) { builder.member(member_path); }"#,
                BuildDeclarationError::MemberPathNotStringLiteral,
            ),
        ] {
            assert_eq!(project(source), Err(expected), "source: {source}");
        }
        assert_eq!(
            project(r#"machine build(builder: &mut Build) { builder.package("\x80name"); }"#),
            Err(BuildDeclarationError::NameNotUtf8)
        );
        assert_eq!(
            project(r#"machine build(builder: &mut Build) { builder.member("\x80path"); }"#),
            Err(BuildDeclarationError::MemberPathNotUtf8)
        );
    }

    #[test]
    fn rejects_duplicate_package_and_workspace_rows() {
        assert_eq!(
            project(
                r#"machine build(builder: &mut Build) { builder.package("one"); builder.package("two"); }"#,
            ),
            Err(BuildDeclarationError::DuplicatePackageDeclarations { count: 2 })
        );
        assert_eq!(
            project(
                r#"machine build(builder: &mut Build) { builder.member("same/path"); builder.member("same/path"); }"#,
            ),
            Err(BuildDeclarationError::DuplicateWorkspaceMember {
                path: "same/path".to_owned()
            })
        );
    }

    #[test]
    fn rejects_role_calls_outside_the_direct_root_entry_shape() {
        for source in [
            r#"machine helper(builder: &mut Build) { builder.package("hidden"); } machine build(builder: &mut Build) {}"#,
            r#"machine build(builder: &mut Build) { state later(builder: &mut Build) { builder.package("nested"); } }"#,
            r#"machine build(builder: &mut Build) { consume(builder.package("expression")); }"#,
        ] {
            assert_eq!(
                project(source),
                Err(BuildDeclarationError::UnsupportedPackageShape),
                "source: {source}"
            );
        }
    }

    #[test]
    fn source_api_reports_lex_and_parse_failures() {
        assert!(matches!(
            project("machine build(builder: &mut Build) { ` }"),
            Err(BuildDeclarationError::Lex { .. })
        ));
        assert!(matches!(
            project("machine build(builder: &mut Build) {"),
            Err(BuildDeclarationError::Parse { .. })
        ));
    }

    #[test]
    fn rejects_spoofed_toolchain_vocabulary_and_nonordinary_builds() {
        for source in [
            r#"data Build {} machine build(builder: &mut Build) { builder.package("spoof"); }"#,
            r#"machine Build::package(name: &[u8]) {} machine build(builder: &mut Build) { builder.package("spoof"); }"#,
        ] {
            assert!(matches!(
                project(source),
                Err(BuildDeclarationError::AuthoredToolchainVocabulary { .. })
            ));
        }
        assert!(matches!(
            project(r#"machine Owner::build(builder: &mut Build) { builder.package("scoped"); }"#),
            Err(BuildDeclarationError::ScopedBuildMachine { .. })
        ));
        assert_eq!(
            project("boundary machine build(builder: &mut Build);"),
            Err(BuildDeclarationError::InvalidBuildMachine)
        );
    }

    #[test]
    fn validated_names_and_member_paths_match_the_declared_canonical_forms() {
        for name in [
            "Arithmetic-Kernels",
            "arithmetic_kernels",
            "arithmetic--kernels",
            "123-tools",
        ] {
            assert!(ProjectName::parse(name).is_err(), "accepted {name:?}");
        }
        for path in ["", "/absolute", "packages/../escape", "packages//double"] {
            assert!(
                WorkspaceMemberPath::parse(path).is_err(),
                "accepted {path:?}"
            );
        }
    }
}
