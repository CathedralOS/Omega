use checked_trees::CheckedTrees;
use semantic_vocabulary::{StructuralFieldId, StructuralTypeId};
use terminal_fuel::{FuelChargeSite, TerminalFuelMeter};
use terminal_interpreter::{
    TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus, TerminalScalarValue,
    TerminalStructuralBooleanFieldValue, TerminalStructuralValue,
};
use terminal_psi::{
    OperationKind, StructuralAccess, StructuralMultiplicity, StructuralTypeShape, TerminalModule,
};

pub fn check(source: &str) -> CheckedTrees {
    let mut sources = source::SourceMap::default();
    let source_id = sources
        .add(std::path::PathBuf::from("main.omg"), source.to_owned())
        .source_id;
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize owned scalar graph");
    let mut syntax = syntax_trees::SyntaxTrees::new(source_id);
    tokens_to_syntax_trees::parse_syntax_trees_into_with_id(&mut syntax, source_id, &tokens)
        .expect("parse owned scalar graph");
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees_with_sources(
        &syntax,
        std::sync::Arc::new(sources),
    )
    .expect("resolve with source map");
    let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type owned scalar graph");
    typed_trees_to_checked_trees::lower_typed_trees(typed).unwrap_or_else(|diagnostics| {
        panic!("check owned scalar graph: {diagnostics:#?}\n{source}")
    })
}

pub fn publish(source: &str, entry: &str) -> (CheckedTrees, TerminalModule, Vec<u8>, Vec<u8>) {
    let checked = check(source);
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, entry)
        .unwrap_or_else(|error| panic!("lower owned scalar graph {entry}: {error:?}\n{source}"));
    let debug = lowered
        .debug_map
        .expect("source-backed graph has a debug map");
    assert!(!debug.sites.is_empty());
    let debug_bytes = terminal_codec::encode_debug_map(&lowered.semantic_module, &debug).unwrap();
    assert_eq!(
        terminal_codec::decode_debug_map(&lowered.semantic_module, &debug_bytes).unwrap(),
        debug
    );
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, entry)
        .produce_artifact()
        .expect("publish owned scalar graph closure");
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
    terminal_verifier::verify_module(
        &module,
        &proof,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("independent canonical verification");
    assert_eq!(
        terminal_codec::encode_module(&module).unwrap(),
        artifact.semantic_bytes()
    );
    (
        checked,
        module,
        artifact.semantic_bytes().to_vec(),
        artifact.proof_bytes().to_vec(),
    )
}

pub fn field(
    module: &TerminalModule,
    structural_type: StructuralTypeId,
    identity: &str,
) -> StructuralFieldId {
    let declaration = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == structural_type)
        .expect("retained input type");
    let StructuralTypeShape::Record { fields } = &declaration.shape else {
        panic!("record input");
    };
    fields
        .iter()
        .find(|field| field.identity == identity)
        .expect("exact authored field")
        .id
}

pub fn execute(source: &str, entry: &str, left: bool, right: bool, expected: bool) {
    let (checked, module, semantic_bytes, proof_bytes) = publish(source, entry);
    for name in ["inspect", "enter"] {
        let machine = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap();
        let graph = checked
            .facts
            .flow
            .terminal_scalar_graphs
            .for_machine(machine.symbol)
            .expect("authored scalar graph");
        assert_eq!(graph.states.len(), 1);
        let state = &graph.states[0];
        let (scalars, structures) = if name == "inspect" {
            ([0, 2], [1, 3])
        } else {
            ([1, 3], [0, 2])
        };
        assert_eq!(
            state
                .scalar_parameters
                .iter()
                .map(|parameter| parameter.source_position)
                .collect::<Vec<_>>(),
            scalars
        );
        assert_eq!(
            state
                .structural_parameters
                .iter()
                .map(|parameter| parameter.position)
                .collect::<Vec<_>>(),
            structures
        );
    }
    let root = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    assert_eq!(
        module.machines.len(),
        if entry == "inspect" { 3 } else { 4 }
    );
    assert!(
        module.machines.iter().all(|machine| matches!(
            machine.result,
            terminal_psi::TerminalMachineResult::Scalar(_)
        )),
        "no Unit wrapper"
    );
    assert_eq!(root.parameters.len(), 2);
    assert_eq!(root.structural_parameters.len(), 2);
    assert_eq!(
        root.structural_parameters[0].structural_type,
        root.structural_parameters[1].structural_type
    );
    assert_ne!(
        root.structural_parameters[0].place,
        root.structural_parameters[1].place
    );
    let mut structural_arguments = Vec::new();
    let mut fields = Vec::new();
    for (argument_index, (parameter, value)) in root
        .structural_parameters
        .iter()
        .zip([left, right])
        .enumerate()
    {
        assert_eq!(parameter.access, StructuralAccess::Owned);
        assert_eq!(parameter.multiplicity, StructuralMultiplicity::Unrestricted);
        assert!(parameter.qualifications.is_empty());
        assert!(parameter.projected_qualifications.is_empty());
        structural_arguments.push(TerminalStructuralValue {
            opaque_identity: 71 + argument_index as u64,
            structural_type: parameter.structural_type,
            qualifications: Vec::new(),
            path: Vec::new(),
        });
        for (identity, value) in [("value", value), ("spare", !value)] {
            fields.push(TerminalStructuralBooleanFieldValue {
                argument_index: argument_index as u32,
                path: Vec::new(),
                field: field(&module, parameter.structural_type, identity),
                value,
            });
        }
    }
    let mut execution =
        TerminalExecution::start_artifact_with_structural_arguments_and_boolean_fields(
            &semantic_bytes,
            &proof_bytes,
            &proof_admission::AdmissionProfile::default(),
            &[
                TerminalScalarValue::Boolean(false),
                TerminalScalarValue::Boolean(true),
            ],
            &structural_arguments,
            &fields,
        )
        .expect("reload with distinct owned field values");
    let mut meter = TerminalFuelMeter::with_allowance(0);
    let mut complete = false;
    for _ in 0..256 {
        match execution
            .resume(&mut meter)
            .expect("execute owned scalar graph")
        {
            TerminalExecutionStatus::Complete(result) => {
                assert_eq!(
                    result,
                    TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(expected)),
                    "{entry}\n{source}"
                );
                complete = true;
                break;
            }
            TerminalExecutionStatus::SponsorExhausted(_) => meter.replenish(1).unwrap(),
            other => panic!("unexpected owned scalar outcome: {other:?}"),
        }
    }
    assert!(complete, "one-unit resumptions finish");
    let mut invocations = std::collections::BTreeMap::from([(module.entry, 1)]);
    for operation in module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
    {
        if let OperationKind::Call { callee, .. }
        | OperationKind::CallStructuralScalar { callee, .. } = operation.kind
        {
            let usage = meter
                .usage()
                .at(FuelChargeSite::Operation(operation.id))
                .expect("nested call executes");
            *invocations.entry(callee).or_default() += usage.executions();
        }
    }
    for machine in &module.machines {
        if machine
            .structural_parameters
            .iter()
            .any(|parameter| parameter.access == StructuralAccess::MutableBorrow)
        {
            assert_eq!(
                invocations[&machine.id], 2,
                "both nested stamp calls execute"
            );
        }
        for operation in machine.blocks.iter().flat_map(|block| &block.operations) {
            assert_eq!(
                meter
                    .usage()
                    .at(FuelChargeSite::Operation(operation.id))
                    .expect("straight-line operation executes")
                    .executions(),
                invocations[&machine.id],
                "no replay across fuel suspension"
            );
        }
    }
}

pub fn reject(checked: &CheckedTrees, mutation: &str) {
    assert!(
        terminal_production::TerminalProductionRequest::new(checked, "enter")
            .produce_artifact()
            .is_err(),
        "accepted owned scalar custody mutation: {mutation}"
    );
}
