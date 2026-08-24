use crate::identity::PackageName;
use psi_source_files_to_tokens::Lexer;
use psi_syntax_trees::SyntaxTrees;
use psi_syntax_trees::expression::ExpressionNode;
use psi_syntax_trees::item::Item;
use psi_syntax_trees::types::TypeReferenceNode;
use psi_tokens_to_syntax_trees::parse_syntax_trees;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

const BUILD_FILE_NAME: &str = "build.omg";
const PACKAGE_CONST_NAME: &str = "PACKAGE";
const PACKAGE_TYPE_NAME: &str = "Package";
const PACKAGE_NAME_FIELD: &str = "name";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageDeclaration {
    pub name: PackageName,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageDeclarationError {
    MissingBuildFile { path: PathBuf },
    ReadBuildFile { path: PathBuf, message: String },
    InvalidBuildFileEncoding { path: PathBuf },
    Lex { message: String },
    Parse { message: String },
    AuthoredPackageType,
    MissingPackageDeclaration,
    DuplicatePackageDeclarations { count: usize },
    ScopedPackageDeclaration { scope: String },
    WrongDeclarationType,
    InitializerNotLiteral,
    WrongLiteralType,
    CaseLiteral,
    WrongLiteralFields,
    NameNotStringLiteral,
    NameNotUtf8,
    InvalidPackageName { message: String },
}

impl fmt::Display for PackageDeclarationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingBuildFile { path } => {
                write!(
                    formatter,
                    "package build file is missing: {}",
                    path.display()
                )
            }
            Self::ReadBuildFile { path, message } => {
                write!(formatter, "cannot read {}: {message}", path.display())
            }
            Self::InvalidBuildFileEncoding { path } => {
                write!(formatter, "{} is not UTF-8 Omega source", path.display())
            }
            Self::Lex { message } => write!(formatter, "cannot lex package build: {message}"),
            Self::Parse { message } => write!(formatter, "cannot parse package build: {message}"),
            Self::AuthoredPackageType => {
                formatter.write_str("package build must not declare toolchain type `Package`")
            }
            Self::MissingPackageDeclaration => formatter.write_str(
                "package build must declare `const PACKAGE: Package = Package { name: \"...\" };`",
            ),
            Self::DuplicatePackageDeclarations { count } => {
                write!(formatter, "package build declares `PACKAGE` {count} times")
            }
            Self::ScopedPackageDeclaration { scope } => {
                write!(
                    formatter,
                    "package declaration must be free, not `{scope}::PACKAGE`"
                )
            }
            Self::WrongDeclarationType => formatter.write_str("`PACKAGE` must have type `Package`"),
            Self::InitializerNotLiteral => {
                formatter.write_str("`PACKAGE` must use a direct `Package` literal")
            }
            Self::WrongLiteralType => {
                formatter.write_str("`PACKAGE` initializer must construct `Package`")
            }
            Self::CaseLiteral => {
                formatter.write_str("`PACKAGE` initializer must be a record literal, not a case")
            }
            Self::WrongLiteralFields => {
                formatter.write_str("`PACKAGE` literal must contain exactly the field `name`")
            }
            Self::NameNotStringLiteral => {
                formatter.write_str("`PACKAGE.name` must be a direct string literal")
            }
            Self::NameNotUtf8 => formatter.write_str("`PACKAGE.name` must contain UTF-8 bytes"),
            Self::InvalidPackageName { message } => formatter.write_str(message),
        }
    }
}

impl std::error::Error for PackageDeclarationError {}

/// Extract the package-authored human name from the one root `build.omg`.
///
/// This is intentionally syntax-tree validation, not build evaluation. It
/// reads no imports, dependencies, generated files, or build-host services.
pub fn extract_package_declaration(
    package_root: impl AsRef<Path>,
) -> Result<PackageDeclaration, PackageDeclarationError> {
    let build_path = package_root.as_ref().join(BUILD_FILE_NAME);
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
    extract_from_source(source)
}

fn extract_from_source(source: &str) -> Result<PackageDeclaration, PackageDeclarationError> {
    let tokens = Lexer::new(source)
        .tokenize()
        .map_err(|error| PackageDeclarationError::Lex {
            message: error.message,
        })?;
    let syntax_trees =
        parse_syntax_trees(&tokens).map_err(|error| PackageDeclarationError::Parse {
            message: error.message,
        })?;
    extract_from_syntax_trees(&syntax_trees)
}

fn extract_from_syntax_trees(
    syntax_trees: &SyntaxTrees,
) -> Result<PackageDeclaration, PackageDeclarationError> {
    if syntax_trees
        .root_items()
        .any(|item| matches!(item, Item::Data(data) if data.name.as_str() == PACKAGE_TYPE_NAME))
    {
        return Err(PackageDeclarationError::AuthoredPackageType);
    }

    let declarations = syntax_trees
        .root_items()
        .filter_map(|item| match item {
            Item::Const(declaration) if declaration.name.as_str() == PACKAGE_CONST_NAME => {
                Some(declaration)
            }
            _ => None,
        })
        .collect::<Vec<_>>();

    let [declaration] = declarations.as_slice() else {
        return if declarations.is_empty() {
            Err(PackageDeclarationError::MissingPackageDeclaration)
        } else {
            Err(PackageDeclarationError::DuplicatePackageDeclarations {
                count: declarations.len(),
            })
        };
    };

    if !declaration.scope.as_str().is_empty() {
        return Err(PackageDeclarationError::ScopedPackageDeclaration {
            scope: declaration.scope.as_str().to_owned(),
        });
    }

    if !matches!(
        syntax_trees
            .type_references
            .type_reference(declaration.type_reference),
        TypeReferenceNode::Named(name) if name.as_str() == PACKAGE_TYPE_NAME
    ) {
        return Err(PackageDeclarationError::WrongDeclarationType);
    }

    let ExpressionNode::StructLiteral(literal) =
        syntax_trees.expressions.expression(declaration.value)
    else {
        return Err(PackageDeclarationError::InitializerNotLiteral);
    };
    if literal.type_name.as_str() != PACKAGE_TYPE_NAME {
        return Err(PackageDeclarationError::WrongLiteralType);
    }
    if literal.case_name.is_some() {
        return Err(PackageDeclarationError::CaseLiteral);
    }

    let fields = syntax_trees.expressions.struct_fields(literal.fields);
    let [name_field] = fields else {
        return Err(PackageDeclarationError::WrongLiteralFields);
    };
    if name_field.name.as_str() != PACKAGE_NAME_FIELD {
        return Err(PackageDeclarationError::WrongLiteralFields);
    }

    let ExpressionNode::String(name_bytes) = syntax_trees.expressions.expression(name_field.value)
    else {
        return Err(PackageDeclarationError::NameNotStringLiteral);
    };
    let name = std::str::from_utf8(name_bytes).map_err(|_| PackageDeclarationError::NameNotUtf8)?;
    let name = PackageName::parse(name)
        .map_err(|message| PackageDeclarationError::InvalidPackageName { message })?;

    Ok(PackageDeclaration { name })
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
    }

    impl Drop for PackageFixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn declaration(name: &str) -> String {
        format!(
            r#"
            const PACKAGE: Package = Package {{ name: "{name}" }};
            machine build(builder: &mut Build) {{
            }}
            "#
        )
    }

    #[test]
    fn extracts_canonical_package_declaration_from_root_build() {
        let fixture = PackageFixture::with_source(&declaration("arithmetic-kernels"));
        assert_eq!(
            fixture.extract().unwrap(),
            PackageDeclaration {
                name: PackageName::parse("arithmetic-kernels").unwrap(),
            }
        );
    }

    #[test]
    fn rejects_missing_unreadable_and_non_utf8_build_files() {
        let missing = PackageFixture::empty();
        assert!(matches!(
            missing.extract(),
            Err(PackageDeclarationError::MissingBuildFile { .. })
        ));

        let unreadable = PackageFixture::empty();
        fs::create_dir(unreadable.root.join(BUILD_FILE_NAME)).expect("create directory build.omg");
        assert!(matches!(
            unreadable.extract(),
            Err(PackageDeclarationError::ReadBuildFile { .. })
        ));

        let invalid_encoding = PackageFixture::empty();
        fs::write(invalid_encoding.root.join(BUILD_FILE_NAME), [0xff])
            .expect("write invalid source bytes");
        assert!(matches!(
            invalid_encoding.extract(),
            Err(PackageDeclarationError::InvalidBuildFileEncoding { .. })
        ));
    }

    #[test]
    fn rejects_unlexable_and_unparsable_build_files() {
        let unlexable = PackageFixture::with_source("const PACKAGE: Package = `;");
        assert!(matches!(
            unlexable.extract(),
            Err(PackageDeclarationError::Lex { .. })
        ));

        let unparsable = PackageFixture::with_source("const PACKAGE: Package = Package {");
        assert!(matches!(
            unparsable.extract(),
            Err(PackageDeclarationError::Parse { .. })
        ));
    }

    #[test]
    fn rejects_missing_duplicate_and_scoped_package_declarations() {
        let missing = PackageFixture::with_source("machine build(builder: &mut Build) {}");
        assert!(matches!(
            missing.extract(),
            Err(PackageDeclarationError::MissingPackageDeclaration)
        ));

        let duplicate = PackageFixture::with_source(
            r#"
            const PACKAGE: Package = Package { name: "first-package" };
            const PACKAGE: Package = Package { name: "second-package" };
            "#,
        );
        assert!(matches!(
            duplicate.extract(),
            Err(PackageDeclarationError::DuplicatePackageDeclarations { count: 2 })
        ));

        let scoped = PackageFixture::with_source(
            r#"const Build::PACKAGE: Package = Package { name: "scoped-package" };"#,
        );
        assert!(matches!(
            scoped.extract(),
            Err(PackageDeclarationError::ScopedPackageDeclaration { .. })
        ));
    }

    #[test]
    fn rejects_package_authored_toolchain_vocabulary() {
        let fixture = PackageFixture::with_source(
            r#"
            data Package { name: &[u8]; }
            const PACKAGE: Package = Package { name: "spoofed-package" };
            "#,
        );
        assert!(matches!(
            fixture.extract(),
            Err(PackageDeclarationError::AuthoredPackageType)
        ));
    }

    #[test]
    fn rejects_wrong_declaration_type_and_literal_shape() {
        let wrong_type = PackageFixture::with_source(
            r#"const PACKAGE: package = Package { name: "wrong-type" };"#,
        );
        assert!(matches!(
            wrong_type.extract(),
            Err(PackageDeclarationError::WrongDeclarationType)
        ));

        let nonliteral = PackageFixture::with_source("const PACKAGE: Package = load_package();");
        assert!(matches!(
            nonliteral.extract(),
            Err(PackageDeclarationError::InitializerNotLiteral)
        ));

        let wrong_literal_type = PackageFixture::with_source(
            r#"const PACKAGE: Package = package { name: "wrong-literal" };"#,
        );
        assert!(matches!(
            wrong_literal_type.extract(),
            Err(PackageDeclarationError::WrongLiteralType)
        ));

        let case_literal = PackageFixture::with_source(
            r#"const PACKAGE: Package = Package::Named { name: "case-literal" };"#,
        );
        assert!(matches!(
            case_literal.extract(),
            Err(PackageDeclarationError::CaseLiteral)
        ));
    }

    #[test]
    fn rejects_missing_extra_duplicate_and_misspelled_fields() {
        for source in [
            "const PACKAGE: Package = Package {};",
            r#"const PACKAGE: Package = Package { name: "one", extra: "two" };"#,
            r#"const PACKAGE: Package = Package { name: "one", name: "two" };"#,
            r#"const PACKAGE: Package = Package { Name: "wrong-case" };"#,
        ] {
            let fixture = PackageFixture::with_source(source);
            assert!(matches!(
                fixture.extract(),
                Err(PackageDeclarationError::WrongLiteralFields)
            ));
        }
    }

    #[test]
    fn rejects_nonliteral_effectful_and_dependency_based_names() {
        for source in [
            "const PACKAGE: Package = Package { name: dependency_name };",
            "const PACKAGE: Package = Package { name: read_name() };",
        ] {
            let fixture = PackageFixture::with_source(source);
            assert!(matches!(
                fixture.extract(),
                Err(PackageDeclarationError::NameNotStringLiteral)
            ));
        }
    }

    #[test]
    fn rejects_non_utf8_literal_bytes_and_noncanonical_names() {
        let non_utf8 = PackageFixture::with_source(
            r#"const PACKAGE: Package = Package { name: "\x80package" };"#,
        );
        assert!(matches!(
            non_utf8.extract(),
            Err(PackageDeclarationError::NameNotUtf8)
        ));

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
    fn declaration_name_is_case_sensitive() {
        let fixture = PackageFixture::with_source(
            r#"const package: Package = Package { name: "wrong-constant-case" };"#,
        );
        assert!(matches!(
            fixture.extract(),
            Err(PackageDeclarationError::MissingPackageDeclaration)
        ));
    }
}
