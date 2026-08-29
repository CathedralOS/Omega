//! Hermetic projection of literal dependency requests from `build.omg`.

use crate::declarations::declaration::{
    BuildDeclaration, BuildDeclarationError, convert_shared_declaration,
};
use crate::resolution::identity::{AliasName, PackageName};
use omega_build_declarations as shared;
use psi_source_files_to_tokens::Lexer;
use psi_syntax_trees::SyntaxTrees;
use psi_syntax_trees::expression::{ExpressionHandle, ExpressionNode};
use psi_syntax_trees::item::Item;
use psi_syntax_trees::statement::{StatementHandle, StatementNode};
use psi_tokens_to_syntax_trees::parse_syntax_trees;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

const BUILD_FILE_NAME: &str = "build.omg";
const BUILD_TYPE_NAME: &str = "Build";
const SOURCE_TYPE_NAME: &str = "Source";
const DEPEND_MACHINE_NAME: &str = "depend";
const DEPEND_AS_MACHINE_NAME: &str = "depend_as";

/// One source request projected without evaluating `build.omg`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DependencySourceRequest {
    Path {
        explicit_alias: Option<AliasName>,
        location: String,
    },
    Git {
        explicit_alias: Option<AliasName>,
        repository: String,
        revision: String,
    },
}

impl DependencySourceRequest {
    pub fn explicit_alias(&self) -> Option<&AliasName> {
        match self {
            Self::Path { explicit_alias, .. } | Self::Git { explicit_alias, .. } => {
                explicit_alias.as_ref()
            }
        }
    }

    /// Resolve the requester-local import name after source custody has read
    /// the dependency's own package declaration.
    ///
    /// The package-authored name supplies the ordinary alias. An explicit
    /// `depend_as` alias is only a local name-resolution override and never
    /// participates in package or source identity.
    pub fn resolved_alias(&self, package_name: &PackageName) -> AliasName {
        self.explicit_alias()
            .cloned()
            .unwrap_or_else(|| package_name.default_alias())
    }
}

/// One authoritative project role and its direct dependency requests,
/// projected from the same parsed `build.omg` tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildDependencyProjection {
    declaration: BuildDeclaration,
    dependencies: Vec<DependencySourceRequest>,
}

impl BuildDependencyProjection {
    pub const fn declaration(&self) -> &BuildDeclaration {
        &self.declaration
    }

    pub fn dependencies(&self) -> &[DependencySourceRequest] {
        &self.dependencies
    }

    pub fn into_parts(self) -> (BuildDeclaration, Vec<DependencySourceRequest>) {
        (self.declaration, self.dependencies)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DependencyProjectionError {
    MissingBuildFile { path: PathBuf },
    ReadBuildFile { path: PathBuf, message: String },
    InvalidBuildFileEncoding { path: PathBuf },
    Lex { message: String },
    Parse { message: String },
    BuildDeclaration(Box<BuildDeclarationError>),
    AuthoredToolchainVocabulary { name: String },
    ScopedBuildMachine { scope: String },
    DuplicateBuildMachines { count: usize },
    InvalidBuildMachine,
    MissingBuildEntry,
    InvalidBuildParameter,
    UnsupportedDependencyShape,
    WrongDependencyReceiver,
    WrongDependencyArguments,
    AliasNotString,
    AliasNotUtf8,
    InvalidAlias { alias: String },
    SourceNotLiteral,
    WrongSourceType,
    MissingSourceCase,
    UnsupportedSourceCase { case_name: String },
    WrongSourceFields { case_name: String },
    SourceFieldNotString { field: String },
    SourceFieldNotUtf8 { field: String },
}

impl fmt::Display for DependencyProjectionError {
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
            Self::BuildDeclaration(error) => {
                write!(formatter, "cannot project dependencies without a valid project role: {error}")
            }
            Self::AuthoredToolchainVocabulary { name } => write!(
                formatter,
                "package build must not declare toolchain dependency vocabulary `{name}`"
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
            Self::UnsupportedDependencyShape => formatter.write_str(
                "dependency requests must be direct canonical `builder.depend(Source::...)` or `builder.depend_as(alias, Source::...)` statements in the root build entry",
            ),
            Self::WrongDependencyReceiver => formatter.write_str(
                "dependency request receiver must be the root build machine's first parameter",
            ),
            Self::WrongDependencyArguments => formatter.write_str(
                "`builder.depend` must have one source argument and `builder.depend_as` must have one direct alias literal followed by one source argument; neither accepts static, evidence, operational, or discard modifiers",
            ),
            Self::AliasNotString => formatter.write_str(
                "dependency alias must be a direct string literal",
            ),
            Self::AliasNotUtf8 => formatter.write_str(
                "dependency alias must contain UTF-8 bytes",
            ),
            Self::InvalidAlias { alias } => write!(
                formatter,
                "dependency alias `{alias}` must use snake_case Omega identifier spelling"
            ),
            Self::SourceNotLiteral => formatter.write_str(
                "dependency source must be a direct `Source::Path` or `Source::Git` literal",
            ),
            Self::WrongSourceType => {
                formatter.write_str("dependency source literal must construct `Source`")
            }
            Self::MissingSourceCase => formatter.write_str(
                "dependency source must select the `Source::Path` or `Source::Git` case",
            ),
            Self::UnsupportedSourceCase { case_name } => {
                write!(formatter, "unsupported dependency source case `Source::{case_name}`")
            }
            Self::WrongSourceFields { case_name } => {
                write!(formatter, "`Source::{case_name}` has noncanonical fields")
            }
            Self::SourceFieldNotString { field } => {
                write!(formatter, "dependency source field `{field}` must be a direct string literal")
            }
            Self::SourceFieldNotUtf8 { field } => {
                write!(formatter, "dependency source field `{field}` must contain UTF-8 bytes")
            }
        }
    }
}

impl std::error::Error for DependencyProjectionError {}

/// Project direct dependency declarations from the immutable package root.
///
/// This parses only the root `build.omg`; it does not evaluate build code,
/// imports, constants, helpers, control flow, or providers. The same parsed
/// tree must first produce one authoritative package, application, or workspace
/// declaration; absence is not a second implicit project kind.
pub fn extract_dependency_projection(
    package_root: impl AsRef<Path>,
) -> Result<Vec<DependencySourceRequest>, DependencyProjectionError> {
    extract_build_dependency_projection(package_root).map(|projection| projection.dependencies)
}

/// Project the project role and direct dependencies together from one parse.
pub fn extract_build_dependency_projection(
    package_root: impl AsRef<Path>,
) -> Result<BuildDependencyProjection, DependencyProjectionError> {
    let build_path = package_root.as_ref().join(BUILD_FILE_NAME);
    let source_bytes = match fs::read(&build_path) {
        Ok(source) => source,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(DependencyProjectionError::MissingBuildFile { path: build_path });
        }
        Err(error) => {
            return Err(DependencyProjectionError::ReadBuildFile {
                path: build_path,
                message: error.to_string(),
            });
        }
    };
    let source = std::str::from_utf8(&source_bytes).map_err(|_| {
        DependencyProjectionError::InvalidBuildFileEncoding {
            path: build_path.clone(),
        }
    })?;
    extract_build_projection_from_source(source)
}

pub(crate) fn extract_from_source(
    source: &str,
) -> Result<Vec<DependencySourceRequest>, DependencyProjectionError> {
    extract_build_projection_from_source(source).map(|projection| projection.dependencies)
}

fn extract_build_projection_from_source(
    source: &str,
) -> Result<BuildDependencyProjection, DependencyProjectionError> {
    let tokens = Lexer::new(source)
        .tokenize()
        .map_err(|error| DependencyProjectionError::Lex {
            message: error.message,
        })?;
    let syntax_trees =
        parse_syntax_trees(&tokens).map_err(|error| DependencyProjectionError::Parse {
            message: error.message,
        })?;
    extract_build_projection_from_syntax_trees(&syntax_trees)
}

fn extract_build_projection_from_syntax_trees(
    syntax_trees: &SyntaxTrees,
) -> Result<BuildDependencyProjection, DependencyProjectionError> {
    reject_authored_toolchain_vocabulary(syntax_trees)?;
    let build_entry = match shared::project_build_entry_syntax(syntax_trees) {
        Ok(projection) => projection,
        Err(error @ shared::BuildDeclarationError::MissingBuildDeclaration) => {
            reject_unprojected_dependency_syntax(syntax_trees, &[], &[], &[])?;
            return Err(map_build_declaration_error(error));
        }
        Err(error) => return Err(map_build_declaration_error(error)),
    };
    let entry_handle = build_entry.build_entry();
    let entry = syntax_trees.items.state(entry_handle);
    let builder_parameter = syntax_trees
        .items
        .state_parameter(build_entry.builder_parameter());
    let builder_name = builder_parameter.name.as_str();
    let mut requests = Vec::new();
    let mut accepted_statements = Vec::new();
    let mut accepted_sources = Vec::new();
    let mut accepted_aliases = Vec::new();
    for statement_handle in syntax_trees.items.statements(entry.statements) {
        let StatementNode::Call(call) = syntax_trees.statements.statement(*statement_handle) else {
            continue;
        };
        if !matches!(
            call.target.as_str(),
            DEPEND_MACHINE_NAME | DEPEND_AS_MACHINE_NAME
        ) {
            continue;
        }
        if call.receiver_starts_at_self
            || !matches!(
                syntax_trees.statements.identifier_path_members(call.receiver),
                [receiver] if receiver.as_str() == builder_name
            )
        {
            return Err(DependencyProjectionError::WrongDependencyReceiver);
        }
        if !call.machine_arguments.is_empty()
            || !call.evidence_arguments.is_empty()
            || call.operational_acknowledgement != Default::default()
            || call.discards_result
        {
            return Err(DependencyProjectionError::WrongDependencyArguments);
        }
        let arguments = syntax_trees.statements.expression_handles(call.arguments);
        let (explicit_alias, source_handle) = match call.target.as_str() {
            DEPEND_MACHINE_NAME => {
                let [source_handle] = arguments else {
                    return Err(DependencyProjectionError::WrongDependencyArguments);
                };
                (None, *source_handle)
            }
            DEPEND_AS_MACHINE_NAME => {
                let [alias_handle, source_handle] = arguments else {
                    return Err(DependencyProjectionError::WrongDependencyArguments);
                };
                let alias = project_alias_literal(syntax_trees, *alias_handle)?;
                accepted_aliases.push(*alias_handle);
                (Some(alias), *source_handle)
            }
            _ => unreachable!("dependency operation filtered above"),
        };
        let request = project_source_literal(syntax_trees, source_handle, explicit_alias)?;
        requests.push(request);
        accepted_statements.push(*statement_handle);
        accepted_sources.push(source_handle);
    }

    reject_unprojected_dependency_syntax(
        syntax_trees,
        &accepted_statements,
        &accepted_sources,
        &accepted_aliases,
    )?;
    let role_projection = shared::project_build_declaration_in_entry(syntax_trees, build_entry)
        .map_err(map_build_declaration_error)?;
    let declaration = convert_shared_declaration(role_projection.into_declaration());
    Ok(BuildDependencyProjection {
        declaration,
        dependencies: requests,
    })
}

fn map_build_declaration_error(error: BuildDeclarationError) -> DependencyProjectionError {
    match error {
        BuildDeclarationError::ScopedBuildMachine { scope } => {
            DependencyProjectionError::ScopedBuildMachine { scope }
        }
        BuildDeclarationError::DuplicateBuildMachines { count } => {
            DependencyProjectionError::DuplicateBuildMachines { count }
        }
        BuildDeclarationError::InvalidBuildMachine => {
            DependencyProjectionError::InvalidBuildMachine
        }
        BuildDeclarationError::MissingBuildEntry => DependencyProjectionError::MissingBuildEntry,
        BuildDeclarationError::InvalidBuildParameter => {
            DependencyProjectionError::InvalidBuildParameter
        }
        error => DependencyProjectionError::BuildDeclaration(Box::new(error)),
    }
}

fn reject_authored_toolchain_vocabulary(
    syntax_trees: &SyntaxTrees,
) -> Result<(), DependencyProjectionError> {
    for item in syntax_trees.root_items() {
        match package_authored_type_name(item) {
            Some(name) if matches!(machine_leaf_name(name), BUILD_TYPE_NAME | SOURCE_TYPE_NAME) => {
                return Err(DependencyProjectionError::AuthoredToolchainVocabulary {
                    name: machine_leaf_name(name).to_owned(),
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
                        DEPEND_MACHINE_NAME | DEPEND_AS_MACHINE_NAME
                    ) =>
            {
                return Err(DependencyProjectionError::AuthoredToolchainVocabulary {
                    name: format!("Build::{}", machine.name.as_str()),
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

fn project_source_literal(
    syntax_trees: &SyntaxTrees,
    source_handle: ExpressionHandle,
    explicit_alias: Option<AliasName>,
) -> Result<DependencySourceRequest, DependencyProjectionError> {
    let ExpressionNode::StructLiteral(literal) = syntax_trees.expressions.expression(source_handle)
    else {
        return Err(DependencyProjectionError::SourceNotLiteral);
    };
    if literal.type_name.as_str() != SOURCE_TYPE_NAME {
        return Err(DependencyProjectionError::WrongSourceType);
    }
    let Some(case_name) = literal.case_name.as_ref() else {
        return Err(DependencyProjectionError::MissingSourceCase);
    };
    let fields = syntax_trees.expressions.struct_fields(literal.fields);
    match case_name.as_str() {
        "Path" => {
            let [field] = fields else {
                return Err(DependencyProjectionError::WrongSourceFields {
                    case_name: "Path".to_owned(),
                });
            };
            if field.name.as_str() != "location" {
                return Err(DependencyProjectionError::WrongSourceFields {
                    case_name: "Path".to_owned(),
                });
            }
            Ok(DependencySourceRequest::Path {
                explicit_alias,
                location: string_field(syntax_trees, "location", field.value)?,
            })
        }
        "Git" => {
            if fields.len() != 2
                || fields
                    .iter()
                    .filter(|field| field.name.as_str() == "repository")
                    .count()
                    != 1
                || fields
                    .iter()
                    .filter(|field| field.name.as_str() == "revision")
                    .count()
                    != 1
            {
                return Err(DependencyProjectionError::WrongSourceFields {
                    case_name: "Git".to_owned(),
                });
            }
            let repository = fields
                .iter()
                .find(|field| field.name.as_str() == "repository")
                .expect("validated Git repository field");
            let revision = fields
                .iter()
                .find(|field| field.name.as_str() == "revision")
                .expect("validated Git revision field");
            Ok(DependencySourceRequest::Git {
                explicit_alias,
                repository: string_field(syntax_trees, "repository", repository.value)?,
                revision: string_field(syntax_trees, "revision", revision.value)?,
            })
        }
        unsupported => Err(DependencyProjectionError::UnsupportedSourceCase {
            case_name: unsupported.to_owned(),
        }),
    }
}

fn project_alias_literal(
    syntax_trees: &SyntaxTrees,
    alias_handle: ExpressionHandle,
) -> Result<AliasName, DependencyProjectionError> {
    let ExpressionNode::String(bytes) = syntax_trees.expressions.expression(alias_handle) else {
        return Err(DependencyProjectionError::AliasNotString);
    };
    let alias = std::str::from_utf8(bytes).map_err(|_| DependencyProjectionError::AliasNotUtf8)?;
    AliasName::parse(alias).map_err(|_| DependencyProjectionError::InvalidAlias {
        alias: alias.to_owned(),
    })
}

fn string_field(
    syntax_trees: &SyntaxTrees,
    field: &str,
    value: ExpressionHandle,
) -> Result<String, DependencyProjectionError> {
    let ExpressionNode::String(bytes) = syntax_trees.expressions.expression(value) else {
        return Err(DependencyProjectionError::SourceFieldNotString {
            field: field.to_owned(),
        });
    };
    std::str::from_utf8(bytes).map(str::to_owned).map_err(|_| {
        DependencyProjectionError::SourceFieldNotUtf8 {
            field: field.to_owned(),
        }
    })
}

fn reject_unprojected_dependency_syntax(
    syntax_trees: &SyntaxTrees,
    accepted_statements: &[StatementHandle],
    accepted_sources: &[ExpressionHandle],
    accepted_aliases: &[ExpressionHandle],
) -> Result<(), DependencyProjectionError> {
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
                    DEPEND_MACHINE_NAME | DEPEND_AS_MACHINE_NAME
                ) && !accepted_statements.contains(statement_handle)
                {
                    return Err(DependencyProjectionError::UnsupportedDependencyShape);
                }
            }
        }
    }

    for (expression_handle, expression) in syntax_trees.expressions.iter_expressions() {
        match expression {
            ExpressionNode::StructLiteral(literal)
                if literal.type_name.as_str() == SOURCE_TYPE_NAME =>
            {
                if !accepted_sources.contains(&expression_handle) {
                    return Err(DependencyProjectionError::UnsupportedDependencyShape);
                }
            }
            ExpressionNode::Call(call)
                if matches!(
                    call.target.as_str(),
                    DEPEND_MACHINE_NAME | DEPEND_AS_MACHINE_NAME
                ) =>
            {
                match (
                    call.target.as_str(),
                    syntax_trees.expressions.expression_handles(call.arguments),
                ) {
                    (DEPEND_MACHINE_NAME, [source]) if accepted_sources.contains(source) => {}
                    (DEPEND_AS_MACHINE_NAME, [alias, source])
                        if accepted_aliases.contains(alias)
                            && accepted_sources.contains(source) => {}
                    _ => return Err(DependencyProjectionError::UnsupportedDependencyShape),
                }
            }
            _ => {}
        }
    }
    Ok(())
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
                "omega-dependency-projection-{}-{nonce}",
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

        fn extract(&self) -> Result<Vec<DependencySourceRequest>, DependencyProjectionError> {
            extract_dependency_projection(&self.root)
        }
    }

    impl Drop for PackageFixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn projects_path_and_git_requests_in_authored_order() {
        let fixture = PackageFixture::with_source(
            r#"
            machine build(builder: &mut Build, filesystem: &mut Filesystem) {
                builder.application("dependency-projection-probe");
                builder.depend(Source::Path { location: "../local" });
                builder.depend_as("arithmetic_kernels", Source::Git {
                    revision: "0123456789abcdef",
                    repository: "ssh://git@github.com/CathedralOS/example.git"
                });
            }
            "#,
        );
        let projection = extract_build_dependency_projection(&fixture.root).unwrap();
        assert!(matches!(
            projection.declaration(),
            BuildDeclaration::Application(application)
                if application.name.as_str() == "dependency-projection-probe"
        ));
        assert_eq!(
            projection.dependencies(),
            vec![
                DependencySourceRequest::Path {
                    explicit_alias: None,
                    location: "../local".to_owned(),
                },
                DependencySourceRequest::Git {
                    explicit_alias: Some(AliasName::parse("arithmetic_kernels").unwrap()),
                    repository: "ssh://git@github.com/CathedralOS/example.git".to_owned(),
                    revision: "0123456789abcdef".to_owned(),
                },
            ]
        );
    }

    #[test]
    fn resolves_default_alias_from_the_dependency_declaration() {
        let declared_name = PackageName::parse("arithmetic-kernels").unwrap();
        let ordinary = DependencySourceRequest::Git {
            explicit_alias: None,
            repository: "https://github.com/CathedralOS/arithmetic-kernels.git".to_owned(),
            revision: "main".to_owned(),
        };
        let renamed = DependencySourceRequest::Git {
            explicit_alias: Some(AliasName::parse("kernels").unwrap()),
            repository: "https://github.com/CathedralOS/arithmetic-kernels.git".to_owned(),
            revision: "main".to_owned(),
        };

        assert_eq!(
            ordinary.resolved_alias(&declared_name).as_str(),
            "arithmetic_kernels"
        );
        assert_eq!(renamed.resolved_alias(&declared_name).as_str(), "kernels");
    }

    #[test]
    fn absent_build_machine_is_not_an_implicit_project_role() {
        let fixture = PackageFixture::with_source("target windows_x64 { }");
        assert!(matches!(
            fixture.extract(),
            Err(DependencyProjectionError::BuildDeclaration(error))
                if matches!(*error, BuildDeclarationError::MissingBuildDeclaration)
        ));
    }

    #[test]
    fn rejects_missing_unreadable_non_utf8_unlexable_and_unparsable_files() {
        let missing = PackageFixture::empty();
        assert!(matches!(
            missing.extract(),
            Err(DependencyProjectionError::MissingBuildFile { .. })
        ));

        let unreadable = PackageFixture::empty();
        fs::create_dir(unreadable.root.join(BUILD_FILE_NAME)).expect("create build directory");
        assert!(matches!(
            unreadable.extract(),
            Err(DependencyProjectionError::ReadBuildFile { .. })
        ));

        let invalid_encoding = PackageFixture::empty();
        fs::write(invalid_encoding.root.join(BUILD_FILE_NAME), [0xff]).expect("write bad UTF-8");
        assert!(matches!(
            invalid_encoding.extract(),
            Err(DependencyProjectionError::InvalidBuildFileEncoding { .. })
        ));

        let unlexable = PackageFixture::with_source("machine build(builder: &mut Build) { ` }");
        assert!(matches!(
            unlexable.extract(),
            Err(DependencyProjectionError::Lex { .. })
        ));
        let unparsable = PackageFixture::with_source("machine build(builder: &mut Build) {");
        assert!(matches!(
            unparsable.extract(),
            Err(DependencyProjectionError::Parse { .. })
        ));
    }

    #[test]
    fn rejects_duplicate_and_scoped_build_machines() {
        let duplicate = PackageFixture::with_source(
            "machine build(builder: &mut Build) {} machine build(builder: &mut Build) {}",
        );
        assert!(matches!(
            duplicate.extract(),
            Err(DependencyProjectionError::DuplicateBuildMachines { count: 2 })
        ));

        let scoped = PackageFixture::with_source("machine Owner::build(builder: &mut Build) {}");
        assert!(matches!(
            scoped.extract(),
            Err(DependencyProjectionError::ScopedBuildMachine { .. })
        ));
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
                Err(DependencyProjectionError::InvalidBuildParameter)
            ));
        }
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
                Err(DependencyProjectionError::InvalidBuildMachine)
            ));
        }
    }

    #[test]
    fn rejects_authored_dependency_vocabulary() {
        for source in [
            "data Build {} machine build(builder: &mut Build) {}",
            "data Source {} machine build(builder: &mut Build) {}",
            "domain u64::Source; machine build(builder: &mut Build) {}",
            "trait Build {} machine build(builder: &mut Build) {}",
            "machine Build::depend(source: Source) {} machine build(builder: &mut Build) {}",
            "machine Build::depend_as(alias: &[u8], source: Source) {} machine build(builder: &mut Build) {}",
        ] {
            let fixture = PackageFixture::with_source(source);
            let result = fixture.extract();
            assert!(
                matches!(
                    result,
                    Err(DependencyProjectionError::AuthoredToolchainVocabulary { .. })
                ),
                "unexpected projection result for {source:?}: {result:?}"
            );
        }
    }

    #[test]
    fn rejects_wrong_receiver_and_dependency_argument_shapes() {
        let cases = [
            (
                r#"machine build(builder: &mut Build) { other.depend(Source::Path { location: "x" }); }"#,
                "receiver",
            ),
            (
                r#"machine build(builder: &mut Build) { builder.depend("alias", Source::Path { location: "x" }); }"#,
                "arguments",
            ),
            (
                r#"machine build(builder: &mut Build) { builder.depend_as(Source::Path { location: "x" }); }"#,
                "arguments",
            ),
            (
                r#"machine build(builder: &mut Build) { builder.depend_as("alias", Source::Path { location: "x" }, "extra"); }"#,
                "arguments",
            ),
        ];
        for (source, expected) in cases {
            let fixture = PackageFixture::with_source(source);
            let error = fixture.extract().unwrap_err();
            assert!(
                matches!(
                    (&error, expected),
                    (
                        DependencyProjectionError::WrongDependencyReceiver,
                        "receiver"
                    ) | (
                        DependencyProjectionError::WrongDependencyArguments,
                        "arguments"
                    )
                ),
                "unexpected error: {error:?}"
            );
        }
    }

    #[test]
    fn rejects_nonliteral_non_utf8_and_invalid_explicit_aliases() {
        let nonliteral = PackageFixture::with_source(
            r#"machine build(builder: &mut Build) { builder.depend_as(alias, Source::Path { location: "x" }); }"#,
        );
        assert_eq!(
            nonliteral.extract().unwrap_err(),
            DependencyProjectionError::AliasNotString
        );

        let non_utf8 = PackageFixture::with_source(
            r#"machine build(builder: &mut Build) { builder.depend_as("\xff", Source::Path { location: "x" }); }"#,
        );
        assert_eq!(
            non_utf8.extract().unwrap_err(),
            DependencyProjectionError::AliasNotUtf8
        );

        for alias in ["BadAlias", "bad-alias", "_bad", "bad__alias", "bad_"] {
            let fixture = PackageFixture::with_source(&format!(
                r#"machine build(builder: &mut Build) {{ builder.depend_as("{alias}", Source::Path {{ location: "x" }}); }}"#,
            ));
            assert_eq!(
                fixture.extract().unwrap_err(),
                DependencyProjectionError::InvalidAlias {
                    alias: alias.to_owned()
                }
            );
        }
    }

    #[test]
    fn rejects_nonliteral_wrong_type_missing_and_unknown_source_cases() {
        let cases = [
            ("source", DependencyProjectionError::SourceNotLiteral),
            (
                r#"Other::Path { location: "x" }"#,
                DependencyProjectionError::WrongSourceType,
            ),
            (
                r#"Source { location: "x" }"#,
                DependencyProjectionError::MissingSourceCase,
            ),
            (
                r#"Source::Archive { location: "x" }"#,
                DependencyProjectionError::UnsupportedSourceCase {
                    case_name: "Archive".to_owned(),
                },
            ),
        ];
        for (source, expected) in cases {
            let fixture = PackageFixture::with_source(&format!(
                "machine build(builder: &mut Build) {{ builder.depend({source}); }}"
            ));
            assert_eq!(fixture.extract().unwrap_err(), expected);
        }
    }

    #[test]
    fn rejects_missing_extra_duplicate_and_nonliteral_source_fields() {
        for source in [
            "Source::Path {}",
            r#"Source::Path { path: "x" }"#,
            r#"Source::Path { location: "x", extra: "y" }"#,
            r#"Source::Path { location: "x", location: "y" }"#,
            r#"Source::Git { repository: "x" }"#,
            r#"Source::Git { repository: "x", revision: "y", revision: "z" }"#,
        ] {
            let fixture = PackageFixture::with_source(&format!(
                "machine build(builder: &mut Build) {{ builder.depend({source}); }}"
            ));
            assert!(matches!(
                fixture.extract(),
                Err(DependencyProjectionError::WrongSourceFields { .. })
            ));
        }

        let nonliteral = PackageFixture::with_source(
            "machine build(builder: &mut Build) { builder.depend(Source::Path { location: path }); }",
        );
        assert!(matches!(
            nonliteral.extract(),
            Err(DependencyProjectionError::SourceFieldNotString { .. })
        ));
        let non_utf8 = PackageFixture::with_source(
            r#"machine build(builder: &mut Build) { builder.depend(Source::Path { location: "\xff" }); }"#,
        );
        assert!(matches!(
            non_utf8.extract(),
            Err(DependencyProjectionError::SourceFieldNotUtf8 { .. })
        ));
    }

    #[test]
    fn rejects_nested_helper_and_control_flow_dependency_requests() {
        let helper = PackageFixture::with_source(
            r#"
            machine add_dependency(builder: &mut Build) {
                builder.depend_as("hidden_alias", Source::Path { location: "hidden" });
            }
            machine build(builder: &mut Build) { add_dependency(builder); }
            "#,
        );
        assert!(matches!(
            helper.extract(),
            Err(DependencyProjectionError::UnsupportedDependencyShape)
        ));

        let nested_state = PackageFixture::with_source(
            r#"
            machine build(builder: &mut Build) {
                state later(builder: &mut Build) {
                    builder.depend(Source::Path { location: "conditional" });
                }
            }
            "#,
        );
        assert!(matches!(
            nested_state.extract(),
            Err(DependencyProjectionError::UnsupportedDependencyShape)
        ));
    }

    #[test]
    fn rejects_dependency_syntax_without_an_authoritative_build_machine() {
        let fixture = PackageFixture::with_source(
            r#"
            machine helper(builder: &mut Build) {
                builder.depend(Source::Path { location: "hidden" });
            }
            "#,
        );
        assert!(matches!(
            fixture.extract(),
            Err(DependencyProjectionError::UnsupportedDependencyShape)
        ));
    }
}
