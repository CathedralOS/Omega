/*
Syntax-tree projection for `build.omg`.

We locate the canonical root `machine build(builder: &mut Build)` entry, read
its direct `Build::{package,application,member,artifact_only}` statements, and
reject every call shape that would require evaluation or hide a role statement
inside a helper state, nested machine, or subexpression.

Why inspect the expression table after scanning the root entry's statements?
Statement calls are parsed as expressions first and then wrapped in statement
nodes, so `syntax_trees.expressions` contains one `ExpressionNode::Call` for
every call anywhere in the file. An accepted root-entry statement's call
expression is the unique occurrence whose argument-handle slice matches the
statement's argument-handle slice; any other call targeting a role operation
anywhere in the file fails with that operation's `Unsupported*Shape` error.
*/

use crate::build_declaration::{
    APPLICATION_MACHINE_NAME, ARTIFACT_ONLY_MACHINE_NAME, BUILD_MACHINE_NAME, BUILD_TYPE_NAME,
    BUILDER_PARAMETER_NAME, MEMBER_MACHINE_NAME, PACKAGE_MACHINE_NAME,
};
use crate::dependencies::is_dependency_call;
use crate::direct_build_call::{DirectBuildCallError, direct_build_call_operands};
use crate::{
    ApplicationDeclaration, BuildDeclaration, BuildDeclarationError, BuildDeclarationKind,
    PackageDeclaration, ProjectName, WorkspaceDeclaration, WorkspaceMemberPath,
};
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{ExpressionHandle, ExpressionNode};
use syntax_trees::item::{Item, StateHandle, StateParameterHandle};
use syntax_trees::statement::StatementNode;
use syntax_trees::types::TypeReferenceNode;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildDeclarationSyntaxProjection {
    declaration: BuildDeclaration,
    build: BuildEntrySyntaxProjection,
}

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

pub fn project_build_declaration_from_source(
    source: &str,
) -> Result<BuildDeclaration, BuildDeclarationError> {
    let parsed = psi_compiler::parse_text(source).map_err(|error| match error {
        psi_compiler::SourceParseError::Lex { message } => BuildDeclarationError::Lex { message },
        psi_compiler::SourceParseError::Parse { message } => {
            BuildDeclarationError::Parse { message }
        }
    })?;
    project_build_declaration_from_syntax_trees(&parsed.syntax)
}

pub(crate) fn project_build_declaration_from_syntax_trees(
    syntax_trees: &SyntaxTrees,
) -> Result<BuildDeclaration, BuildDeclarationError> {
    project_build_declaration_syntax(syntax_trees)
        .map(BuildDeclarationSyntaxProjection::into_declaration)
}

pub fn project_build_entry_syntax(
    syntax_trees: &SyntaxTrees,
) -> Result<BuildEntrySyntaxProjection, BuildDeclarationError> {
    reject_authored_toolchain_vocabulary(syntax_trees)?;
    let named_builds = syntax_trees
        .root_items()
        .filter_map(|item| match item {
            Item::Machine(machine)
                if machine_leaf_name(machine.name.as_str(syntax_trees)) == BUILD_MACHINE_NAME =>
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
                .as_str(syntax_trees)
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
    if builder.name.as_str(syntax_trees) != BUILDER_PARAMETER_NAME || builder.is_const {
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
        TypeReferenceNode::Named(name) if name.as_str(syntax_trees) == BUILD_TYPE_NAME
    ) {
        return Err(BuildDeclarationError::InvalidBuildParameter);
    }

    Ok(BuildEntrySyntaxProjection {
        build_entry,
        builder_parameter,
    })
}

pub(crate) fn project_build_declaration_syntax(
    syntax_trees: &SyntaxTrees,
) -> Result<BuildDeclarationSyntaxProjection, BuildDeclarationError> {
    let build = match project_build_entry_syntax(syntax_trees) {
        Ok(build) => build,
        Err(BuildDeclarationError::MissingBuildDeclaration) => {
            reject_unprojected_build_declaration_syntax(syntax_trees, &[])?;
            return Err(BuildDeclarationError::MissingBuildDeclaration);
        }
        Err(error) => return Err(error),
    };
    project_build_declaration_in_entry(syntax_trees, build)
}

pub fn project_build_declaration_in_entry(
    syntax_trees: &SyntaxTrees,
    build: BuildEntrySyntaxProjection,
) -> Result<BuildDeclarationSyntaxProjection, BuildDeclarationError> {
    let entry = syntax_trees.items.state(build.build_entry());

    let mut packages = Vec::new();
    let mut applications = Vec::new();
    let mut members = Vec::new();
    let mut artifact_only = 0usize;
    let mut accepted: Vec<&[ExpressionHandle]> = Vec::new();
    for statement_handle in syntax_trees.items.statements(entry.statements) {
        let StatementNode::Call(call) = syntax_trees.statements.statement(*statement_handle) else {
            continue;
        };
        let Some(operation) = DeclarationOperation::classify(call.target.as_str(syntax_trees))
        else {
            continue;
        };
        let operands =
            direct_build_call_operands(syntax_trees, call).map_err(|failure| match failure {
                DirectBuildCallError::WrongReceiver => operation.wrong_receiver(),
                DirectBuildCallError::WrongArguments => operation.wrong_arguments(),
            })?;
        if operation == DeclarationOperation::ArtifactOnly {
            if !operands.is_empty() {
                return Err(operation.wrong_arguments());
            }
            artifact_only += 1;
        } else {
            let [literal] = operands else {
                return Err(operation.wrong_arguments());
            };
            match operation {
                DeclarationOperation::Package => packages.push(PackageDeclaration {
                    name: project_name_literal(syntax_trees, *literal)?,
                }),
                DeclarationOperation::Application => applications.push(ApplicationDeclaration {
                    name: project_name_literal(syntax_trees, *literal)?,
                    artifact_only: false,
                }),
                DeclarationOperation::Member => {
                    members.push(project_member_path_literal(syntax_trees, *literal)?)
                }
                DeclarationOperation::ArtifactOnly => {
                    unreachable!("artifact-only carries no literal")
                }
            }
        }
        accepted.push(syntax_trees.statements.expression_handles(call.arguments));
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
    reject_unprojected_build_declaration_syntax(syntax_trees, &accepted)?;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeclarationOperation {
    Package,
    Application,
    Member,
    ArtifactOnly,
}

impl DeclarationOperation {
    fn classify(target: &str) -> Option<Self> {
        Some(match target {
            PACKAGE_MACHINE_NAME => Self::Package,
            APPLICATION_MACHINE_NAME => Self::Application,
            MEMBER_MACHINE_NAME => Self::Member,
            ARTIFACT_ONLY_MACHINE_NAME => Self::ArtifactOnly,
            _ => return None,
        })
    }

    fn wrong_receiver(self) -> BuildDeclarationError {
        match self {
            Self::Package => BuildDeclarationError::WrongPackageReceiver,
            Self::Application => BuildDeclarationError::WrongApplicationReceiver,
            Self::Member => BuildDeclarationError::WrongMemberReceiver,
            Self::ArtifactOnly => BuildDeclarationError::WrongArtifactOnlyReceiver,
        }
    }

    fn wrong_arguments(self) -> BuildDeclarationError {
        match self {
            Self::Package => BuildDeclarationError::WrongPackageArguments,
            Self::Application => BuildDeclarationError::WrongApplicationArguments,
            Self::Member => BuildDeclarationError::WrongMemberArguments,
            Self::ArtifactOnly => BuildDeclarationError::WrongArtifactOnlyArguments,
        }
    }

    fn unsupported_shape(self) -> BuildDeclarationError {
        match self {
            Self::Package => BuildDeclarationError::UnsupportedPackageShape,
            Self::Application => BuildDeclarationError::UnsupportedApplicationShape,
            Self::Member => BuildDeclarationError::UnsupportedMemberShape,
            Self::ArtifactOnly => BuildDeclarationError::UnsupportedArtifactOnlyShape,
        }
    }
}

fn reject_authored_toolchain_vocabulary(
    syntax_trees: &SyntaxTrees,
) -> Result<(), BuildDeclarationError> {
    for item in syntax_trees.root_items() {
        if let Some(name) = package_authored_type_name(syntax_trees, item)
            && machine_leaf_name(name) == BUILD_TYPE_NAME
        {
            return Err(BuildDeclarationError::AuthoredToolchainVocabulary {
                name: BUILD_TYPE_NAME.to_owned(),
            });
        }
        let Item::Machine(machine) = item else {
            continue;
        };
        let Some(owner) = machine.attached_data.as_ref() else {
            continue;
        };
        if owner.as_str(syntax_trees) != BUILD_TYPE_NAME {
            continue;
        }
        let leaf = machine_leaf_name(machine.name.as_str(syntax_trees));
        if DeclarationOperation::classify(leaf).is_some()
            || is_dependency_call(Some(BUILD_TYPE_NAME), leaf)
        {
            return Err(BuildDeclarationError::AuthoredToolchainVocabulary {
                name: machine.name.as_str(syntax_trees).to_owned(),
            });
        }
    }
    Ok(())
}

pub fn package_authored_type_name<'syntax>(
    syntax_trees: &'syntax SyntaxTrees,
    item: &'syntax Item,
) -> Option<&'syntax str> {
    match item {
        Item::Data(data) => Some(data.name.as_str(syntax_trees)),
        Item::Domain(domain) => Some(domain.name.as_str(syntax_trees)),
        Item::Trait(definition) => Some(definition.name.as_str(syntax_trees)),
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
    accepted: &[&[ExpressionHandle]],
) -> Result<(), BuildDeclarationError> {
    for (_, expression) in syntax_trees.expressions.iter_expressions() {
        let ExpressionNode::Call(call) = expression else {
            continue;
        };
        let Some(operation) = DeclarationOperation::classify(call.target.as_str(syntax_trees))
        else {
            continue;
        };
        let arguments = syntax_trees.expressions.expression_handles(call.arguments);
        if !accepted.contains(&arguments) {
            return Err(operation.unsupported_shape());
        }
    }
    Ok(())
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
