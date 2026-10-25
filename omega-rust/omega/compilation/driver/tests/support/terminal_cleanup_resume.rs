//! Portable corpus provision for resumable cleanup of a source-produced linear owner.
//! This adapter records the exact authored Console byte boundary. It is a
//! semantic test sink, not native Console/provider qualification.

use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, MachineId, ScalarType};
use std::sync::{Arc, Mutex};
use terminal_codec::CanonicalTerminalArtifact;
use terminal_fuel::{FuelChargeSite, TerminalFuelMeter};
use terminal_interpreter::{
    TerminalArtifactInterpretError, TerminalEffectRejection, TerminalExecutionResult,
    TerminalExecutionStatus, TerminalImportSignature, TerminalInstanceCloseError,
    TerminalInterpretError, TerminalProgram, TerminalRuntimeSupplyRequest, TerminalScalarValue,
    TerminalStructuralInputs, TerminalUnitImport,
};

const EXPECTED: &[u8] = b"OPBA\n";

/// A failed lifecycle assertion is a red fixture, not an aborted corpus.
pub(super) fn observe(artifact: &CanonicalTerminalArtifact) -> Result<(), String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| exercise(artifact))).map_err(
        |failure| {
            failure
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| {
                    failure
                        .downcast_ref::<&str>()
                        .map(|message| (*message).to_owned())
                })
                .unwrap_or_else(|| "cleanup resume provision failed".to_owned())
        },
    )
}

fn hook_sites(module: &terminal_psi::TerminalModule, machine: MachineId) -> Vec<FuelChargeSite> {
    let hook = module.machine(machine).expect("exact cleanup target");
    assert!(hook.nominal_cleanup_subject.is_some());
    let sites = hook
        .blocks
        .iter()
        .flat_map(|block| {
            block
                .operations
                .iter()
                .map(|operation| FuelChargeSite::Operation(operation.id))
                .chain(block.terminator.edges().map(FuelChargeSite::Edge))
        })
        .collect::<Vec<_>>();
    assert!(!sites.is_empty(), "retained executable nominal hook");
    sites
}

fn exercise(artifact: &CanonicalTerminalArtifact) {
    let program = TerminalProgram::borrow(artifact.view());
    let module = artifact.semantic_module();
    let terminal_psi::TargetMachineCallable::Machine(entry) = module.entry else {
        panic!("cleanup helper has an ordinary entry");
    };
    let cleanup_edges = module
        .machine(entry)
        .unwrap()
        .blocks
        .iter()
        .flat_map(|block| block.terminator.cleanup_rosters())
        .filter(|(_, actions)| {
            actions
                .iter()
                .any(|action| action.invoked_machine().is_some())
        })
        .map(|(edge, _)| FuelChargeSite::Edge(edge))
        .collect::<Vec<_>>();
    assert!(
        !cleanup_edges.is_empty(),
        "entry retains nominal edge cleanup"
    );
    // Inspect the actual source-produced cleanup partition. Type identities are
    // canonical semantic payloads, not readable declaration-path suffixes.
    let actions = module
        .machine(entry)
        .unwrap()
        .blocks
        .iter()
        .flat_map(|block| block.terminator.cleanup_rosters())
        .flat_map(|(_, actions)| actions)
        .filter_map(|action| match action {
            terminal_psi::TerminalAffineCleanupAction::InvokeNominal(action) => Some(action),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [parent, second, first] = actions.as_slice() else {
        panic!("one parent and two child nominal cleanup actions");
    };
    assert!(
        parent.retains_descendants && !second.retains_descendants && !first.retains_descendants
    );
    assert!(parent.path.is_empty() && parent.consumed_claims.is_empty());
    assert_eq!(parent.place, second.place);
    assert_eq!(parent.place, first.place);
    let terminal_psi::StructuralTypeShape::Record { fields, .. } = &module
        .structural_type(parent.structural_type)
        .unwrap()
        .shape
    else {
        panic!("parent retains its complete record");
    };
    assert_eq!(fields.len(), 2);
    assert_eq!(
        first.path,
        [terminal_psi::StructuralPathSegment::Field(
            fields[0].identity
        )]
    );
    assert_eq!(
        second.path,
        [terminal_psi::StructuralPathSegment::Field(
            fields[1].identity
        )]
    );
    assert_eq!(first.structural_type, second.structural_type);
    assert!(
        fields
            .iter()
            .all(|field| field.field_type.as_structural() == Some(first.structural_type))
    );
    assert!(
        module
            .structural_type(first.structural_type)
            .unwrap()
            .owns_linear_claim
    );
    assert_eq!(first.consumed_claims.len(), 1);
    assert_eq!(second.consumed_claims.len(), 1);
    assert_ne!(first.consumed_claims, second.consumed_claims);
    assert_eq!(first.cleanup_machine, second.cleanup_machine);
    for action in [parent, second, first] {
        assert_eq!(
            module
                .machine(action.cleanup_machine)
                .unwrap()
                .nominal_cleanup_subject,
            Some(action.structural_type)
        );
    }
    let parent_sites = hook_sites(module, parent.cleanup_machine);
    let child_sites = hook_sites(module, first.cleanup_machine);
    // The configured corpus host deliberately supplies one buffered byte sink.
    // Catalog availability alone neither selects that import nor binds it.
    let uninstalled = program.execution_requirements(None).unwrap();
    assert!(uninstalled.imports.is_empty() && uninstalled.external_completions.is_empty());
    let [slot] = uninstalled.missing_providers.as_slice() else {
        panic!("one exact uninstalled Console byte demand");
    };
    let declaration = module.boundary_machine(*slot).unwrap();
    assert_eq!(
        TerminalImportSignature::from(declaration),
        TerminalImportSignature {
            completion: terminal_psi::BoundaryCompletion::NormalReturn,
            scalar_parameters: vec![ScalarType::Integer(
                IntegerType::new(IntegerSign::Signed, 32).unwrap()
            )],
            structural_parameters: Vec::new(),
            result: terminal_psi::BoundaryMachineResult::Unit,
        }
    );
    assert!(matches!(
        program.instantiate(terminal_interpreter::TerminalInstantiation { scalar_arguments: &[], structural_inputs: TerminalStructuralInputs::default(), ..Default::default() }),
        Err(TerminalArtifactInterpretError::Execution(
            TerminalInterpretError::ProviderInstallationMissing(boundary)
        )) if boundary == *slot
    ));
    let installation = program
        .admit_runtime_supplies(TerminalRuntimeSupplyRequest {
            x86_flags: None,
            entry: None,
            installation: None,
            host_imports: std::slice::from_ref(slot),
            target_machines: None,
            target_layouts: None,
        })
        .expect("explicit corpus host byte-import policy");
    let installed = program.execution_requirements(Some(&installation)).unwrap();
    assert_eq!(installed.imports, [*slot]);
    assert!(installed.missing_providers.is_empty() && installed.external_completions.is_empty());
    let instantiate = |bytes: &Arc<Mutex<Vec<u8>>>| {
        let mut instance = program
            .instantiate(terminal_interpreter::TerminalInstantiation {
                scalar_arguments: &[],
                structural_inputs: TerminalStructuralInputs::default(),
                installation: Some(&installation),
                ..Default::default()
            })
            .expect("instantiate Unit cleanup helper");
        let slots = instance.unbound_imports().collect::<Vec<_>>();
        assert_eq!(
            slots,
            [*slot],
            "only the admitted exact byte slot is demanded"
        );
        let declaration = instance.import(slots[0]).unwrap();
        assert_eq!(
            declaration.scalar_parameters,
            [ScalarType::Integer(
                IntegerType::new(IntegerSign::Signed, 32).unwrap()
            )],
            "Console sink implements exactly one i32 argument"
        );
        let bytes = Arc::clone(bytes);
        let adapter = TerminalUnitImport::for_import(
            declaration,
            move |arguments: &[TerminalScalarValue]| {
                let [
                    TerminalScalarValue::Integer {
                        value: IntegerValue::Signed(byte),
                        ..
                    },
                ] = arguments
                else {
                    return Err(TerminalEffectRejection::new("expected one signed byte"));
                };
                let byte = u8::try_from(*byte)
                    .map_err(|_| TerminalEffectRejection::new("Console byte out of range"))?;
                bytes.lock().unwrap().push(byte);
                Ok(())
            },
        )
        .expect("exact normal Unit import with no structural arguments");
        instance
            .bind_import(slots[0], adapter)
            .expect("bind exact declared Console signature");
        instance
    };

    let reference_bytes = Arc::new(Mutex::new(Vec::new()));
    let mut reference = instantiate(&reference_bytes);
    let mut unbounded = TerminalFuelMeter::unbounded();
    let complete = TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit);
    assert_eq!(reference.invoke(&mut unbounded).unwrap(), complete);
    assert_eq!(reference_bytes.lock().unwrap().as_slice(), EXPECTED);

    let paused_bytes = Arc::new(Mutex::new(Vec::new()));
    let mut paused = instantiate(&paused_bytes);
    let mut meter = TerminalFuelMeter::with_allowance(0);
    let mut before_cleanup = false;
    let mut inside_parent_after_write = false;
    let mut inside_child_after_write = false;
    let mut finished = false;
    // One activation, funding exactly the next refused semantic site. The
    // reference work bounds the loop; this is not a sweep of separate budgets.
    for _ in 0..=unbounded.usage().total_units() {
        match paused.invoke(&mut meter).expect("resume checked cleanup") {
            TerminalExecutionStatus::SponsorExhausted(exhaustion) => {
                let prefix = paused_bytes.lock().unwrap().clone();
                assert!(
                    EXPECTED.starts_with(&prefix),
                    "unexpected effects: {prefix:?}"
                );
                assert!(!paused.is_finished());
                assert!(paused.result().is_none());
                assert_eq!(paused.close(), Err(TerminalInstanceCloseError::Busy));
                assert!(!paused.is_closed(), "Busy retains the live activation");
                let usage_before_retry = meter.usage().clone();
                let allowance_before_retry = meter.remaining_allowance();
                let schedule_before_retry = meter.schedule();
                let effects_before_retry = paused.effects().to_vec();
                assert_eq!(
                    paused.invoke(&mut meter).unwrap(),
                    TerminalExecutionStatus::SponsorExhausted(exhaustion)
                );
                assert_eq!(
                    meter.usage(),
                    &usage_before_retry,
                    "unfunded retry charges nothing"
                );
                assert_eq!(meter.remaining_allowance(), allowance_before_retry);
                assert_eq!(meter.schedule(), schedule_before_retry);
                assert_eq!(paused.effects(), effects_before_retry.as_slice());
                assert_eq!(
                    *paused_bytes.lock().unwrap(),
                    prefix,
                    "retry repeats no write"
                );
                assert!(!paused.is_finished());
                assert!(paused.result().is_none());
                before_cleanup |= prefix == b"O" && cleanup_edges.contains(&exhaustion.site);
                // P has executed in this exact parent hook while both linear
                // children remain; B has executed in this exact child hook
                // while A is still owed. A prior unrelated write cannot count.
                inside_parent_after_write |=
                    prefix == b"OP" && parent_sites.contains(&exhaustion.site);
                inside_child_after_write |=
                    prefix == b"OPB" && child_sites.contains(&exhaustion.site);
                let additional = exhaustion
                    .required_units
                    .checked_sub(exhaustion.remaining_units)
                    .filter(|units| *units != 0)
                    .expect("refused site needs positive additional allowance");
                meter.replenish(additional).expect("replenish next site");
            }
            status => {
                assert_eq!(status, complete);
                finished = true;
                break;
            }
        }
    }
    assert!(finished, "exact replenishment makes progress");
    assert!(before_cleanup, "pause before the ordered cleanup stream");
    assert!(
        inside_parent_after_write,
        "pause in parent after P, before B/A"
    );
    assert!(inside_child_after_write, "pause in child after B, before A");
    assert_eq!(paused_bytes.lock().unwrap().as_slice(), EXPECTED);
    assert!(paused.is_finished());
    assert_eq!(paused.result(), Some(&TerminalExecutionResult::Unit));
    assert_eq!(paused.effects(), reference.effects());
    assert_eq!(meter.usage(), unbounded.usage(), "no paid site repeats");
    let effects = paused.effects().to_vec();
    let mut empty_budget = TerminalFuelMeter::with_allowance(0);
    assert_eq!(paused.invoke(&mut empty_budget).unwrap(), complete);
    assert_eq!(empty_budget.usage().total_units(), 0);
    assert_eq!(paused.effects(), effects.as_slice());
    assert_eq!(paused_bytes.lock().unwrap().as_slice(), EXPECTED);
    // Successful normal Unit completion itself checks empty live claims and
    // frontier. No private claim-table/debug API is used by this SDK witness.
    paused.close().expect("completed instance closes");
    assert!(paused.is_closed());
    paused.close().expect("close remains idempotent");
    reference.close().expect("reference instance closes");
}
