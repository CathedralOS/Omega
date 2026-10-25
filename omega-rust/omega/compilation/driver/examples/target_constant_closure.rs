//! Exercise historical layout-constant closure admission from authored source.
//!
//! Run `mbx run -p compiler --example target_constant_closure`. The ordinary
//! package route produces each artifact and its real plans. Canonical ingress
//! and receiving/native plan admission must preserve target and provider
//! identity even when a substituted provider returns the same layout extent.

#[path = "../tests/support/fixture_package_inputs.rs"]
mod fixture_package_inputs;

use compiler::{CompileOptions, CompileRequest, RequestedCompileProduct, compile};
use std::path::Path;

fn produce(
    root: &Path,
    target_name: &str,
    product: RequestedCompileProduct,
) -> compiler::CompileReport {
    let prepared =
        fixture_package_inputs::reviewed_repository_fixture_package_inputs(root, Some(target_name))
            .expect("source-backed package inputs");
    let mut request = CompileRequest::new(CompileOptions {
        root_path: prepared.root_path,
        build_dir: Some(std::env::temp_dir().join(format!(
            "omega-layout-closure-{}-{target_name}",
            std::process::id(),
        ))),
        target_name: Some(target_name.to_owned()),
    })
    .with_requested_product(product);
    if let Some(inputs) = prepared.package_inputs {
        request = request.with_package_inputs(inputs);
    }
    compile(request)
        .expect("request admission")
        .into_single_report()
        .expect("source reaches its requested product")
}

fn main() {
    if std::env::args().nth(1).as_deref() == Some("--build-profile") {
        println!("{}", compiler::compiler_build_profile());
        return;
    }
    let root = fixture_package_inputs::repo_root()
        .join("samples/cli/facts/target_layout_derived_constant/main.omg");
    let retained = produce(
        &root,
        "linux_x86_64",
        RequestedCompileProduct::TerminalArtifact,
    )
    .into_retained_terminal_artifact()
    .expect("native proposal");
    let proposal = retained
        .native_realization_proposal()
        .expect("native proposal");
    let decoded =
        terminal_codec::CanonicalTerminalArtifact::from_bytes(&retained.artifact().to_bytes())
            .expect("canonical Terminal roundtrip");
    assert_eq!(
        decoded.manifest().semantic(),
        retained.artifact().manifest().semantic()
    );
    let view = decoded.view();
    let plans = proposal.providers().selection.plans();
    let preimages =
        backend_plan::matching_folded_constant_dependencies(view, "linux_x86_64", plans.iter())
            .expect("native admission rejoins exact real plans");
    assert!(
        !preimages.is_empty(),
        "folding must retain the selected extent provider"
    );
    let admit = |target, inputs: &[Vec<u8>]| {
        terminal_installation::admit_provider_installation(view, &[], &[])
            .expect("historical-only provider catalog")
            .admit_folded_constant_closure(view, target, inputs)
    };
    admit("linux_x86_64", &preimages).expect("exact historical closure");
    assert_eq!(
        admit("linux_arm64", &preimages).unwrap_err(),
        "folded constant closure belongs to another target"
    );
    assert_eq!(
        admit("linux_x86_64", &[]).unwrap_err(),
        "folded constant selected realization closure is incomplete or repeated"
    );
    let mut repeated = preimages.clone();
    repeated.push(preimages[0].clone());
    assert_eq!(
        admit("linux_x86_64", &repeated).unwrap_err(),
        "folded constant selected realization closure is incomplete or repeated"
    );
    let digests = view
        .module()
        .folded_constants
        .dependencies
        .iter()
        .filter(|dependency| dependency.target == "linux_x86_64")
        .map(|dependency| dependency.plan.to_vec())
        .collect::<Vec<_>>();
    assert_eq!(
        admit("linux_x86_64", &digests).unwrap_err(),
        "folded constant normalized plan input differs from its retained commitment"
    );

    let scratch = fixture_package_inputs::repo_root()
        .join("build")
        .join(format!(
            "omega-equal-layout-provider-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
        ));
    std::fs::create_dir(&scratch).expect("private source control directory");
    let source = std::fs::read_to_string(&root).expect("authored witness");
    let alternate_source = source.replace(
        "select_provider<Answer, Small>",
        "select_provider<Answer, Same>",
    );
    assert_ne!(alternate_source, source);
    std::fs::write(scratch.join("main.omg"), alternate_source).unwrap();
    std::fs::copy(root.with_file_name("build.omg"), scratch.join("build.omg")).unwrap();
    let alternate = produce(
        &scratch.join("main.omg"),
        "linux_x86_64",
        RequestedCompileProduct::TerminalArtifact,
    )
    .into_retained_terminal_artifact()
    .expect("alternate native proposal");
    let alternate_plans = alternate
        .native_realization_proposal()
        .unwrap()
        .providers()
        .selection
        .plans();
    let reason = backend_plan::matching_folded_constant_dependencies(
        view,
        "linux_x86_64",
        alternate_plans.iter(),
    )
    .unwrap_err();
    assert!(
        reason.contains("differs from the selected provider plan"),
        "{reason}"
    );
    let alternate_inputs = backend_plan::matching_folded_constant_dependencies(
        alternate.artifact().view(),
        "linux_x86_64",
        alternate_plans.iter(),
    )
    .expect("alternate provider's own real closure");
    assert_eq!(
        admit("linux_x86_64", &alternate_inputs).unwrap_err(),
        "folded constant normalized plan input differs from its retained commitment"
    );
    std::fs::remove_dir_all(scratch).unwrap();
    // This emits a Windows product but executes only its Terminal semantics on
    // this host. The frozen source's branch traps unless the derived value is
    // exactly 9 (the Windows provider extent 8 plus 1), rather than Linux's 5.
    let windows = produce(
        &root,
        "windows_x86_64",
        RequestedCompileProduct::NativeArtifact,
    );
    let artifact = windows
        .retained_native_artifact()
        .expect("Windows native emission");
    let observation = terminal_runtime::execute(terminal_runtime::TerminalRuntimeRequest {
        x86_flags: None,
        provider_proposal: None,
        artifact: artifact.psi_artifact(),
        target: None,
        provisions: windows.runtime_provisions(),
        fuel_allowance: 100_000,
        domain_exit: terminal_runtime::TerminalDomainExitDelegation::IsolatedContainingDomain,
        standard_input: None,
        clock: None,
    })
    .expect("Windows Terminal execution on this host");
    let complete = match &observation.status {
        terminal_interpreter::TerminalExecutionStatus::Complete(
            terminal_interpreter::TerminalExecutionResult::Unit,
        ) => true,
        terminal_interpreter::TerminalExecutionStatus::ExternalCompletion(completion) => {
            completion.return_code == 0
        }
        _ => false,
    };
    assert!(complete, "{:?}", observation.status);
    println!("windows_terminal_exit=0 windows_derived_value=9 foreign_native_executed=false");

    println!(
        "canonical_roundtrip=ok exact_plan=admitted target_mismatch=rejected absent_plan=rejected repeated_plan=rejected digest_substitution=rejected equal_provider_substitution=rejected"
    );
}
