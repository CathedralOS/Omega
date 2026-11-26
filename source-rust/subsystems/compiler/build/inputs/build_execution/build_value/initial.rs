// Initial `BuildPlan` value construction under its admitted vocabulary and filesystem scope.
//
// Populates the interpreter's starting `BuildPlan` struct with baseline defaults
// (`x86_deployment_features = Baseline`, `subsystem = Console`),
// optional `identifier` bytes, zeroed `optimizations` counters, compiler-issued
// private facet markers (`$OmegaBuildLogFacet`, `$OmegaBuildDeclarationsFacet`),
// an enabled `tests` facet, empty `elf_interpreter` rows for each ELF target
// profile, and filesystem grant roots (`$OmegaBuildSourceRoot`,
// `$OmegaBuildOutputRoot`, `$OmegaBuildInputSlots`).

use super::BuildValueAdmission;
use crate::build_execution::{
    BUILD_OUTPUT_ROOT_IDENTITY, BUILD_SOURCE_ROOT_IDENTITY, BuildMachineFilesystemScope,
};
use build_time_values::BuildTimeValue;
use filesystem_authority::FilesystemGrantRootIdentity;

pub(super) fn initial_value(
    admission: &BuildValueAdmission,
    filesystem_scope: &BuildMachineFilesystemScope,
) -> BuildTimeValue {
    // Source cannot construct the issued seal in the Build borrowed from the host.
    let mut build_fields = vec![
        (
            "product".to_owned(),
            BuildTimeValue::Case {
                variant: "Unselected".into(),
                payload: Vec::new(),
            },
        ),
        (
            "package_root".to_owned(),
            BuildTimeValue::Case {
                variant: "Current".into(),
                payload: Vec::new(),
            },
        ),
        (
            "seal".to_owned(),
            BuildTimeValue::Struct {
                type_name: "$OmegaBuildSeal".to_owned(),
                fields: Vec::new(),
            },
        ),
        (
            "x86_deployment_features".to_owned(),
            BuildTimeValue::Case {
                variant: "Baseline".to_owned(),
                payload: Vec::new(),
            },
        ),
        (
            "subsystem".to_owned(),
            BuildTimeValue::Case {
                variant: "Console".to_owned(),
                payload: Vec::new(),
            },
        ),
    ];
    if admission.identifier {
        build_fields.push(("identifier".to_owned(), BuildTimeValue::Text(Vec::new())));
    }
    build_fields.push((
        "optimizations".to_owned(),
        super::optimization::initial_value(),
    ));
    if admission.log {
        build_fields.push((
            "log".to_owned(),
            BuildTimeValue::Struct {
                type_name: "$OmegaBuildLogFacet".to_owned(),
                fields: Vec::new(),
            },
        ));
    }
    if admission.declarations {
        build_fields.push((
            "declarations".to_owned(),
            BuildTimeValue::Struct {
                type_name: "$OmegaBuildDeclarationsFacet".to_owned(),
                fields: Vec::new(),
            },
        ));
    }
    if admission.elf_interpreter {
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
    // Testing starts enabled: a zero-initialized `tests` facet never silently
    // disables registered groups (wiki/spec/build/testing.md).
    if admission.tests {
        build_fields.push((
            "tests".to_owned(),
            BuildTimeValue::Struct {
                type_name: "BuildTests".to_owned(),
                fields: vec![("disabled".to_owned(), BuildTimeValue::Bool(false))],
            },
        ));
    }
    if admission.filesystem_roots {
        let root_facet =
            |type_name: &str, root: FilesystemGrantRootIdentity| BuildTimeValue::Struct {
                type_name: type_name.to_owned(),
                fields: vec![(
                    "root".to_owned(),
                    BuildTimeValue::Int(i64::from(root.get())),
                )],
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
    if admission.inputs {
        build_fields.push(("inputs".to_owned(), filesystem_scope.named_input_facet()));
    }
    BuildTimeValue::Struct {
        type_name: build_declarations::BUILD_TYPE_NAME.to_owned(),
        fields: build_fields,
    }
}
