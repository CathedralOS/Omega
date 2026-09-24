//! An independently compiled C counterparty observes real foreign scalar ABI
//! transport. Its result feeds the next call, rather than being discarded.

#[test]
fn mixed_foreign_scalars_execute_with_exact_argument_and_result_values() {
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    run_macos_scalar_counterparty();
    #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
    eprintln!("skip: mixed foreign scalar execution requires macOS ARM64");
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn run_macos_scalar_counterparty() {
    use super::{
        Fixture, admit_imports, native, terminal_authority_permission_policy,
        terminal_authority_policy,
    };
    use compiler::{RetainedNativeRealizationRequest, SourceEvaluatedImportSettlement};
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::process::Command;

    let fixture = Fixture::with_source(
        "mixed-foreign-scalars",
        "macos_arm64",
        "",
        r#"machine build(builder: &mut Build) {
    builder.application("mixed_foreign_scalars");
    builder.roots.bind(macos_arm64::ProgramEntry, Main::main);
}"#,
    );
    let library = fixture.root.join("libscalar-probe.dylib");
    let c_source = fixture.root.join("scalar-probe.c");
    fs::write(
        &c_source,
        r#"
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
static int invocation;
bool choose(void) { return true; }
double exchange(bool enabled, float small, double wide, int ordinal) {
    if (invocation == 0 && enabled && small == 1.5f && wide == 2.25 && ordinal == 17) {
        invocation = 1;
        return -9.5;
    }
    if (invocation == 1 && !enabled && small == 3.25f && wide == -9.5 && ordinal == 23) {
        invocation = 2;
        return 6.5;
    }
    _Exit(91);
}
float narrow(double result) {
    if (invocation != 2 || result != 6.5) _Exit(94);
    return -1.25f;
}
void finish(float small, double result) {
    if (invocation != 2 || small != -1.25f || result != 6.5) _Exit(92);
    puts("foreign scalars: PASS");
    fflush(stdout);
    _Exit(0);
}
"#,
    )
    .expect("write independent C scalar counterparty");
    let compilation = Command::new("cc")
        .args([
            "-std=c11",
            "-Wall",
            "-Wextra",
            "-Werror",
            "-dynamiclib",
            "-install_name",
        ])
        .arg(&library)
        .arg(&c_source)
        .arg("-o")
        .arg(&library)
        .output()
        .expect("compile C scalar counterparty with the host compiler");
    assert!(
        compilation.status.success(),
        "{}",
        String::from_utf8_lossy(&compilation.stderr)
    );
    let install_name = library.to_str().expect("UTF-8 fixture library path");
    let install_name_length = install_name.len();
    fs::write(
        &fixture.main,
        format!(
            r#"
use omega::language::core::binding;
use omega::language::core::external_binding;
pub boundary trait ScalarProbe {{
    machine choose() -> bool;
    machine exchange(enabled: bool, small: f32, wide: f64, ordinal: i32) -> f64;
    machine narrow(result: f64) -> f32;
    machine finish(small: f32, result: f64);
}}
macos_arm64 machine choose_binding() -> ForeignBinding<{install_name_length}, 7, 0> {{
    ForeignBinding::DllImport {{ import: DllImport::MachODylibSymbol {{
        install_name: "{install_name}", symbol: "_choose",
    }} }}
}}
macos_arm64 machine exchange_binding() -> ForeignBinding<{install_name_length}, 9, 0> {{
    ForeignBinding::DllImport {{ import: DllImport::MachODylibSymbol {{
        install_name: "{install_name}", symbol: "_exchange",
    }} }}
}}
macos_arm64 machine finish_binding() -> ForeignBinding<{install_name_length}, 7, 0> {{
    ForeignBinding::DllImport {{ import: DllImport::MachODylibSymbol {{
        install_name: "{install_name}", symbol: "_finish",
    }} }}
}}
macos_arm64 machine narrow_binding() -> ForeignBinding<{install_name_length}, 7, 0> {{
    ForeignBinding::DllImport {{ import: DllImport::MachODylibSymbol {{
        install_name: "{install_name}", symbol: "_narrow",
    }} }}
}}
machine choose_leaf() -> bool satisfies ScalarProbe::choose via choose_binding();
machine exchange_leaf(enabled: bool, small: f32, wide: f64, ordinal: i32) -> f64
    satisfies ScalarProbe::exchange via exchange_binding();
machine narrow_leaf(result: f64) -> f32 satisfies ScalarProbe::narrow via narrow_binding();
machine finish_leaf(small: f32, result: f64) satisfies ScalarProbe::finish via finish_binding();
data Main {{ probe: Binding<ScalarProbe>; }}
machine Main::main(&mut self) reaches ScalarProbe {{
    let enabled: bool = self.probe.choose();
    let first: f64 = self.probe.exchange(enabled, 1.5f32, 2.25f64, 17);
    let second: f64 = self.probe.exchange(false, 3.25f32, first, 23);
    let small: f32 = self.probe.narrow(second);
    self.probe.finish(small, second);
}}
"#
        ),
    )
    .expect("write source-produced mixed scalar calls");
    let retained = fixture.compile_terminal();
    let admissions = admit_imports(&retained, 0x5343_414c_0001);
    assert_eq!(admissions.len(), 4);
    let imports = admissions
        .iter()
        .map(|admission| {
            SourceEvaluatedImportSettlement::new(&admission.execution, &admission.same_stack)
        })
        .collect::<Vec<_>>();
    let policy = terminal_authority_policy(&retained);
    let permission_policy = terminal_authority_permission_policy(&retained);
    let image_request = native_realization::ExecutableImageEmissionRequest::direct(
        retained
            .native_realization_proposal()
            .expect("native proposal")
            .subsystem(),
    );
    fs::remove_file(&fixture.main).expect("remove Omega source before native realization");
    let artifact = compiler::realize_retained_native_artifact(
        retained,
        RetainedNativeRealizationRequest {
            profile: &proof_admission::AdmissionProfile::default(),
            optimization_selections:
                &optimization_core::PostTerminalOptimizationSelections::default(),
            terminal_authority_policy: policy,
            accepted_package_terminal_authority_permission_policy:
                native_realization::current_terminal_authority_permission_policy(),
            terminal_authority_permission_policy: Some(permission_policy),
            image_request,
            imports: &imports,
        },
    )
    .map_err(|(_, diagnostics)| diagnostics)
    .unwrap_or_else(|diagnostics| {
        panic!("mixed foreign scalar native realization: {diagnostics:#?}")
    });
    let native_realization::RequestedNativeArtifact::Direct(artifact) = artifact else {
        panic!("Mach-O requested a direct native artifact")
    };
    artifact
        .validate()
        .expect("independently replay native scalar ABI custody");
    assert!(matches!(
        artifact.physical_evidence_scope(),
        native::NativePhysicalEvidenceScope::ValidatedOptimizedProjection(_)
    ));
    let evidence = artifact
        .physical_evidence()
        .expect("complete physical evidence");
    assert_eq!(evidence.projection().boundary_occurrences().len(), 5);
    assert_eq!(evidence.children().len(), 5);
    assert_eq!(artifact.object().foreign_calls().len(), 5);
    let executable = fixture.root.join("foreign-scalars");
    fs::write(&executable, &artifact.image().output().bytes).expect("write published native image");
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
    let output = Command::new(&executable)
        .output()
        .expect("execute published native scalar caller");
    assert_eq!(
        output.status.code(),
        Some(0),
        "status {:?}, stderr {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"foreign scalars: PASS\n");
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let parts = artifact.into_parts();
    for mutation in 0..4 {
        let mut replay = super::replay_native_artifact_parts(&parts);
        let call = &mut replay.object.foreign_calls_mut_for_test()[1];
        match mutation {
            0 => call.scalar_arguments[0].parameter_index = 1,
            1 => call.scalar_arguments.swap(0, 1),
            2 => call.scalar_arguments[1].placement = call.scalar_arguments[0].placement.clone(),
            _ => {
                call.scalar_result.as_mut().unwrap().home.scalar_type =
                    semantic_vocabulary::ScalarType::Boolean
            }
        }
        assert!(
            native::NativeArtifact::from_replayed_parts(replay).is_err(),
            "foreign scalar artifact mutation {mutation} must reject"
        );
    }
    super::assert_physical_children_mutation_rejected(&parts, |children| {
        children.remove(0);
    });
    super::assert_physical_children_mutation_rejected(&parts, |children| {
        let mut child = children[1].clone().into_parts();
        let native::PhysicalChildParent::BoundaryTraitSettlement(parent) = child.parent else {
            panic!("foreign scalar settlement parent")
        };
        let mut parent = parent.into_parts();
        parent.requirement_identity.push_str("::substituted");
        child.parent = native::PhysicalChildParent::BoundaryTraitSettlement(
            native::BoundaryTraitSettlement::from_replayed_parts(parent),
        );
        children[1] = native::NativePhysicalChild::from_replayed_parts(child);
    });
}
