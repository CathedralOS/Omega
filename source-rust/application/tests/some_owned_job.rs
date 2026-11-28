//! The provider supplies an established resident. The stored operation accepts
//! the complete owner by value and returns it in a new closed package.
#[path = "support/program.rs"]
mod program;

const SOURCE: &str = include_str!("fixtures/callables/run/some_owned_job/main.omg");

fn saved_complete(source: &str, expected_sites: usize) -> terminal_psi::TerminalModule {
    let scratch = platform_custody::temporary_directory::TemporaryDirectory::create(
        "owning-package-receipt",
        false,
    )
    .unwrap();
    let file = scratch.path().join("complete.psi");
    let (output, report) = program::invoke(
        source,
        &[
            "psi",
            "inspect",
            "--machine",
            "complete",
            "--output",
            file.to_str().unwrap(),
        ],
    );
    assert!(output.status.success(), "{report}");
    assert!(report.contains("psi selected_machine=complete"), "{report}");
    let bytes = std::fs::read(file).unwrap();
    let saved = terminal_codec::CanonicalTerminalArtifact::from_bytes(&bytes)
        .expect("admit the saved selected machine with its live claims");
    assert_eq!(bytes, saved.to_bytes());
    let module = saved.closed_module().module();
    assert_eq!(module.execution.packages.sites.len(), expected_sites);
    module.clone()
}

fn refused(module: &terminal_psi::TerminalModule) -> bool {
    terminal_codec::encode_module(module)
        .and_then(|bytes| terminal_codec::ClosedTerminalModule::from_semantic_bytes(&bytes))
        .is_err()
}

#[test]
fn a_stored_operation_consumes_an_owner_and_repackages_its_custody() {
    let module = saved_complete(SOURCE, 5);
    let mut lost_claims = module.clone();
    for contract in &mut lost_claims.execution.callables.contracts {
        contract.entry_claim_paths.clear();
    }
    assert!(
        refused(&lost_claims),
        "the stored machine must accept its owned .storage claim"
    );
    let mut lost_domains = module;
    let sites = &lost_domains.execution.packages.sites;
    let mut removed = false;
    for machine in &mut lost_domains.machines {
        for op in machine
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.operations)
        {
            if matches!(
                op.kind,
                terminal_psi::OperationKind::MoveStructuralValue { .. }
            ) && sites.iter().any(|site| {
                site.machine == machine.id
                    && site.operation == op.id
                    && matches!(
                        site.event,
                        terminal_psi::TerminalPackageEvent::Project { .. }
                    )
            }) && let terminal_psi::OperationResult::Structural(result) = &mut op.result
            {
                removed |= !result.projected_qualifications.is_empty();
                result.projected_qualifications.clear();
            }
        }
    }
    assert!(removed);
    assert!(
        refused(&lost_domains),
        "the opened owner must retain its resident domain"
    );
}

#[test]
fn a_stored_callback_returns_its_owner_to_another_callback_of_the_same_package() {
    let source = include_str!("fixtures/callables/run/some_owned_job/returned_owner.omg");
    let module = saved_complete(source, 7);
    let mut checked = 0;
    for operation in module
        .machines
        .iter()
        .flat_map(|machine| machine.operations())
    {
        if let terminal_psi::OperationKind::CallMachine { contract, .. } = operation.kind {
            let mut missing_returned_custody = module.clone();
            let advertised = missing_returned_custody
                .execution
                .callables
                .contracts
                .iter_mut()
                .find(|row| row.id == contract)
                .unwrap();
            assert!(!advertised.result_claim_paths.is_empty());
            advertised.result_claim_paths.clear();
            assert!(
                refused(&missing_returned_custody),
                "the callable must return its advertised custody"
            );
            checked += 1;
        }
    }
    assert_eq!(
        checked, 2,
        "both the returned owner and receipt carry storage"
    );
}

#[test]
fn the_copying_operation_refuses_an_owning_package() {
    let command = ["psi", "inspect", "--machine", "collect"];
    let (output, report) = program::invoke(SOURCE, &command);
    assert!(output.status.success(), "{report}");
    let (line, original) = SOURCE
        .lines()
        .enumerate()
        .find(|(_, line)| line.starts_with("machine collect("))
        .unwrap();
    let copied = SOURCE.replace(original, &original.replace("u8", "Job"));
    let (output, report) = program::invoke(&copied, &command);
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains(
            "type argument `Job` for `T` has `[linear]` but the binder requires `[copy]`"
        ),
        "{report}"
    );
    assert!(
        report.contains(&format!("main.omg:{}:", line + 1)),
        "{report}"
    );
}
