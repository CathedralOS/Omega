//! Projecting a build declaration from source or syntax trees: locating the
//! canonical build entry, reading the role call, and rejecting every shape
//! that would need evaluation.

use crate::build_declaration::{
    APPLICATION_MACHINE_NAME, ARTIFACT_ONLY_MACHINE_NAME, BUILD_MACHINE_NAME, BUILD_TYPE_NAME,
    BUILDER_PARAMETER_NAME, MEMBER_MACHINE_NAME, PACKAGE_MACHINE_NAME,
};
use crate::dependencies::is_dependency_call_name;
use crate::{
    ApplicationDeclaration, BUILD_FILE_NAME, BuildDeclaration, BuildDeclarationError,
    BuildDeclarationKind, PackageDeclaration, ProjectName, WorkspaceDeclaration,
    WorkspaceMemberPath,
};
use source_files_to_tokens::Lexer;
use std::fs;
use std::path::Path;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{ExpressionHandle, ExpressionNode};
use syntax_trees::item::{Item, StateHandle, StateParameterHandle};
use syntax_trees::statement::{StatementHandle, StatementNode};
use syntax_trees::types::TypeReferenceNode;
use tokens_to_syntax_trees::parse_syntax_trees;

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
    let parameters = syntax_trees.items.state_parameters(entry.parameters);
    let [builder_parameter] = parameters else {
        return Err(BuildDeclarationError::InvalidBuildParameter);
    };
    let builder_parameter = *builder_parameter;
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
            reject_unprojected_build_declaration_syntax(syntax_trees, &[], &[], 0)?;
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
    let mut artifact_only = 0usize;
    let mut accepted_statements = Vec::new();
    let mut accepted_literals = Vec::new();
    for statement_handle in syntax_trees.items.statements(entry.statements) {
        let StatementNode::Call(call) = syntax_trees.statements.statement(*statement_handle) else {
            continue;
        };
        let operation = call.target.as_str();
        if !matches!(
            operation,
            PACKAGE_MACHINE_NAME
                | APPLICATION_MACHINE_NAME
                | MEMBER_MACHINE_NAME
                | ARTIFACT_ONLY_MACHINE_NAME
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
        // The artifact-only application modifier is unconditional: it takes no
        // name or evidence operand, so there is no literal to retain.
        if operation == ARTIFACT_ONLY_MACHINE_NAME {
            if !call.arguments.is_empty() {
                return Err(BuildDeclarationError::WrongArtifactOnlyArguments);
            }
            artifact_only += 1;
            accepted_statements.push(*statement_handle);
            continue;
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
                artifact_only: false,
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
    if artifact_only > 1 {
        return Err(BuildDeclarationError::DuplicateArtifactOnlyDeclarations {
            count: artifact_only,
        });
    }
    reject_unprojected_build_declaration_syntax(
        syntax_trees,
        &accepted_statements,
        &accepted_literals,
        artifact_only,
    )?;
    let declared_kinds = usize::from(!packages.is_empty())
        + usize::from(!applications.is_empty())
        + usize::from(!members.is_empty());
    if declared_kinds > 1 {
        return Err(BuildDeclarationError::MixedBuildDeclarations);
    }
    let declaration = if let Some(package) = packages.pop() {
        if artifact_only > 0 {
            return Err(BuildDeclarationError::ArtifactOnlyRequiresApplication {
                found: BuildDeclarationKind::Package,
            });
        }
        BuildDeclaration::Package(package)
    } else if let Some(mut application) = applications.pop() {
        application.artifact_only = artifact_only > 0;
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
            // Artifact-only alone is a modifier, not a role: the build still
            // owes one package/application/workspace declaration.
            return Err(BuildDeclarationError::MissingBuildDeclaration);
        }
        if artifact_only > 0 {
            return Err(BuildDeclarationError::ArtifactOnlyRequiresApplication {
                found: BuildDeclarationKind::Workspace,
            });
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
        ARTIFACT_ONLY_MACHINE_NAME => BuildDeclarationError::WrongArtifactOnlyReceiver,
        _ => unreachable!("only declaration operations request receiver errors"),
    }
}

fn wrong_arguments_error(operation: &str) -> BuildDeclarationError {
    match operation {
        PACKAGE_MACHINE_NAME => BuildDeclarationError::WrongPackageArguments,
        APPLICATION_MACHINE_NAME => BuildDeclarationError::WrongApplicationArguments,
        MEMBER_MACHINE_NAME => BuildDeclarationError::WrongMemberArguments,
        ARTIFACT_ONLY_MACHINE_NAME => BuildDeclarationError::WrongArtifactOnlyArguments,
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
                    && (matches!(
                        machine_leaf_name(machine.name.as_str()),
                        PACKAGE_MACHINE_NAME
                            | APPLICATION_MACHINE_NAME
                            | MEMBER_MACHINE_NAME
                            | ARTIFACT_ONLY_MACHINE_NAME
                    ) || is_dependency_call_name(machine_leaf_name(machine.name.as_str()))) =>
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
    accepted_artifact_only: usize,
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
                    PACKAGE_MACHINE_NAME
                        | APPLICATION_MACHINE_NAME
                        | MEMBER_MACHINE_NAME
                        | ARTIFACT_ONLY_MACHINE_NAME
                ) && !accepted_statements.contains(statement_handle)
                {
                    return Err(unsupported_shape_error(call.target.as_str()));
                }
            }
        }
    }

    // Statement calls are parsed as expressions first and then converted, so
    // every accepted declaration statement leaves one matching
    // `ExpressionNode::Call` behind in the expression table. Named
    // declarations are correlated through their single literal argument.
    // `builder.artifact_only()` carries no literal, so its zero-argument
    // leftovers are counted instead: an expression-position occurrence (for
    // example nested under `consume(...)`) has no accepted statement and tips
    // the count past the accepted total.
    let mut artifact_only_leftovers = 0usize;
    for (_, expression) in syntax_trees.expressions.iter_expressions() {
        if let ExpressionNode::Call(call) = expression
            && matches!(
                call.target.as_str(),
                PACKAGE_MACHINE_NAME
                    | APPLICATION_MACHINE_NAME
                    | MEMBER_MACHINE_NAME
                    | ARTIFACT_ONLY_MACHINE_NAME
            )
        {
            let arguments = syntax_trees.expressions.expression_handles(call.arguments);
            if call.target.as_str() == ARTIFACT_ONLY_MACHINE_NAME {
                if arguments.is_empty() {
                    artifact_only_leftovers += 1;
                    continue;
                }
                return Err(unsupported_shape_error(call.target.as_str()));
            }
            if !matches!(arguments, [literal] if accepted_literals.contains(literal)) {
                return Err(unsupported_shape_error(call.target.as_str()));
            }
        }
    }
    if artifact_only_leftovers != accepted_artifact_only {
        return Err(BuildDeclarationError::UnsupportedArtifactOnlyShape);
    }
    Ok(())
}

fn unsupported_shape_error(operation: &str) -> BuildDeclarationError {
    match operation {
        PACKAGE_MACHINE_NAME => BuildDeclarationError::UnsupportedPackageShape,
        APPLICATION_MACHINE_NAME => BuildDeclarationError::UnsupportedApplicationShape,
        MEMBER_MACHINE_NAME => BuildDeclarationError::UnsupportedMemberShape,
        ARTIFACT_ONLY_MACHINE_NAME => BuildDeclarationError::UnsupportedArtifactOnlyShape,
        _ => unreachable!("only declaration operations request shape errors"),
    }
}

pub(crate) fn is_snake_case(value: &str) -> bool {
    if !value.as_bytes().first().is_some_and(u8::is_ascii_lowercase) || value.ends_with('_') {
        return false;
    }
    let mut previous_separator = false;
    for byte in value.bytes() {
        if byte == b'_' {
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

pub(crate) fn is_portable_path_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_')
}
