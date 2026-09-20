//! A root whose build selects a dependency's provider with
//! `CompositionMode::Independent` settles only when that dependency was
//! compiled as its own component and published a description this build can
//! verify.
//!
//! Every fixture drives the real compiler entrance twice: once over the
//! dependency package alone, which is what "compiled as its own component"
//! means here, and once over the consuming root with the published
//! description attached to its target inputs. Nothing hand-authors a
//! description, a subject, or a realization inventory — the producer under
//! test publishes them from the dependency's own checked compilation, and
//! the build re-verifies them under its own admission profile.

use super::{TempTree, identity};
use compiler::{
    CheckedCompileRequest, CompileOptions, CompileRequest, RequestedCompileProduct, compile,
    compile_to_checked, published_independent_component_description,
};
use package_compilation::{
    BuildDeclarationKind, IndependentComponentDescription, PackageCompilationInputError,
    PackageCompilationInputs, PackageDependencyBinding, PackageSourceBinding,
};
use semantic_vocabulary::PackageKeyIdentity;
use std::path::{Path, PathBuf};
use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

/// The dependency's public provider closure. Two provider types satisfy the
/// same boundary requirement: the component's own build seals the requirement
/// it calls with the vtable provider, and the checked adapter beside it is
/// the realization a consumer may deploy independently. The component's entry
/// must call the requirement — that call is what puts the checked candidate
/// in the module's realization roster, so the published export roster is
/// module-derived rather than asserted.
const COMPONENT_SOURCE: &str = r#"use omega::language::core::service;
pub boundary trait Pick {
    machine mark(value: i32);
}

pub data VtablePick { mark: addr; }
pub machine VtablePick::mark(value: i32)
satisfies Pick::mark
via Binding::VtableField(mark);

pub data PickProvider { }
pub machine PickProvider::mark_adapter(value: i32) satisfies Pick::mark { }

pub data ComponentEntry { pick: Service<Pick>; }
pub machine ComponentEntry::main(&mut self) reaches Pick invokes Pick; {
    self.pick.mark(7);
}
"#;

/// The same closure after the adapter machine is renamed. A description
/// published before this edit still verifies — it is a complete, honest
/// description of the earlier component — but it exports the earlier
/// realization coordinate, so it can no longer realize the consumer's
/// selected plan.
const RENAMED_ADAPTER_COMPONENT_SOURCE: &str = r#"use omega::language::core::service;
pub boundary trait Pick {
    machine mark(value: i32);
}

pub data VtablePick { mark: addr; }
pub machine VtablePick::mark(value: i32)
satisfies Pick::mark
via Binding::VtableField(mark);

pub data PickProvider { }
pub machine PickProvider::mark_adapter_v2(value: i32) satisfies Pick::mark { }

pub data ComponentEntry { pick: Service<Pick>; }
pub machine ComponentEntry::main(&mut self) reaches Pick invokes Pick; {
    self.pick.mark(7);
}
"#;

/// The same provider component, but its entry transitively calls an
/// unresolved `reaches <=` requirement. The component still compiles and
/// publishes a verifiable description — the retained row arrives as a
/// service bound installation still owes, beside the concrete reach's
/// ceiling rows — but an `Independent` join must reject it rather than
/// treat the bound as resolved reach. The call sits in an unattached
/// helper: a boundary call inside `ComponentEntry::main` itself would
/// stop body construction at the provider-attachment gate before any row
/// is retained, and a direct `pub boundary requirement` call would
/// demand a selected provider at validation.
const BOUNDED_COMPONENT_SOURCE: &str = r#"use omega::language::core::service;
pub boundary trait Pick {
    machine mark(value: i32);
}

pub boundary trait Console {}
pub boundary trait Installer { machine install() reaches <= Console; }

pub data VtablePick { mark: addr; }
pub machine VtablePick::mark(value: i32)
satisfies Pick::mark
via Binding::VtableField(mark);

pub data PickProvider { }
pub machine PickProvider::mark_adapter(value: i32) satisfies Pick::mark { }

pub data Helper { }
pub machine Helper::run()
reaches Installer + Console
invokes Installer;
{
    Installer::install();
}

pub data ComponentEntry { pick: Service<Pick>; }
pub machine ComponentEntry::main(&mut self)
reaches Pick + Installer + Console
invokes Pick;
invokes Installer;
{
    self.pick.mark(7);
    Helper::run();
}
"#;

/// An unrelated component: it declares, seals, and realizes a requirement of
/// its own. Its description is complete and verifiable, and it realizes
/// nothing the consuming root selected.
const FOREIGN_COMPONENT_SOURCE: &str = r#"use omega::language::core::service;
pub boundary trait Other {
    machine mark(value: i32);
}

pub data VtableOther { mark: addr; }
pub machine VtableOther::mark(value: i32)
satisfies Other::mark
via Binding::VtableField(mark);

pub data OtherProvider { }
pub machine OtherProvider::mark_adapter(value: i32) satisfies Other::mark { }

pub data ComponentEntry { other: Service<Other>; }
pub machine ComponentEntry::main(&mut self) reaches Other invokes Other; {
    self.other.mark(7);
}
"#;

fn write_component_package(
    directory: &Path,
    package_name: &str,
    target_name: &str,
    requirement: &str,
    sealing_provider: &str,
    source: &str,
) {
    write_component_package_with(
        directory,
        package_name,
        target_name,
        requirement,
        sealing_provider,
        source,
        "",
    );
}

fn write_component_package_with(
    directory: &Path,
    package_name: &str,
    target_name: &str,
    requirement: &str,
    sealing_provider: &str,
    source: &str,
    extra_build: &str,
) {
    TempTree::write(directory.join("pick.omg"), source);
    TempTree::write(
        directory.join("build.omg"),
        &format!(
            r#"machine build(builder: &mut Build) {{
    builder.package("{package_name}");
    builder.select_provider<{requirement}, {sealing_provider}>(CompositionMode::Fused);
{extra_build}    builder.roots.bind({target_name}::ProgramEntry, ComponentEntry::main);
}}
"#
        ),
    );
}

/// Compile one dependency package as its own component and publish the
/// canonical description of the result through the compiler's producer.
fn published_component(
    package: PackageKeyIdentity,
    directory: &Path,
    target_name: &str,
) -> IndependentComponentDescription {
    let inputs = PackageCompilationInputs::new_package(
        package,
        vec![PackageSourceBinding::new(
            package,
            "component",
            directory.to_path_buf(),
        )],
        Vec::new(),
    )
    .expect("one-package dependency component graph");
    let checked = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs),
        ..CheckedCompileRequest::new(&directory.join("pick.omg"), Some(target_name))
    })
    .expect("the dependency compiles as its own component");
    published_independent_component_description(
        directory.join("pick.omg"),
        checked,
        proof_admission::AdmissionProfile::default(),
    )
    .expect("the dependency component publishes one canonical description")
}

fn write_consuming_root(directory: &Path, target_name: &str) {
    write_consuming_root_with(directory, target_name, "");
}

fn write_consuming_root_with(directory: &Path, target_name: &str, extra_build: &str) {
    write_consuming_root_selecting(directory, target_name, "Independent", extra_build);
}

fn write_consuming_root_fused(directory: &Path, target_name: &str) {
    write_consuming_root_selecting(directory, target_name, "Fused", "");
}

fn write_consuming_root_selecting(
    directory: &Path,
    target_name: &str,
    composition_mode: &str,
    extra_build: &str,
) {
    TempTree::write(
        directory.join("main.omg"),
        "use dep::pick;\n\ndata Main { }\nmachine Main::main(&mut self) { }\n",
    );
    TempTree::write(
        directory.join("build.omg"),
        &format!(
            r#"machine build(builder: &mut Build) {{
    builder.application("independent-consumer");
    builder.depend_as("dep", Source::Path {{ location: "../pick-component" }});
    builder.select_provider<Pick, PickProvider>(CompositionMode::{composition_mode});
{extra_build}    builder.roots.bind({target_name}::ProgramEntry, Main::main);
}}
"#
        ),
    );
}

const ROOT_PACKAGE_MARKER: u8 = 1;
const DEPENDENCY_PACKAGE_MARKER: u8 = 2;

struct IndependentFixture {
    _tree: TempTree,
    root: PathBuf,
    dependency: PathBuf,
    target_name: &'static str,
}

impl IndependentFixture {
    fn new(target_name: &'static str) -> Self {
        let tree = TempTree::new();
        let root = tree.package("consumer");
        let dependency = tree.package("pick-component");
        write_component_package(
            &dependency,
            "pick-component",
            target_name,
            "Pick",
            "PickProvider",
            COMPONENT_SOURCE,
        );
        write_consuming_root(&root, target_name);
        Self {
            _tree: tree,
            root,
            dependency,
            target_name,
        }
    }

    fn inputs(&self) -> PackageCompilationInputs {
        PackageCompilationInputs::new(
            identity(ROOT_PACKAGE_MARKER),
            BuildDeclarationKind::Application,
            vec![
                PackageSourceBinding::new(
                    identity(ROOT_PACKAGE_MARKER),
                    "consumer",
                    self.root.clone(),
                ),
                PackageSourceBinding::new(
                    identity(DEPENDENCY_PACKAGE_MARKER),
                    "component",
                    self.dependency.clone(),
                ),
            ],
            vec![PackageDependencyBinding::new(
                identity(ROOT_PACKAGE_MARKER),
                "dep",
                identity(DEPENDENCY_PACKAGE_MARKER),
            )],
        )
        .expect("two-package independent consumer graph")
    }

    fn published(&self) -> IndependentComponentDescription {
        published_component(
            identity(DEPENDENCY_PACKAGE_MARKER),
            &self.dependency,
            self.target_name,
        )
    }

    /// Publish the description of an unrelated component under this
    /// dependency's package identity: a substituted publication the
    /// consuming build must reject on its own evidence.
    fn published_foreign_component(&self) -> IndependentComponentDescription {
        let directory = self.root.parent().expect("fixture tree").join("other");
        std::fs::create_dir(&directory).expect("create unrelated component directory");
        write_component_package(
            &directory,
            "other-component",
            self.target_name,
            "Other",
            "VtableOther",
            FOREIGN_COMPONENT_SOURCE,
        );
        published_component(
            identity(DEPENDENCY_PACKAGE_MARKER),
            &directory,
            self.target_name,
        )
    }

    fn rewrite_dependency(&self, source: &str) {
        self.rewrite_dependency_with(source, "");
    }

    fn rewrite_dependency_with(&self, source: &str, extra_build: &str) {
        write_component_package_with(
            &self.dependency,
            "pick-component",
            self.target_name,
            "Pick",
            "PickProvider",
            source,
            extra_build,
        );
    }

    fn attach(
        &self,
        descriptions: Vec<IndependentComponentDescription>,
    ) -> PackageCompilationInputs {
        self.inputs()
            .with_independent_component_descriptions(descriptions)
            .expect("descriptions naming this dependency attach to the consuming root")
    }

    fn compile_root(
        &self,
        inputs: PackageCompilationInputs,
    ) -> Result<compiler::CheckedCompilation, Vec<diagnostics::Diagnostic>> {
        compile_to_checked(CheckedCompileRequest {
            package_inputs: Some(inputs),
            ..CheckedCompileRequest::new(&self.root.join("main.omg"), Some(self.target_name))
        })
    }

    /// Drive the consuming root through the real compile entrance to the
    /// requested product, not stopping at the checked boundary.
    fn produce_root(
        &self,
        inputs: PackageCompilationInputs,
        product: RequestedCompileProduct,
    ) -> Result<compiler::CompileReport, Vec<diagnostics::Diagnostic>> {
        compile(
            CompileRequest::new(CompileOptions {
                root_path: self.root.join("main.omg"),
                build_dir: Some(self._tree.0.join(format!("{product:?}-output"))),
                target_name: Some(self.target_name.to_owned()),
            })
            .with_package_inputs(inputs)
            .with_requested_product(product),
        )
        .and_then(compiler::CompileOutcomes::into_single_report)
    }
}

/// A mechanism-bearing component: beside the sealed `Pick` requirement its
/// entry performs an immediate port-space write through checked assembly.
/// The write lowers to a `PortWrite` operation the description binds to a
/// derived `port_mechanism_assumption` digest, so the published description
/// carries an inseparable assumption roster.
const MECHANISM_COMPONENT_SOURCE: &str = r#"use omega::language::core::assembly;
use omega::language::core::service;

pub boundary trait Pick {
    machine mark(value: i32);
}

pub data VtablePick { mark: addr; }
pub machine VtablePick::mark(value: i32)
satisfies Pick::mark
via Binding::VtableField(mark);

pub data PickProvider { }
pub machine PickProvider::mark_adapter(value: i32) satisfies Pick::mark { }

pub machine signal_port()
reaches PortIo
{
    asm where clobbers r10, r11, r15, rax, rdx {
        out 0x3F8, 0x41
    }
}

pub data ComponentEntry { pick: Service<Pick>; }
pub machine ComponentEntry::main(&mut self)
reaches Pick + PortIo
invokes Pick;
{
    self.pick.mark(7);
    signal_port();
}
"#;

fn rejects_with(diagnostics: &[diagnostics::Diagnostic], fragments: &[&str]) {
    assert!(
        diagnostics.iter().any(|diagnostic| fragments
            .iter()
            .all(|fragment| diagnostic.message.contains(fragment))),
        "expected {fragments:?} among: {diagnostics:#?}"
    );
}

#[test]
fn independent_selection_settles_on_the_dependency_component_description() {
    let Some(target_name) = super::host_target_name() else {
        return;
    };
    let fixture = IndependentFixture::new(target_name);
    let published = fixture.published();
    assert_eq!(published.package(), identity(DEPENDENCY_PACKAGE_MARKER));
    assert!(!published.description().is_empty());
    let inputs = fixture.attach(vec![published]);
    fixture
        .compile_root(inputs)
        .expect("an independent selection over a described dependency component settles");
}

#[test]
fn a_settled_independent_edge_rejects_terminal_product_emission() {
    let Some(target_name) = super::host_target_name() else {
        return;
    };
    let fixture = IndependentFixture::new(target_name);
    let inputs = fixture.attach(vec![fixture.published()]);
    fixture
        .compile_root(inputs.clone())
        .expect("the same edge still settles at the checked boundary");
    let diagnostics = fixture
        .produce_root(inputs, RequestedCompileProduct::TerminalArtifact)
        .expect_err("a settled independent edge has no product substrate to emit");
    rejects_with(
        &diagnostics,
        &[
            "settled with an independent composition edge",
            "refusing to emit a fused artifact",
        ],
    );
}

#[test]
fn a_settled_independent_edge_rejects_native_product_emission() {
    let Some(target_name) = super::host_target_name() else {
        return;
    };
    let fixture = IndependentFixture::new(target_name);
    let diagnostics = fixture
        .produce_root(
            fixture.attach(vec![fixture.published()]),
            RequestedCompileProduct::NativeArtifact,
        )
        .expect_err("the direct native route realizes the same silent fused edge");
    rejects_with(
        &diagnostics,
        &[
            "settled with an independent composition edge",
            "refusing to emit a fused artifact",
        ],
    );
}

#[test]
fn independent_selection_without_a_description_reaches_the_component_fence() {
    let Some(target_name) = super::host_target_name() else {
        return;
    };
    let fixture = IndependentFixture::new(target_name);
    let diagnostics = fixture
        .compile_root(fixture.inputs())
        .expect_err("an independent selection never falls through as fused");
    rejects_with(
        &diagnostics,
        &[
            "no verified component description was supplied",
            "refusing to treat the edge as fused",
        ],
    );
}

#[test]
fn substituted_description_bytes_reject_against_the_published_subject() {
    let Some(target_name) = super::host_target_name() else {
        return;
    };
    let fixture = IndependentFixture::new(target_name);
    let published = fixture.published();
    let substituted = IndependentComponentDescription::new(
        published.package(),
        published.expected_subject(),
        fixture.published_foreign_component().description().to_vec(),
    );
    let diagnostics = fixture
        .compile_root(fixture.attach(vec![substituted]))
        .expect_err("another component's bytes cannot answer this dependency's subject");
    rejects_with(
        &diagnostics,
        &[
            "component description attached for dependency package `component`",
            "failed independent verification",
            "cannot deploy that package as an independent component",
        ],
    );
}

#[test]
fn an_unrelated_component_description_realizes_no_selected_plan() {
    let Some(target_name) = super::host_target_name() else {
        return;
    };
    let fixture = IndependentFixture::new(target_name);
    let diagnostics = fixture
        .compile_root(fixture.attach(vec![fixture.published_foreign_component()]))
        .expect_err("a component that realizes nothing selected cannot close the edge");
    rejects_with(
        &diagnostics,
        &[
            "no verified component realizes it",
            "refusing to treat the edge as fused",
        ],
    );
}

#[test]
fn independent_selection_rejects_a_component_exporting_an_unresolved_row() {
    let Some(target_name) = super::host_target_name() else {
        return;
    };
    let fixture = IndependentFixture::new(target_name);
    fixture.rewrite_dependency(BOUNDED_COMPONENT_SOURCE);
    let published = fixture.published();
    let diagnostics = fixture
        .compile_root(fixture.attach(vec![published]))
        .expect_err(
            "a component retaining an unresolved installation-bound row cannot close the edge",
        );
    rejects_with(
        &diagnostics,
        &[
            "unresolved installation-bound requirement row",
            "Installer::install",
            "refusing to treat the edge as fused",
        ],
    );
}

#[test]
fn a_stale_description_no_longer_realizes_the_selected_plan() {
    let Some(target_name) = super::host_target_name() else {
        return;
    };
    let fixture = IndependentFixture::new(target_name);
    let stale = fixture.published();
    fixture.rewrite_dependency(RENAMED_ADAPTER_COMPONENT_SOURCE);
    let diagnostics = fixture
        .compile_root(fixture.attach(vec![stale]))
        .expect_err("a description published before the adapter was renamed is stale");
    rejects_with(
        &diagnostics,
        &[
            "no verified component realizes it",
            "refusing to treat the edge as fused",
        ],
    );
}

#[test]
fn a_mechanism_bearing_component_rejects_without_authored_acceptance() {
    let Some(target_name) = super::host_target_name() else {
        return;
    };
    let fixture = IndependentFixture::new(target_name);
    fixture.rewrite_dependency_with(
        MECHANISM_COMPONENT_SOURCE,
        "    builder.freestanding = true;\n",
    );
    let published = fixture.published();
    let description = component_description::decode_component_description(published.description())
        .expect("the mechanism component publishes a decodable description");
    assert!(
        !description.assumptions.is_empty(),
        "the port write binds an inseparable assumption digest"
    );
    let diagnostics = fixture
        .compile_root(fixture.attach(vec![published]))
        .expect_err("the consuming build has no vocabulary to accept the mechanism assumption");
    rejects_with(
        &diagnostics,
        &[
            "failed independent verification",
            "is not accepted",
            "cannot deploy that package as an independent component",
        ],
    );
}

fn assumption_spelling(digest: &[u8; 32]) -> String {
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[test]
fn an_authored_acceptance_settles_the_mechanism_bearing_component() {
    let Some(target_name) = super::host_target_name() else {
        return;
    };
    let fixture = IndependentFixture::new(target_name);
    fixture.rewrite_dependency_with(
        MECHANISM_COMPONENT_SOURCE,
        "    builder.freestanding = true;\n",
    );
    let published = fixture.published();
    let description = component_description::decode_component_description(published.description())
        .expect("the mechanism component publishes a decodable description");
    let [assumption] = *description.assumptions else {
        panic!("the port write binds exactly one inseparable assumption digest");
    };
    write_consuming_root_with(
        &fixture.root,
        target_name,
        &format!(
            "    builder.freestanding = true;\n    builder.accept_component_assumption(\"{}\");\n",
            assumption_spelling(&assumption)
        ),
    );
    fixture
        .compile_root(fixture.attach(vec![published]))
        .expect("the authored acceptance admits the mechanism-bearing component");
}

#[test]
fn a_different_accepted_digest_leaves_the_mechanism_unaccepted() {
    let Some(target_name) = super::host_target_name() else {
        return;
    };
    let fixture = IndependentFixture::new(target_name);
    fixture.rewrite_dependency_with(
        MECHANISM_COMPONENT_SOURCE,
        "    builder.freestanding = true;\n",
    );
    let published = fixture.published();
    let description = component_description::decode_component_description(published.description())
        .expect("the mechanism component publishes a decodable description");
    let [assumption] = *description.assumptions else {
        panic!("the port write binds exactly one inseparable assumption digest");
    };
    let mut other = assumption;
    other[0] ^= 0xFF;
    write_consuming_root_with(
        &fixture.root,
        target_name,
        &format!(
            "    builder.accept_component_assumption(\"{}\");\n",
            assumption_spelling(&other)
        ),
    );
    let diagnostics = fixture
        .compile_root(fixture.attach(vec![published]))
        .expect_err("accepting a different digest never covers the component's assumption");
    rejects_with(
        &diagnostics,
        &["failed independent verification", "is not accepted"],
    );
}

#[test]
fn a_malformed_acceptance_spelling_rejects_at_its_own_declaration() {
    let Some(target_name) = super::host_target_name() else {
        return;
    };
    let fixture = IndependentFixture::new(target_name);
    write_consuming_root_with(
        &fixture.root,
        target_name,
        "    builder.accept_component_assumption(\"not-a-digest\");\n",
    );
    let diagnostics = fixture
        .compile_root(fixture.attach(vec![fixture.published()]))
        .expect_err("a malformed digest spelling rejects at its authored declaration");
    rejects_with(&diagnostics, &["64 hexadecimal characters"]);
}

#[test]
fn a_settled_independent_selection_retains_the_admitted_description_as_custody() {
    let Some(target_name) = super::host_target_name() else {
        return;
    };
    let fixture = IndependentFixture::new(target_name);
    let published = fixture.published();
    let checked = fixture
        .compile_root(fixture.attach(vec![published.clone()]))
        .expect("an independent selection over a described dependency component settles");
    let retained = checked.independent_component_descriptions();
    assert_eq!(
        retained.len(),
        1,
        "settlement retains exactly the attached descriptions"
    );
    assert_eq!(retained[0].package(), identity(DEPENDENCY_PACKAGE_MARKER));
    assert_eq!(
        retained[0].description(),
        published.description(),
        "custody retains the admitted bytes so a replay re-verifies the same admission"
    );
    assert!(
        checked.accepted_component_assumptions().is_empty(),
        "the consuming build declared no component-assumption acceptances"
    );
}

#[test]
fn a_description_naming_the_root_package_rejects_at_attachment() {
    let Some(target_name) = super::host_target_name() else {
        return;
    };
    let fixture = IndependentFixture::new(target_name);
    let published = fixture.published();
    let rooted = IndependentComponentDescription::new(
        identity(ROOT_PACKAGE_MARKER),
        published.expected_subject(),
        published.description().to_vec(),
    );
    let errors = fixture
        .inputs()
        .with_independent_component_descriptions(vec![rooted])
        .expect_err("the root package cannot attach a description of itself");
    assert!(
        errors.iter().any(|error| matches!(
            error,
            PackageCompilationInputError::RootIndependentComponentDescription { package }
                if *package == identity(ROOT_PACKAGE_MARKER)
        )),
        "expected the root-attachment rejection among: {errors:#?}"
    );
}

#[test]
fn a_description_naming_a_foreign_package_rejects_at_attachment() {
    const FOREIGN_PACKAGE_MARKER: u8 = 3;
    let Some(target_name) = super::host_target_name() else {
        return;
    };
    let fixture = IndependentFixture::new(target_name);
    let published = fixture.published();
    let foreign = IndependentComponentDescription::new(
        identity(FOREIGN_PACKAGE_MARKER),
        published.expected_subject(),
        published.description().to_vec(),
    );
    let errors = fixture
        .inputs()
        .with_independent_component_descriptions(vec![foreign])
        .expect_err("a description may only name a package inside the consuming closure");
    assert!(
        errors.iter().any(|error| matches!(
            error,
            PackageCompilationInputError::ForeignIndependentComponentDescription { package }
                if *package == identity(FOREIGN_PACKAGE_MARKER)
        )),
        "expected the foreign-attachment rejection among: {errors:#?}"
    );
}

#[test]
fn a_second_description_for_the_same_dependency_rejects_at_attachment() {
    let Some(target_name) = super::host_target_name() else {
        return;
    };
    let fixture = IndependentFixture::new(target_name);
    let published = fixture.published();
    let duplicate = IndependentComponentDescription::new(
        published.package(),
        published.expected_subject(),
        published.description().to_vec(),
    );
    let errors = fixture
        .inputs()
        .with_independent_component_descriptions(vec![published, duplicate])
        .expect_err("one dependency admits exactly one component description");
    assert!(
        errors.iter().any(|error| matches!(
            error,
            PackageCompilationInputError::DuplicateIndependentComponentDescription { package }
                if *package == identity(DEPENDENCY_PACKAGE_MARKER)
        )),
        "expected the duplicate-attachment rejection among: {errors:#?}"
    );
}

#[test]
fn a_genuine_description_beside_a_substituted_subject_rejects() {
    let Some(target_name) = super::host_target_name() else {
        return;
    };
    let fixture = IndependentFixture::new(target_name);
    let published = fixture.published();
    let substituted = IndependentComponentDescription::new(
        published.package(),
        TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([0xAA; 32]),
        },
        published.description().to_vec(),
    );
    let diagnostics = fixture
        .compile_root(fixture.attach(vec![substituted]))
        .expect_err(
            "the carried subject is a coordinate the caller supplies, not a field read from the bytes",
        );
    rejects_with(
        &diagnostics,
        &[
            "failed independent verification",
            "is not the admitted subject",
        ],
    );
}

#[test]
fn an_attached_description_is_unmatched_when_the_dependency_selects_fused() {
    let Some(target_name) = super::host_target_name() else {
        return;
    };
    let fixture = IndependentFixture::new(target_name);
    write_consuming_root_fused(&fixture.root, target_name);
    let diagnostics = fixture
        .compile_root(fixture.attach(vec![fixture.published()]))
        .expect_err("a verified component no Independent selection consumes is not inert input");
    rejects_with(
        &diagnostics,
        &["realizes no independently selected provider plan"],
    );
}
