//! Structural equations recover arguments by declaration, not layout or spelling.

use compiler::{CheckedCompilation, CheckedCompileRequest, compile_to_checked};
use diagnostics::Diagnostic;
use package_compilation::{PackageCompilationInputs, PackageSourceBinding};
use semantic_vocabulary::PackageKeyIdentity;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_PROJECT: AtomicU64 = AtomicU64::new(0);

#[path = "../../../../../tests/native-differential/tests/common/native_function.rs"]
#[allow(dead_code)]
mod native_function;

const SOURCE: &str = r#"
data Cell<Element, const Capacity: u64> { storage: [Element; Capacity]; }
machine capacity<Backing, Element, const Capacity: u64>() -> u64
where Backing == Cell<Element, Capacity>
{ Capacity }
machine recovered() -> u64 { capacity<Cell<u8, 7>>() }
"#;

struct Project(PathBuf);

impl Project {
    fn new(sources: &[(&str, &str)]) -> Self {
        let root = std::env::temp_dir().join(format!(
            "omega-application-equations-{}-{}",
            std::process::id(),
            NEXT_PROJECT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        for (name, source) in sources {
            fs::write(root.join(name), source).unwrap();
        }
        Self(root)
    }

    fn check(&self) -> Result<CheckedCompilation, Vec<Diagnostic>> {
        let package = PackageKeyIdentity::from_digest([1; 32]).unwrap();
        let inputs = PackageCompilationInputs::new_package(
            package,
            vec![PackageSourceBinding::new(
                package,
                "application-equations",
                self.0.clone(),
            )],
            Vec::new(),
        )
        .unwrap();
        compile_to_checked(CheckedCompileRequest {
            package_inputs: Some(inputs),
            ..CheckedCompileRequest::new(&self.0.join("main.omg"), Some("macos_arm64"))
        })
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        if !std::thread::panicking() {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
}

fn executes(sources: &[(&str, &str)]) -> terminal_codec::CanonicalTerminalArtifact {
    let project = Project::new(sources);
    let checked = project
        .check()
        .unwrap_or_else(|errors| panic!("{}: {errors:#?}", project.0.display()));
    let entries = checked
        .typed
        .machines()
        .iter()
        .filter(|machine| checked.typed.machine_type_parameters(machine).is_empty())
        .map(|machine| checked.symbols.display_path(machine.symbol, "::"))
        .filter(|name| name == "recovered" || name.ends_with("::recovered"))
        .collect::<Vec<_>>();
    let [entry] = entries.as_slice() else {
        panic!("one closed recovered entry: {entries:?}");
    };
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, entry)
        .produce_artifact()
        .expect("application equation reaches Terminal");
    drop(checked);
    let root = project.0.clone();
    drop(project);
    assert!(!root.exists());
    assert_eq!(
        terminal_interpreter::interpret_terminal_artifact(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &proof_admission::AdmissionProfile::default(),
            &[]
        )
        .unwrap(),
        terminal_interpreter::TerminalExecutionResult::Scalar(
            terminal_interpreter::TerminalScalarValue::Integer {
                scalar_type: semantic_vocabulary::IntegerType::new(
                    semantic_vocabulary::IntegerSign::Unsigned,
                    64
                )
                .unwrap(),
                value: semantic_vocabulary::IntegerValue::Unsigned(7),
            }
        )
    );
    artifact
}

fn rejects(sources: &[(&str, &str)], fragment: &str) {
    let project = Project::new(sources);
    let Err(errors) = project.check() else {
        panic!("invalid application must fail ({fragment}): {sources:?}");
    };
    assert!(
        errors.iter().any(|error| error.message.contains(fragment)),
        "expected {fragment:?}: {errors:#?}"
    );
}

#[test]
fn declared_application_recovers_element_and_capacity_natively() {
    let artifact = executes(&[("main.omg", SOURCE)]);
    assert_native_seven(&artifact);
}

fn assert_native_seven(artifact: &terminal_codec::CanonicalTerminalArtifact) {
    let selections = optimization_core::OptimizationSelections::new([]).unwrap();
    let optimized = native_realization::optimize_artifact_sections(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &proof_admission::AdmissionProfile::default(),
        native_realization::compiler_baseline_request_v1(&selections),
    )
    .unwrap();
    let physical =
        native_realization::stage_optimized_verified_physical_pipeline_with_provider_executions(
            optimized,
            target::NativeTarget::host(),
            &[],
        )
        .unwrap();
    let fragments = machine_emission::stage_optimized_function_fragment_emission(
        physical.into_function_fragment_emission_source(),
    )
    .unwrap();
    let framed = machine_emission::stage_function_fragment_frame_application(fragments).unwrap();
    let text = machine_emission::stage_optimized_fixed_frame_text_section(framed).unwrap();
    let source = std::sync::Arc::new(
        object_file::stage_optimized_relocation_free_object_container(text).unwrap(),
    );
    let object = image_emission::build_function_fragment_object_artifact(source.clone()).unwrap();
    image_emission::validate_function_fragment_object_artifact(&source, &object).unwrap();
    let image = image_emission::emit_executable_image(&object, 0).unwrap();
    image_emission::validate_executable_image(&object, &image).unwrap();
    #[cfg(any(
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(target_os = "macos", target_arch = "aarch64")
    ))]
    native_function::assert_c_text(
        &image.output().final_text_bytes,
        object.entry_function().text_offset,
        "#include <stdint.h>\nextern uint64_t omega_entry(void);\nint main(void) { return omega_entry() == 7 ? 0 : 1; }",
    );
    #[cfg(not(any(
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(target_os = "macos", target_arch = "aarch64")
    )))]
    eprintln!("SKIP: native application execution requires a supported Linux or macOS host");
}

#[test]
fn module_owned_array_equation_uses_its_declaring_constant_natively() {
    let layout = r#"
module layout;
const Count: u64 = 7;
pub machine width<Backing, Element>() -> u64
where Backing == [Element; Count]
{ Count }
"#;
    let main =
        "use layout; const Count: u64 = 4; machine recovered() -> u64 { layout::width<[u8; 7]>() }";
    let artifact = executes(&[("main.omg", main), ("layout.omg", layout)]);
    assert_native_seven(&artifact);
    let reverse = layout.replace("width<Backing, Element>", "width<Element, Backing>");
    drop(executes(&[
        ("main.omg", &main.replace("width<[u8; 7]>()", "width<u8>()")),
        ("layout.omg", &reverse),
    ]));
    rejects(
        &[
            (
                "main.omg",
                &main.replace("width<[u8; 7]>()", "width<[u8; 4]>()"),
            ),
            ("layout.omg", layout),
        ],
        "conflicting fixed-array lengths",
    );
}

#[test]
fn module_array_equations_select_imports_without_root_name_fallback() {
    let main =
        "use layout; const Count: u64 = 4; machine recovered() -> u64 { layout::width<[u8; 7]>() }";
    let layout = r#"
module layout;
use dimensions::Count;
pub machine width<Backing, Element>() -> u64
where Backing == [Element; Count]
{ 7 }
"#;
    let dimensions = "module dimensions; pub const Count: u64 = 7;";
    drop(executes(&[
        ("main.omg", main),
        ("layout.omg", layout),
        ("dimensions.omg", dimensions),
    ]));
    rejects(
        &[
            ("main.omg", main),
            (
                "layout.omg",
                &layout.replace(
                    "use dimensions::Count;",
                    "use dimensions::Count; use other::Count;",
                ),
            ),
            ("dimensions.omg", dimensions),
            ("other.omg", "module other; pub const Count: u64 = 7;"),
        ],
        "unambiguous selected array length constant",
    );
    rejects(
        &[
            (
                "main.omg",
                &format!("module caller; {}", main.replace("[u8; 7]", "[u8; 4]")),
            ),
            (
                "layout.omg",
                &layout.replace("use dimensions::Count;", "use dimensions;"),
            ),
            ("dimensions.omg", dimensions),
        ],
        "unambiguous selected array length constant",
    );
}

#[test]
fn module_array_equations_validate_the_declared_constant_before_using_its_value() {
    let main = "use layout; machine recovered() -> u64 { layout::width<[u8; 7]>() }";
    for (declaration, fragment) in [
        ("const Count: bool = true;", "integer array length constant"),
        ("const Count: i64 = -1;", "outside the supported extent"),
        ("const Count: u8 = 256;", "does not fit `u8`"),
    ] {
        let layout = format!(
            "module layout; {declaration} pub machine width<Backing, Element>() -> u64 where Backing == [Element; Count] {{ 7 }}"
        );
        rejects(&[("main.omg", main), ("layout.omg", &layout)], fragment);
    }
}

#[test]
fn data_array_equations_check_selected_module_lengths_in_both_directions() {
    let main = "use layout; const Count: u64 = 4;";
    let layout = r#"
module layout;
const Count: u64 = 7;
pub data Buffer<Backing, Element> where Backing == [Element; Count] { storage: Backing; }
pub machine preserve(value: Buffer<[u8; 7]>) -> Buffer<[u8; 7], u8> { value }
"#;
    let reverse = layout
        .replace("Buffer<Backing, Element>", "Buffer<Element, Backing>")
        .replace("Buffer<[u8; 7]>", "Buffer<u8>")
        .replace("Buffer<[u8; 7], u8>", "Buffer<u8, [u8; 7]>");
    for declaration in [layout, &reverse] {
        let project = Project::new(&[("main.omg", main), ("layout.omg", declaration)]);
        // Returning the inferred instance as its explicit tuple checks that
        // both applications selected the same data identity.
        project
            .check()
            .unwrap_or_else(|errors| panic!("{errors:#?}"));
    }
}

#[test]
fn application_equations_compose_with_arrays_and_nested_applications() {
    let array = SOURCE
        .replace(
            "Backing == Cell<Element, Capacity>",
            "Backing == [Cell<Element, Capacity>; 2]",
        )
        .replace("capacity<Cell<u8, 7>>()", "capacity<[Cell<u8, 7>; 2]>()");
    drop(executes(&[("main.omg", &array)]));
    let nested = SOURCE
        .replace(
            "Backing == Cell<Element, Capacity>",
            "Backing == Cell<Cell<Element, 2>, Capacity>",
        )
        .replace(
            "capacity<Cell<u8, 7>>()",
            "capacity<Cell<Cell<u8, 2>, 7>>()",
        );
    drop(executes(&[("main.omg", &nested)]));
}

#[test]
fn application_equations_construct_omitted_backing() {
    let source = SOURCE
        .replace(
            "capacity<Backing, Element, const Capacity: u64>",
            "capacity<Element, const Capacity: u64, Backing>",
        )
        .replace("capacity<Cell<u8, 7>>()", "capacity<u8, 7>()");
    drop(executes(&[("main.omg", &source)]));
}

#[test]
fn data_equations_share_application_matching_and_reverse_construction() {
    let source = r#"
data Cell<Element, const Capacity: u64> { storage: [Element; Capacity]; }
data Buffer<Backing, Element, const Capacity: u64>
where Backing == Cell<Element, Capacity>
{ storage: Backing; }
machine Buffer::recovered<const Capacity: u64>() -> u64 { Capacity }
machine preserve(value: Buffer<Cell<u8, 7> >) -> Buffer<Cell<u8, 7>, u8, 7> { value }
"#;
    drop(executes(&[("main.omg", source)]));
    let reverse = source
        .replace(
            "Buffer<Backing, Element, const Capacity: u64>",
            "Buffer<Element, const Capacity: u64, Backing>",
        )
        .replace("Buffer<Cell<u8, 7> >", "Buffer<u8, 7>")
        .replace("Buffer<Cell<u8, 7>, u8, 7>", "Buffer<u8, 7, Cell<u8, 7> >");
    drop(executes(&[("main.omg", &reverse)]));
    rejects(
        &[(
            "main.omg",
            &source.replace("Buffer<Cell<u8, 7> >", "Buffer<Cell<u8, 7>, u16, 7>"),
        )],
        "conflicting element types",
    );
}

#[test]
fn repeated_arguments_and_unsupported_lifetimes_remain_obligations() {
    let pair = r#"
data Pair<Left, Right> { left: Left; right: Right; }
machine capacity<Backing, Element>() -> u64 where Backing == Pair<Element, Element> { 7 }
machine recovered() -> u64 { capacity<Pair<u8, u8>>() }
"#;
    drop(executes(&[("main.omg", pair)]));
    rejects(
        &[(
            "main.omg",
            &pair.replace("capacity<Pair<u8, u8>>()", "capacity<Pair<u8, u16>>()"),
        )],
        "conflicting element types",
    );
    let repeated = SOURCE
        .replace(
            "Cell<Element, Capacity>",
            "Cell<Cell<Element, Capacity>, Capacity>",
        )
        .replace(
            "capacity<Cell<u8, 7>>()",
            "capacity<Cell<Cell<u8, 6>, 7>>()",
        );
    rejects(&[("main.omg", &repeated)], "to both 6 and 7");
    let lifetime = SOURCE.replace("data Cell<Element", "data Cell<'storage, Element");
    rejects(&[("main.omg", &lifetime)], "lifetime");
    let shadow = SOURCE
        .replace(
            "capacity<Backing, Element",
            "capacity<Backing, Cell, Element",
        )
        .replace("capacity<Cell<u8, 7>>()", "capacity<Cell<u8, 7>, u8>()");
    rejects(&[("main.omg", &shadow)], "binder as a nominal constructor");
}

#[test]
fn application_equations_preserve_explicit_arguments_and_integer_carriers() {
    for (replacement, fragment) in [
        (
            "capacity<Cell<u8, 7>, u16, 7>()",
            "conflicting element types",
        ),
        ("capacity<Cell<u8, 7>, u8, 8>()", "explicit argument is 8"),
        ("capacity<Cell<u8, 7u8>>()", "different integer meaning"),
        ("capacity<Cell<7, 7>>()", "type"),
        ("capacity<Cell<u8>>()", "argument"),
    ] {
        rejects(
            &[(
                "main.omg",
                &SOURCE.replace("capacity<Cell<u8, 7>>()", replacement),
            )],
            fragment,
        );
    }
    let overflow = SOURCE
        .replace("const Capacity: u64", "const Capacity: u8")
        .replace("Cell<u8, 7>", "Cell<u8, 256>");
    rejects(&[("main.omg", &overflow)], "outside");
    let obligation = SOURCE.replace(
        "data Cell<Element, const Capacity: u64> {",
        "data Cell<Element, const Capacity: u64> where Capacity > 8 {",
    );
    rejects(&[("main.omg", &obligation)], "is false");
    let generic_caller = obligation.replace("machine recovered() -> u64 { capacity<Cell<u8, 7>>() }", "machine wrapper<Unused>() -> u64 { capacity<Cell<u8, 7>>() } machine recovered() -> u64 { wrapper<u8>() }");
    rejects(&[("main.omg", &generic_caller)], "is false");
    let constructed = obligation
        .replace(
            "capacity<Backing, Element, const Capacity: u64>",
            "capacity<Element, const Capacity: u64, Backing>",
        )
        .replace("capacity<Cell<u8, 7>>()", "capacity<u8, 7>()");
    rejects(&[("main.omg", &constructed)], "is false");
}

#[test]
fn completion_reuses_an_existing_instance_and_checks_closed_generic_callers() {
    let source = format!(
        "{SOURCE}\ndata Owner {{ cell: Cell<u8, 7>; }}\nmachine Cell::width<const Capacity: u64>() -> u64 {{ Capacity }}"
    );
    let project = Project::new(&[("main.omg", &source)]);
    let checked = project
        .check()
        .unwrap_or_else(|errors| panic!("{errors:#?}"));
    assert_eq!(
        checked
            .typed
            .machines()
            .iter()
            .filter(|machine| checked.typed.machine_type_parameters(machine).is_empty())
            .filter(|machine| checked
                .symbols
                .display_path(machine.symbol, "::")
                .ends_with("::width"))
            .count(),
        1,
        "one concrete attached method, not one per normalization pass"
    );
    drop(checked);
    drop(project);
    drop(executes(&[("main.omg", &source)]));
    let generic_caller = SOURCE.replace("machine recovered() -> u64 { capacity<Cell<u8, 7>>() }", "machine wrapper<Unused>() -> u64 { capacity<Cell<u8, 7>>() } machine recovered() -> u64 { wrapper<u8>() }");
    drop(executes(&[("main.omg", &generic_caller)]));
}

#[test]
fn nested_open_binders_and_unsynthesized_constructors_cannot_hide_in_a_closed_parent() {
    let open = SOURCE.replace("machine recovered() -> u64 { capacity<Cell<u8, 7>>() }", "data Element { field: u8; } machine wrapper<Element>() -> u64 { capacity<Cell<Element, 7>>() } machine recovered() -> u64 { wrapper<u8>() }");
    rejects(&[("main.omg", &open)], "open caller binder");
    let unsupported = format!(
        "{}\ndata Outer<Inner> {{ inner: Inner; }}\nmachine Cell::open<Unused>() -> u64 {{ 0 }}",
        SOURCE
            .replace(
                "Backing == Cell<Element, Capacity>",
                "Backing == Outer<Cell<Element, Capacity>>"
            )
            .replace("capacity<Cell<u8, 7>>()", "capacity<Outer<Cell<u8, 7>>>()")
    );
    rejects(
        &[("main.omg", &unsupported)],
        "unnormalized data application",
    );
}

#[test]
fn application_constructor_selection_stays_with_its_declaring_module() {
    let layout = r#"
module layout;
pub data Cell<Element, const Capacity: u64> { storage: [Element; Capacity]; }
pub machine capacity<Backing, Element, const Capacity: u64>() -> u64
where Backing == Cell<Element, Capacity>
{ Capacity }
"#;
    let decoy = "module decoy; pub data Cell<Element, const Capacity: u64> { storage: [Element; Capacity]; }";
    let main = "use layout; use decoy; machine recovered() -> u64 { layout::capacity<layout::Cell<u8, 7>>() }";
    drop(executes(&[
        ("main.omg", main),
        ("layout.omg", layout),
        ("decoy.omg", decoy),
    ]));
    rejects(
        &[
            ("main.omg", &main.replace("layout::Cell", "decoy::Cell")),
            ("layout.omg", layout),
            ("decoy.omg", decoy),
        ],
        "conflicting nominal constructors",
    );
}
