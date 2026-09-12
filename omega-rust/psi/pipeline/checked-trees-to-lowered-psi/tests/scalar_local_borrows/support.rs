use std::collections::{BTreeMap, BTreeSet};

use checked_trees::CheckedTrees;
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, MachineId, StructuralPlaceKind};
use terminal_fuel::{FuelChargeSite, TerminalFuelMeter};
use terminal_interpreter::{
    TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus, TerminalScalarValue,
    TerminalStructuralPrimitiveValue, TerminalStructuralValue,
};
use terminal_psi::{
    OperationKind, StructuralAccess, StructuralMultiplicity, TerminalMachineResult, TerminalModule,
};

pub fn checked(source: &str) -> CheckedTrees {
    let mut sources = source::SourceMap::default();
    let source_id = sources
        .add(std::path::PathBuf::from("main.omg"), source.to_owned())
        .source_id;
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize scalar local source");
    let mut syntax = syntax_trees::SyntaxTrees::new(source_id);
    tokens_to_syntax_trees::parse_syntax_trees_into_with_id(&mut syntax, source_id, &tokens)
        .expect("parse scalar locals");
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees_with_sources(
        &syntax,
        std::sync::Arc::new(sources),
    )
    .expect("resolve scalar locals with source/debug custody");
    let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type scalar locals");
    typed_trees_to_checked_trees::lower_typed_trees(typed)
        .unwrap_or_else(|diagnostics| panic!("check scalar locals: {diagnostics:#?}\n{source}"))
}

pub fn unsigned(value: u128) -> TerminalScalarValue {
    TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
        value: IntegerValue::Unsigned(value),
    }
}

pub struct Expectations {
    pub result: TerminalExecutionResult,
    pub machines: usize,
    pub locals: usize,
    pub reads: usize,
    pub stores: usize,
    pub stamp_calls: usize,
    pub local_owner_is_entry: bool,
    pub stamp_access: StructuralAccess,
}

impl Expectations {
    pub fn scalar(result: TerminalScalarValue, reads: usize) -> Self {
        Self {
            result: TerminalExecutionResult::Scalar(result),
            machines: 3,
            locals: 1,
            reads,
            stores: 2,
            stamp_calls: 2,
            local_owner_is_entry: true,
            stamp_access: StructuralAccess::MutableBorrow,
        }
    }
}

/// All fixtures are straight-line; every emitted operation must execute once
/// per invocation, including unused snapshots and repeated shared callees.
pub fn execute(
    source: &str,
    arguments: &[TerminalScalarValue],
    expected: Expectations,
) -> TerminalModule {
    let checked = checked(source);
    for machine in checked.machines() {
        assert_eq!(
            checked.machine_states(machine).len(),
            1,
            "no synthetic source states"
        );
    }
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "enter")
        .produce_artifact()
        .expect("scalar local root publishes its complete call closure");
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
    let profile = proof_admission::AdmissionProfile::default();
    terminal_verifier::verify_module(&module, &proof, &profile)
        .expect("canonical scalar local artifact verifies independently");
    assert_eq!(
        terminal_codec::encode_module(&module).unwrap(),
        artifact.semantic_bytes()
    );
    assert_eq!(
        module.machines.len(),
        expected.machines,
        "one body per source machine"
    );
    let root = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    assert_eq!(
        root.parameters.len(),
        arguments.len(),
        "no synthetic scalar inputs"
    );
    let is_unit = expected.result == TerminalExecutionResult::Unit;
    if is_unit {
        assert_eq!(root.result, TerminalMachineResult::Unit);
        assert_eq!(root.structural_parameters.len(), 1);
    } else {
        assert!(
            matches!(root.result, TerminalMachineResult::Scalar(_)),
            "real scalar root"
        );
        assert!(
            root.structural_parameters.is_empty(),
            "locals are not source arguments"
        );
        assert!(
            module
                .machines
                .iter()
                .all(|machine| matches!(machine.result, TerminalMachineResult::Scalar(_))),
            "no Unit wrapper"
        );
    }

    let mut local_places = BTreeSet::new();
    let mut local_count = 0;
    let mut reads = 0;
    let mut stores = 0;
    for machine in &module.machines {
        for operation in machine.blocks.iter().flat_map(|block| &block.operations) {
            match operation.kind {
                OperationKind::EstablishPrimitiveLocal { .. } => {
                    assert!(matches!(machine.result, TerminalMachineResult::Scalar(_)));
                    assert_eq!(
                        machine.id == root.id,
                        expected.local_owner_is_entry,
                        "the authored scalar body owns local storage"
                    );
                    local_count += 1;
                    let result = operation.result.structural().expect("real local place");
                    assert_eq!(result.multiplicity, StructuralMultiplicity::Unrestricted);
                    assert!(result.qualifications.is_empty());
                    assert!(result.projected_qualifications.is_empty());
                    assert!(result.claims.is_empty());
                    local_places.insert(result.place);
                    let declaration = machine
                        .structural_places
                        .iter()
                        .find(|place| place.id == result.place)
                        .expect("declared local root");
                    assert_eq!(
                        declaration.kind,
                        StructuralPlaceKind::OperationResult {
                            producer: operation.id,
                            structural_type: result.structural_type,
                        },
                        "establishment owns the exact operation-result place",
                    );
                }
                OperationKind::PrimitiveScalarRead { .. } => reads += 1,
                OperationKind::WriteOnlyPrimitiveStore { .. } => stores += 1,
                _ => {}
            }
        }
    }
    assert_eq!(local_count, expected.locals, "local establishment sites");
    assert_eq!(
        local_places.len(),
        expected.locals,
        "distinct local referent identities"
    );
    assert_eq!(reads, expected.reads, "fresh primitive observation sites");
    assert_eq!(
        stores, expected.stores,
        "non-observing primitive store sites"
    );

    let stamp_bodies = module
        .machines
        .iter()
        .filter(|machine| {
            matches!(machine.result, TerminalMachineResult::Scalar(_))
            && machine.structural_parameters.len() == 1
            && machine.blocks.iter().flat_map(|block| &block.operations).any(|operation| {
                matches!(operation.kind, OperationKind::WriteOnlyPrimitiveStore { destination, .. }
                    if destination == machine.structural_parameters[0].place)
            })
        })
        .collect::<Vec<_>>();
    assert_eq!(stamp_bodies.len(), 1, "exactly one shared stamp body");
    let stamp = stamp_bodies[0];
    assert_eq!(stamp.structural_parameters[0].access, expected.stamp_access);
    let stamp_calls = module.machines.iter().flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations).filter(|operation| {
            matches!(operation.kind, OperationKind::CallStructuralScalar { callee, .. } if callee == stamp.id)
        }).count();
    assert_eq!(
        stamp_calls, expected.stamp_calls,
        "distinct authored stamp calls"
    );

    let mut execution = if is_unit {
        let parameter = &root.structural_parameters[0];
        TerminalExecution::start_artifact_with_structural_arguments_and_primitive_values(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &profile,
            arguments,
            &[TerminalStructuralValue {
                opaque_identity: 71,
                structural_type: parameter.structural_type,
                qualifications: Vec::new(),
                path: Vec::new(),
            }],
            &[TerminalStructuralPrimitiveValue {
                argument_index: 0,
                value: unsigned(201),
            }],
        )
        .unwrap()
    } else {
        TerminalExecution::start_artifact(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &profile,
            arguments,
        )
        .expect("reload scalar root without external local backing")
    };
    let mut meter = TerminalFuelMeter::with_allowance(0);
    let mut complete = false;
    let mut output_observations = Vec::new();
    for _ in 0..256 {
        let status = execution
            .resume(&mut meter)
            .expect("resume scalar local execution");
        if is_unit {
            output_observations.push(execution.structural_primitive_values()[0].value);
        }
        match status {
            TerminalExecutionStatus::Complete(result) => {
                assert_eq!(result, expected.result, "{source}");
                complete = true;
                break;
            }
            TerminalExecutionStatus::SponsorExhausted(_) => meter.replenish(1).unwrap(),
            other => panic!("unexpected scalar local outcome: {other:?}"),
        }
    }
    assert!(
        complete,
        "scalar locals finish with one-unit replenishments"
    );
    if is_unit {
        output_observations.dedup();
        assert_eq!(
            output_observations,
            [unsigned(201), unsigned(11)],
            "Unit caller observes scalar helper result"
        );
    }
    assert_no_replay(&module, &meter, stamp.id, expected.stamp_calls as u64);
    module
}

fn assert_no_replay(
    module: &TerminalModule,
    meter: &TerminalFuelMeter,
    stamp: MachineId,
    stamp_invocations: u64,
) {
    let mut invocations = BTreeMap::from([(module.entry, 1)]);
    for operation in module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
    {
        let callee = match operation.kind {
            OperationKind::Call { callee, .. }
            | OperationKind::CallUnit { callee, .. }
            | OperationKind::CallStructuralScalar { callee, .. } => callee,
            _ => continue,
        };
        let usage = meter
            .usage()
            .at(FuelChargeSite::Operation(operation.id))
            .expect("authored call executes");
        *invocations.entry(callee).or_default() += usage.executions();
    }
    assert_eq!(invocations[&stamp], stamp_invocations);
    for machine in &module.machines {
        for operation in machine.blocks.iter().flat_map(|block| &block.operations) {
            let usage = meter
                .usage()
                .at(FuelChargeSite::Operation(operation.id))
                .expect("straight-line operation executes");
            assert_eq!(
                usage.executions(),
                invocations[&machine.id],
                "operation {:?} must not replay across suspension",
                operation.id
            );
            if matches!(
                operation.kind,
                OperationKind::EstablishPrimitiveLocal { .. }
                    | OperationKind::PrimitiveScalarRead { .. }
                    | OperationKind::WriteOnlyPrimitiveStore { .. }
                    | OperationKind::CallStructuralScalar { .. }
            ) {
                assert_eq!(
                    usage.units(),
                    usage.executions(),
                    "one unit per committed local operation or borrowed call"
                );
            }
        }
    }
}

pub fn publish_original(source: &str) -> CheckedTrees {
    let checked = checked(source);
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "enter")
        .produce_artifact()
        .expect("unmodified scalar local source must publish before custody mutations");
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
    terminal_verifier::verify_module(
        &module,
        &proof,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("unmodified scalar local publication independently verifies");
    checked
}

pub fn reject(checked: &CheckedTrees, mutation: &str) {
    assert!(
        terminal_production::TerminalProductionRequest::new(checked, "enter")
            .produce_artifact()
            .is_err(),
        "accepted scalar local custody mutation: {mutation}"
    );
}
