//! Construct the initial Build under its admitted vocabulary and filesystem scope.

use super::vocabulary::has_exact_toolchain_build_facet;
use super::{BuildValueAdmission, vocabulary};
use crate::build_execution::{
    BUILD_OUTPUT_ROOT_IDENTITY, BUILD_SOURCE_ROOT_IDENTITY, BuildMachineFilesystemScope,
};
use build_time_evaluation::{BuildMachineFilesystemGrantRootIdentity, BuildTimeValue};
use typed_trees::TypedTrees;

pub(super) fn initial_value(
    admission: &BuildValueAdmission,
    typed: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    filesystem_scope: &BuildMachineFilesystemScope,
) -> BuildTimeValue {
    let mut build_fields = Vec::new();
    if admission.has_toolchain_build_activation {
        build_fields.push((
            "x86_deployment_features".to_owned(),
            BuildTimeValue::Case {
                variant: "Baseline".to_owned(),
                payload: Vec::new(),
            },
        ));
    }
    build_fields.extend([
        (
            "subsystem".to_owned(),
            BuildTimeValue::Case {
                variant: "Console".to_owned(),
                payload: Vec::new(),
            },
        ),
        ("freestanding".to_owned(), BuildTimeValue::Bool(false)),
    ]);
    // The toolchain Build declares `identifier`; an authored Build opts in by
    // declaring the same field. Empty bytes mean no authored identity.
    if vocabulary::build_machine_declares_identifier_field(typed, machine) {
        build_fields.push(("identifier".to_owned(), BuildTimeValue::Text(Vec::new())));
    }
    if let Some(field) = admission.optimization.zero_build_field() {
        build_fields.push(field);
    }
    if has_exact_toolchain_build_facet(typed, "BuildLog") {
        build_fields.push((
            "log".to_owned(),
            BuildTimeValue::Struct {
                type_name: "$OmegaBuildLogFacet".to_owned(),
                fields: Vec::new(),
            },
        ));
    }
    // The product facet is a compiler-issued marker: `builder.product.entry`
    // is the only product-selection route, and an authored `BuildProduct`
    // value never carries this private runtime identity.
    if has_exact_toolchain_build_facet(typed, "BuildProduct") {
        build_fields.push((
            "product".to_owned(),
            BuildTimeValue::Struct {
                type_name: "$OmegaBuildProductFacet".to_owned(),
                fields: Vec::new(),
            },
        ));
    }
    // Granular privileged-service grants (wiki/spec/build/permissions.md
    // #privileged-services): every flag is false until the root build machine
    // assigns it, so a hosted image admits no mediated privileged class.
    if has_exact_toolchain_build_facet(typed, "PrivilegedServices") {
        build_fields.push((
            "privileged_services".to_owned(),
            BuildTimeValue::Struct {
                type_name: "PrivilegedServices".to_owned(),
                fields: vec![
                    ("port_io".to_owned(), BuildTimeValue::Bool(false)),
                    ("interrupt_table".to_owned(), BuildTimeValue::Bool(false)),
                ],
            },
        ));
    }
    // ELF loader selection (wiki/spec/build/configuration.md
    // #elf-executable-loader-selection): every row is empty until the root
    // build machine assigns it, so each Linux target keeps its target
    // environment's declared default.
    if has_exact_toolchain_build_facet(typed, "ElfInterpreterSelection") {
        build_fields.push((
            "elf_interpreter".to_owned(),
            BuildTimeValue::Struct {
                type_name: "ElfInterpreterSelection".to_owned(),
                fields: target::ELF_INTERPRETER_TARGET_PROFILES
                    .iter()
                    .map(|target| {
                        (
                            target.target_name().to_owned(),
                            BuildTimeValue::Text(Vec::new()),
                        )
                    })
                    .collect(),
            },
        ));
    }
    if has_exact_toolchain_build_facet(typed, "BuildSource")
        && has_exact_toolchain_build_facet(typed, "BuildOutput")
    {
        let root_facet = |type_name: &str, root: BuildMachineFilesystemGrantRootIdentity| {
            BuildTimeValue::Struct {
                type_name: type_name.to_owned(),
                fields: vec![(
                    "root".to_owned(),
                    BuildTimeValue::Int(i64::from(root.get())),
                )],
            }
        };
        build_fields.extend([
            (
                "source".to_owned(),
                root_facet("$OmegaBuildSourceRoot", BUILD_SOURCE_ROOT_IDENTITY),
            ),
            (
                "output".to_owned(),
                root_facet("$OmegaBuildOutputRoot", BUILD_OUTPUT_ROOT_IDENTITY),
            ),
        ]);
    }
    if has_exact_toolchain_build_facet(typed, "BuildInputs") {
        build_fields.push(("inputs".to_owned(), filesystem_scope.named_input_facet()));
    }
    BuildTimeValue::Struct {
        type_name: "Build".to_owned(),
        fields: build_fields,
    }
}
