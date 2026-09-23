//! The fixture package-inputs harness shared by this crate's test targets.
//!
//! Every target here that compiles a repository fixture, a sample or an
//! authored scratch project reads its package mode from this file, so one
//! projection decides what a fixture depends on and the bundled standard
//! library's location is spelled exactly once:
//!
//! - [`bundled_standard_library_root`] is that one spelling, and
//!   [`bundled_standard_library_dependency_declaration`] is the
//!   `builder.depend` row a synthesized `build.omg` authors to take it;
//! - [`declares_bundled_standard_library`] decides package mode, by projecting
//!   the dependency rows a fixture authored in its own `build.omg` through the
//!   build vocabulary that owns them and resolving each authored location
//!   against the declaring project;
//! - [`standard_library_package_inputs`] binds the compilation root at one
//!   fixture marker and that library at another, under its ordinary
//!   `omega_language_std` alias, reading the root's role and declared name
//!   from its own build declaration instead of restating them;
//! - [`scratch_package_inputs`] binds an authored scratch project that depends
//!   on no published package: its root, and the sibling directories it imports.
//!
//! Fixture identities are markers, not digests of a fixture's content
//! ([`fixture_package_identity`]); each target keeps the markers it already
//! named, and only the standard library's marker has to be nameable by the
//! acceptance below.
//!
//! Acceptance here is this repository's test policy, not evidence that an
//! audit occurred and not production accepted-lock recovery. The entry and
//! console rows are derived from, and replayed against, the exact preliminary
//! checked graph, and their derivations are the compiler test tree's, included
//! rather than copied so both trees admit the same rows.
//!
//! What this tree deliberately does not take from that harness: the transitive
//! `fixture_path_dependencies` closure walk and its scratch-copy writers (no
//! target here compiles a fixture whose dependencies depend on further
//! packages, or relocates a fixture's source), the `TargetProfile`
//! entry-acceptance match (only the Linux x86-64 entry is bound here), and the
//! dangerous-service projection (no target here admits a filesystem or
//! process-exit row).

// Shared across test targets through `#[path]`; each target uses the subset
// its fixtures need.
#![allow(dead_code)]

use build_declarations::{
    BuildDeclaration, BuildDeclarationKind, extract_build_declaration, project_build_entry_syntax,
    project_dependency_rows,
};
use package_compilation::{
    AcceptedSemanticBinding, PackageCompilationInputs, PackageDependencyBinding,
    PackageSourceBinding,
};
use semantic_vocabulary::PackageKeyIdentity;
use source_files_to_tokens::Lexer;
use std::fs;
use std::path::{Path, PathBuf};
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{ExpressionHandle, ExpressionNode};
use tokens_to_syntax_trees::parse_syntax_trees;

// The compiler test target owns the exact provider and entry acceptance these
// harnesses must replay; including those modules keeps both test trees on one
// derivation.
#[path = "../../../../omega-rust/omega/compiler/compiler/tests/support/console_acceptance.rs"]
pub mod console_acceptance;
#[path = "../../../../omega-rust/omega/compiler/compiler/tests/support/linux_entry_acceptance.rs"]
pub mod linux_entry_acceptance;

/// The repository checkout this crate tree lives in, canonicalized so every
/// fixture path and every authored `build.omg` names one absolute location.
pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("native differential tests live under tests/native-differential")
        .canonicalize()
        .expect("the repository checkout resolves")
}

/// A fixture package identity, distinguished only by its marker byte.
pub fn fixture_package_identity(marker: u8) -> PackageKeyIdentity {
    PackageKeyIdentity::from_digest([marker; 32]).expect("nonzero package identity")
}

/// The bundled standard library's root.
///
/// Package mode is never decided from this path by spelling: it is named here
/// so a fixture that authored a dependency row can be recognized by where that
/// row resolves, and so an authored scratch project that must bind the library
/// without declaring it names the same root.
pub fn bundled_standard_library_root() -> PathBuf {
    repo_root().join("source/library/std")
}

/// The bundled standard library's root as a `Source::Path` location literal:
/// absolute, with forward slashes on every host.
pub fn bundled_standard_library_location() -> String {
    bundled_standard_library_root()
        .to_string_lossy()
        .replace('\\', "/")
}

/// The `builder.depend` row a synthesized `build.omg` authors to take the
/// bundled standard library as an ordinary product dependency, indented as a
/// build-entry statement and newline-terminated.
pub fn bundled_standard_library_dependency_declaration() -> String {
    format!(
        "    builder.depend(Source::Path {{ location: \"{}\" }});\n",
        bundled_standard_library_location()
    )
}

/// The reader's entry to fixture package mode: whether a fixture authored a
/// dependency on the bundled standard library in its own `build.omg`.
///
/// A repository fixture compiles in package mode when — and only when — its
/// `build.omg` authors an unconditional `builder.depend`, `depend_as`,
/// `build_depend` or `build_depend_as` row whose `Source::Path` location
/// resolves to [`bundled_standard_library_root`]. The row is read through the
/// build vocabulary that owns it, so a fixture reaching the library through a
/// different number of `..` steps or under an authored alias is recognized and
/// one that merely mentions the path in a comment is not. A project with no
/// `build.omg`, or one whose build declaration the compiler itself refuses,
/// authors no row here and compiles without package inputs.
pub fn declares_bundled_standard_library(project_root: &Path) -> bool {
    let standard_library_root = fs::canonicalize(bundled_standard_library_root())
        .expect("bundled standard library root resolves");
    authored_dependency_locations(project_root)
        .iter()
        .any(|location| location == &standard_library_root)
}

/// The resolved root of every dependency a fixture authored in its own
/// `build.omg`, canonicalized, in authored order.
fn authored_dependency_locations(project_root: &Path) -> Vec<PathBuf> {
    let Ok(source) = fs::read_to_string(project_root.join("build.omg")) else {
        return Vec::new();
    };
    let Ok(tokens) = Lexer::new(&source).tokenize() else {
        return Vec::new();
    };
    let Ok(trees) = parse_syntax_trees(&tokens) else {
        return Vec::new();
    };
    let Ok(build) = project_build_entry_syntax(&trees) else {
        return Vec::new();
    };
    let Ok(rows) = project_dependency_rows(&trees, &build) else {
        return Vec::new();
    };
    rows.iter()
        .map(|row| authored_dependency_location(&trees, project_root, row.source()))
        .collect()
}

/// Read one authored `Source::Path { location }` and resolve it against the
/// declaring project. Every other source kind is a fixture this tree cannot
/// acquire, so it fails loudly instead of compiling without the edge.
fn authored_dependency_location(
    trees: &SyntaxTrees,
    project_root: &Path,
    source: ExpressionHandle,
) -> PathBuf {
    let ExpressionNode::StructLiteral(literal) = trees.expressions.expression(source) else {
        panic!(
            "fixture {} declares an unsupported dependency source: this harness wires only a direct `Source::Path` literal",
            project_root.display()
        )
    };
    let constructor = literal.constructor_name.as_str();
    let (type_name, case_name) = constructor
        .rsplit_once("::")
        .map_or((constructor, None), |(owner, case)| (owner, Some(case)));
    assert!(
        type_name == "Source" && case_name == Some("Path"),
        "fixture {} declares an unsupported dependency source `{constructor}`: this harness wires only `Source::Path`",
        project_root.display()
    );
    let [field] = trees.expressions.struct_fields(literal.fields) else {
        panic!(
            "fixture {}: `Source::Path` takes exactly a `location` field",
            project_root.display()
        )
    };
    assert_eq!(
        field.name.as_str(),
        "location",
        "fixture {}: `Source::Path` takes exactly a `location` field",
        project_root.display()
    );
    let ExpressionNode::String(bytes) = trees.expressions.expression(field.value) else {
        panic!(
            "fixture {}: dependency arguments must be direct string literals",
            project_root.display()
        )
    };
    let authored = std::str::from_utf8(bytes).unwrap_or_else(|_| {
        panic!(
            "fixture {}: dependency literal is not utf8",
            project_root.display()
        )
    });
    fs::canonicalize(project_root.join(authored)).unwrap_or_else(|error| {
        panic!(
            "fixture {} declares dependency location `{authored}`, which does not resolve: {error}",
            project_root.display()
        )
    })
}

/// The role and declared name a project's own `build.omg` names for itself.
fn root_declaration(project_root: &Path) -> (BuildDeclarationKind, String) {
    let declaration = extract_build_declaration(project_root)
        .unwrap_or_else(|error| panic!("project {}: {error}", project_root.display()));
    let root_role = declaration.kind();
    let root_name = match declaration {
        BuildDeclaration::Application(application) => application.name,
        BuildDeclaration::Package(package) => package.name,
        BuildDeclaration::Workspace(_) => {
            panic!(
                "project {} cannot be a workspace root",
                project_root.display()
            )
        }
    };
    (root_role, root_name.into_string())
}

/// Package inputs binding `project_root` as the compilation root, under the
/// identity `root_marker` names, and the bundled standard library under the
/// identity `standard_library_marker` names with its ordinary
/// `omega_language_std` alias.
///
/// The reconciled graph is what `use omega_language_std::console` resolves
/// against; the unmanaged route would instead look for an
/// `omega_language_std/` module below the project root. The root's role and
/// declared name come from its own build declaration, so no caller restates
/// them.
pub fn standard_library_package_inputs(
    project_root: &Path,
    root_marker: u8,
    standard_library_marker: u8,
) -> PackageCompilationInputs {
    let (root_role, root_name) = root_declaration(project_root);
    let root_identity = fixture_package_identity(root_marker);
    let standard_library_identity = fixture_package_identity(standard_library_marker);
    PackageCompilationInputs::new(
        root_identity,
        root_role,
        vec![
            PackageSourceBinding::new(root_identity, root_name, project_root.to_path_buf()),
            PackageSourceBinding::new(
                standard_library_identity,
                "omega_language_std",
                bundled_standard_library_root(),
            ),
        ],
        vec![PackageDependencyBinding::new(
            root_identity,
            "omega_language_std",
            standard_library_identity,
        )],
    )
    .unwrap_or_else(|errors| panic!("project {}: {errors:#?}", project_root.display()))
}

/// Package inputs for an authored scratch project that depends on no published
/// package: `packages` names the compilation root first, then every sibling
/// directory it imports, each as `(marker, declared name, root)`. Every
/// sibling is bound as a product dependency of the compilation root under its
/// own declared name, which is the alias the root's `use` names.
pub fn scratch_package_inputs(packages: &[(u8, &str, &Path)]) -> PackageCompilationInputs {
    let [(root_marker, _, root_path), siblings @ ..] = packages else {
        panic!("a scratch project binds at least its compilation root")
    };
    let root_identity = fixture_package_identity(*root_marker);
    PackageCompilationInputs::new_package(
        root_identity,
        packages
            .iter()
            .map(|(marker, name, path)| {
                PackageSourceBinding::new(
                    fixture_package_identity(*marker),
                    *name,
                    path.to_path_buf(),
                )
            })
            .collect(),
        siblings
            .iter()
            .map(|(marker, name, _)| {
                PackageDependencyBinding::new(
                    root_identity,
                    *name,
                    fixture_package_identity(*marker),
                )
            })
            .collect(),
    )
    .unwrap_or_else(|errors| panic!("scratch project {}: {errors:#?}", root_path.display()))
}

/// The reviewed Linux x86-64 `ProgramEntry` candidate the bundled standard
/// library owns, under the identity `marker` names.
///
/// The program-entry root must be accepted before a build machine's
/// `roots.bind` evaluates during checked compilation. The target contract is
/// checked as dependency source first: application discovery alone cannot
/// authorize that role.
pub fn linux_x86_64_entry_binding(marker: u8) -> AcceptedSemanticBinding {
    linux_entry_acceptance::candidate_linux_x86_64_entry_binding(
        &bundled_standard_library_root(),
        fixture_package_identity(marker),
    )
    .expect("exact Linux x86-64 program entry binding")
}
