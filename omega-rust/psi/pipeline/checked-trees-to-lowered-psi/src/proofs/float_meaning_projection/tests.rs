//! Float meaning projection tests.

use super::{
    BlockId, CheckedFloatMeaningProjection, CheckedFloatMeaningProjectionError,
    CheckedFloatProjectionSource, CheckedProofOnlyValueType, DirectBlockFloatParameter,
    DirectCallFloatResult, DirectMachineFloatParameter, DirectMachineFloatResult,
    FloatMeaningProjection, FloatMeaningProjectionLoweringError, FloatMeaningProjectionOperation,
    FloatMeaningSource, FloatProjectionInput, FloatProjectionInputId,
    FloatSemanticApplicationOperand, IeeeFloatFormat, MachineId, PrimitiveType, ProofOnlyValueType,
    ProofValueDeclaration, ProofValueId, ScalarType, TerminalMachine, TerminalMachineResult,
    lower_float_meaning_projection, rejoin_float_semantic_applications,
    resolve_direct_float_source_binding,
};
use checked_trees::{
    CheckedDirectBlockFloatParameter, CheckedDirectCallFloatResult,
    CheckedDirectMachineFloatParameter, CheckedDirectMachineFloatResult,
    CheckedFloatProjectionInput, CheckedFloatProjectionInputId, CheckedFloatSemanticApplication,
    CheckedFloatSemanticApplicationOperand, CheckedFloatUseSite, CheckedProofValueDeclaration,
    CheckedProofValueId,
};
use numerics::float_projection::FloatProjectionOperation;
use source::{SourceMap, SourceOrigin};
use source_files_to_tokens::Lexer;
use std::path::PathBuf;
use std::sync::Arc;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
use tokens_to_syntax_trees::{parse_syntax_trees_into_with_id, parse_syntax_trees_with_id};

fn checked_projection() -> CheckedFloatMeaningProjection {
    CheckedFloatMeaningProjection {
        result: CheckedProofValueDeclaration {
            id: CheckedProofValueId(4),
            value_type: CheckedProofOnlyValueType::FloatMeaning,
        },
        source: CheckedFloatProjectionSource::TransitionalInput(CheckedFloatProjectionInput {
            id: CheckedFloatProjectionInputId(9),
            primitive: PrimitiveType::F64,
        }),
        operation: FloatProjectionOperation::Meaning64,
        contract: FloatProjectionOperation::Meaning64.contract_identity(),
    }
}
#[test]
fn lowering_preserves_dense_identities_and_exact_format() {
    let lowered = lower_float_meaning_projection(checked_projection(), None).unwrap();
    assert_eq!(lowered.result.id, ProofValueId(4));
    assert_eq!(
        lowered.source,
        FloatMeaningSource::TransitionalInput(FloatProjectionInput {
            id: FloatProjectionInputId(9),
            format: IeeeFloatFormat::Binary64,
        })
    );
    assert_eq!(
        lowered.operation,
        FloatMeaningProjectionOperation::Meaning64
    );
    let expected = FloatProjectionOperation::Meaning64.contract_identity();
    assert_eq!(lowered.contract.format, expected.format);
    assert_eq!(lowered.contract.operation, expected.operation);
    assert_eq!(lowered.contract.declaration, expected.declaration);
    assert_eq!(lowered.contract.catalog_version, expected.catalog_version);
    assert_eq!(lowered.contract.commitment, expected.commitment);
}

#[test]
fn direct_machine_parameter_lowers_to_an_exact_terminal_binding_when_available() {
    let mut checked = checked_projection();
    checked.source =
        CheckedFloatProjectionSource::DirectMachineParameter(CheckedDirectMachineFloatParameter {
            owner_machine: symbols::SymbolHandle::from_arena_index(3),
            parameter: symbols::SymbolHandle::from_arena_index(5),
            fallback: CheckedFloatProjectionInput {
                id: CheckedFloatProjectionInputId(9),
                primitive: PrimitiveType::F64,
            },
        });
    let direct = DirectMachineFloatParameter {
        owner: semantic_vocabulary::MachineId::new(4).unwrap(),
        parameter: semantic_vocabulary::ValueId::new(6).unwrap(),
        format: IeeeFloatFormat::Binary64,
    };
    let lowered = lower_float_meaning_projection(
        checked,
        Some(FloatMeaningSource::DirectMachineParameter(direct)),
    )
    .unwrap();
    assert_eq!(
        lowered.source,
        FloatMeaningSource::DirectMachineParameter(direct)
    );
}

#[test]
fn direct_machine_parameter_retains_fallback_without_an_artifact_binding() {
    let mut checked = checked_projection();
    checked.source =
        CheckedFloatProjectionSource::DirectMachineParameter(CheckedDirectMachineFloatParameter {
            owner_machine: symbols::SymbolHandle::from_arena_index(3),
            parameter: symbols::SymbolHandle::from_arena_index(5),
            fallback: CheckedFloatProjectionInput {
                id: CheckedFloatProjectionInputId(9),
                primitive: PrimitiveType::F64,
            },
        });
    let lowered = lower_float_meaning_projection(checked, None).unwrap();
    assert_eq!(
        lowered.source,
        FloatMeaningSource::TransitionalInput(FloatProjectionInput {
            id: FloatProjectionInputId(9),
            format: IeeeFloatFormat::Binary64,
        })
    );
}

#[test]
fn direct_machine_result_lowers_to_exact_terminal_binding_or_fallback() {
    let mut checked = checked_projection();
    checked.source =
        CheckedFloatProjectionSource::DirectMachineResult(CheckedDirectMachineFloatResult {
            owner_machine: symbols::SymbolHandle::from_arena_index(3),
            fallback: CheckedFloatProjectionInput {
                id: CheckedFloatProjectionInputId(9),
                primitive: PrimitiveType::F64,
            },
        });
    let direct = DirectMachineFloatResult {
        owner: semantic_vocabulary::MachineId::new(4).unwrap(),
        result: semantic_vocabulary::ValueId::new(8).unwrap(),
        format: IeeeFloatFormat::Binary64,
    };
    let lowered = lower_float_meaning_projection(
        checked.clone(),
        Some(FloatMeaningSource::DirectMachineResult(direct)),
    )
    .unwrap();
    assert_eq!(
        lowered.source,
        FloatMeaningSource::DirectMachineResult(direct)
    );

    assert_eq!(
        lower_float_meaning_projection(checked, None)
            .unwrap()
            .source,
        FloatMeaningSource::TransitionalInput(FloatProjectionInput {
            id: FloatProjectionInputId(9),
            format: IeeeFloatFormat::Binary64,
        })
    );
}

#[test]
fn direct_block_parameter_lowers_to_exact_terminal_binding_or_fallback() {
    let mut checked = checked_projection();
    checked.source =
        CheckedFloatProjectionSource::DirectBlockParameter(CheckedDirectBlockFloatParameter {
            owner_machine: symbols::SymbolHandle::from_arena_index(3),
            owner_state: symbols::SymbolHandle::from_arena_index(7),
            parameter: symbols::SymbolHandle::from_arena_index(9),
            fallback: CheckedFloatProjectionInput {
                id: CheckedFloatProjectionInputId(9),
                primitive: PrimitiveType::F64,
            },
        });
    let direct = DirectBlockFloatParameter {
        owner: semantic_vocabulary::MachineId::new(4).unwrap(),
        block: BlockId::new(2).unwrap(),
        parameter: semantic_vocabulary::ValueId::new(6).unwrap(),
        format: IeeeFloatFormat::Binary64,
    };
    let lowered = lower_float_meaning_projection(
        checked.clone(),
        Some(FloatMeaningSource::DirectBlockParameter(direct)),
    )
    .unwrap();
    assert_eq!(
        lowered.source,
        FloatMeaningSource::DirectBlockParameter(direct)
    );

    assert_eq!(
        lower_float_meaning_projection(checked, None)
            .unwrap()
            .source,
        FloatMeaningSource::TransitionalInput(FloatProjectionInput {
            id: FloatProjectionInputId(9),
            format: IeeeFloatFormat::Binary64,
        })
    );
}

#[test]
fn lowering_rejects_cross_format_checked_operation() {
    let mut checked = checked_projection();
    checked.operation = FloatProjectionOperation::Meaning32;
    assert_eq!(
        lower_float_meaning_projection(checked, None),
        Err(
            FloatMeaningProjectionLoweringError::InvalidCheckedProjection(
                CheckedFloatMeaningProjectionError::SourceFormatMismatch,
            )
        )
    );
}

#[test]
fn lowering_preserves_exact_literal_bits_without_a_producer_coordinate() {
    let mut checked = checked_projection();
    checked.source = CheckedFloatProjectionSource::ExactBinary64Literal(0x8000_0000_0000_0000);
    let lowered = lower_float_meaning_projection(checked, None).unwrap();
    assert_eq!(
        lowered.source,
        FloatMeaningSource::ExactBinary64Literal(0x8000_0000_0000_0000)
    );
}

/// A nested-state arrival contract projects its own scalar parameters.
/// Checked production supplies the `DirectBlockParameter` provenance here
/// through the real source-to-checked pipeline. Scalar-graph and composed
/// Unit admission still require each state contract to remain a pure
/// scalar qualification, so a meaning-equality contract on a nested state
/// has no emitted Terminal block yet: the test stages the checked graph
/// and emitted artifact that admission produces for this machine shape
/// and proves the resolver rejoins exact machine, block, and parameter
/// identity. Once the admission boundary widens, `lower_machine` also runs
/// the independent Terminal verifier over the same emitted rows.
#[test]
fn nested_state_contract_projects_an_exact_terminal_block_parameter() {
    let mut checked = checked_float_fixture(
        r#"
                machine state_source(selected: bool, seed: f64) -> bool
                requires
                    Float::meaning64(seed) == Float::meaning64(seed)
                ensures
                    true == true
                {
                    transition selected {
                        true -> inspect(seed)
                        false -> finish(selected)
                    }

                    state inspect(value: f64) -> bool
                    requires
                        Float::meaning64(value) == Float::meaning64(value)
                    { true }

                    state finish(result: bool) -> bool { result }
                }
            "#,
    );
    let projection = checked
        .facts
        .proof
        .float_meaning_projections
        .iter()
        .find(|projection| {
            matches!(
                projection.source,
                CheckedFloatProjectionSource::DirectBlockParameter(_)
            )
        })
        .cloned()
        .expect("the nested-state contract produces direct block provenance");
    let CheckedFloatProjectionSource::DirectBlockParameter(provenance) = projection.source else {
        unreachable!("found by source class");
    };
    let machine = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.symbol == provenance.owner_machine)
        .expect("the owner machine is authored");
    assert_eq!(
        checked.typed.symbols.display_path(machine.symbol, "::"),
        "state_source"
    );
    let states = checked.typed.machine_states(machine);
    assert_eq!(states.len(), 3);
    let inspect = states
        .iter()
        .find(|state| state.symbol == provenance.owner_state)
        .expect("the owner state is the authored nested state");
    assert_eq!(checked.typed.symbols.name(inspect.symbol), "inspect");
    let value = checked
        .typed
        .state_parameters(inspect)
        .iter()
        .find(|parameter| parameter.symbol == provenance.parameter)
        .expect("the projected parameter is the authored formal");
    assert_eq!(checked.typed.symbols.name(value.symbol), "value");
    assert_eq!(provenance.fallback.primitive, PrimitiveType::F64);

    // The scalar graph is the checked state-order and scalar-signature
    // evidence the resolver consumes; stage the rows admission produces
    // for this machine once the state-contract boundary widens.
    let graph_state =
        |state: &checked_trees::state::State| checked_trees::CheckedScalarStateGraph {
            erased_scalar_parameters: Vec::new(),
            erased_proof_parameters: Vec::new(),
            state: state.symbol,
            structural_parameters: Vec::new(),
            scalar_parameters: checked
                .typed
                .state_parameters(state)
                .iter()
                .enumerate()
                .map(
                    |(position, parameter)| checked_trees::CheckedStructuralScalarParameterPlan {
                        source_position: u32::try_from(position).unwrap(),
                        primitive_type: checked
                            .typed
                            .primitive_type_reference(parameter.type_reference)
                            .expect("fixture parameters are scalar"),
                    },
                )
                .collect(),
            parameter_types: Vec::new(),
            parameter_storage: arena::HandleSpan::default(),
            primitive_locals: Vec::new(),
            bindings: Vec::new(),
            unit_operations: Vec::new(),
            result_type: PrimitiveType::Bool,
            terminator: checked_trees::CheckedScalarStateTerminator::Return {
                statement_ordinal: 0,
            },
        };
    checked.facts.flow.terminal_scalar_graphs.machines.push(
        checked_trees::CheckedScalarMachineGraph {
            machine: machine.symbol,
            states: states.iter().map(graph_state).collect(),
            ranked_scc: None,
        },
    );

    // Mirror `build_scalar_graph_module` identity conventions: machine
    // scalar formals take the first value identities, each non-entry state
    // block follows the entry block by source-state position, and its
    // block parameters take the next dense value identities.
    let terminal_owner = MachineId::new(1).unwrap();
    let inspect_value = semantic_vocabulary::ValueId::new(3).unwrap();
    let block = |id: u64, parameters| terminal_psi::Block {
        id: BlockId::new(id).unwrap(),
        erased_scalar_formals: Vec::new(),
        erased_proof_formals: Vec::new(),
        parameters,
        structural_parameters: Vec::new(),
        operations: Vec::new(),
        terminator: terminal_psi::Terminator::ReturnUnit {
            edge: semantic_vocabulary::EdgeId::new(id).unwrap(),
            trivial_affine_discards: Vec::new(),
        },
    };
    let terminal_machine = TerminalMachine {
        id: terminal_owner,
        attachment: None,
        parameters: vec![
            terminal_psi::ValueDeclaration {
                id: semantic_vocabulary::ValueId::new(1).unwrap(),
                scalar_type: ScalarType::Boolean,
                qualifications: semantic_vocabulary::ScalarQualificationSetId::ZERO,
            },
            terminal_psi::ValueDeclaration {
                id: semantic_vocabulary::ValueId::new(2).unwrap(),
                scalar_type: ScalarType::IeeeFloat(IeeeFloatFormat::Binary64),
                qualifications: semantic_vocabulary::ScalarQualificationSetId::ZERO,
            },
        ],
        structural_parameters: Vec::new(),
        ranked_scc: None,
        result: TerminalMachineResult::Scalar(terminal_psi::ValueDeclaration {
            id: semantic_vocabulary::ValueId::new(5).unwrap(),
            scalar_type: ScalarType::Boolean,
            qualifications: semantic_vocabulary::ScalarQualificationSetId::ZERO,
        }),
        structural_places: Vec::new(),
        entry_claims: Vec::new(),
        declared_service_reach: Vec::new(),
        closed_reach_application: None,
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: BlockId::new(1).unwrap(),
        blocks: vec![
            block(1, Vec::new()),
            block(
                2,
                vec![terminal_psi::ValueDeclaration {
                    id: inspect_value,
                    scalar_type: ScalarType::IeeeFloat(IeeeFloatFormat::Binary64),
                    qualifications: semantic_vocabulary::ScalarQualificationSetId::ZERO,
                }],
            ),
            block(
                3,
                vec![terminal_psi::ValueDeclaration {
                    id: semantic_vocabulary::ValueId::new(4).unwrap(),
                    scalar_type: ScalarType::Boolean,
                    qualifications: semantic_vocabulary::ScalarQualificationSetId::ZERO,
                }],
            ),
        ],
        contract: terminal_psi::MachineContract {
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            id: crate::terminal_identities::contract_id(1),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    };
    let direct = resolve_direct_float_source_binding(
        &checked,
        &[(machine.symbol, terminal_owner)],
        &[terminal_machine],
        &[],
        &[],
        &[],
        projection.clone(),
    )
    .expect("the emitted artifact admits the exact binding")
    .expect("the owner machine is emitted");
    assert_eq!(
        direct,
        FloatMeaningSource::DirectBlockParameter(DirectBlockFloatParameter {
            owner: terminal_owner,
            block: BlockId::new(2).unwrap(),
            parameter: inspect_value,
            format: IeeeFloatFormat::Binary64,
        })
    );
    let lowered = lower_float_meaning_projection(projection, Some(direct.clone()))
        .expect("the resolved source lowers");
    assert_eq!(lowered.source, direct);
    assert_eq!(
        lowered.operation,
        FloatMeaningProjectionOperation::Meaning64
    );

    // Without the emitted owner the same checked provenance retains its
    // transitional fallback rather than binding to a foreign block.
    let unbound = resolve_direct_float_source_binding(
        &checked,
        &[],
        &[],
        &[],
        &[],
        &[],
        checked
            .facts
            .proof
            .float_meaning_projections
            .iter()
            .find(|projection| {
                matches!(
                    projection.source,
                    CheckedFloatProjectionSource::DirectBlockParameter(_)
                )
            })
            .expect("the provenance row remains")
            .clone(),
    )
    .expect("an unemitted owner is not an error");
    assert_eq!(unbound, None);
}

/// A callee's transported `ensures` instantiates at the call use: `result`
/// re-binds to the exact scalar the emitted call operation produces, so the
/// Terminal module carries a `DirectCallResult` source the verifier rejoins.
#[test]
fn transported_ensures_result_lowers_to_the_emitted_call_result() {
    let checked = checked_float_fixture(
        r#"
                machine helper(value: f32) -> f32
                requires Float::meaning32(value) == Float::meaning32(value);
                ensures Float::meaning32(result) == Float::meaning32(result);
                { value }

                machine caller(value: f32) -> f32
                requires Float::meaning32(value) == Float::meaning32(value);
                ensures Float::meaning32(result) == Float::meaning32(result);
                { helper(value) }
            "#,
    );
    let use_site_row = checked
        .facts
        .proof
        .float_meaning_projections
        .iter()
        .find(|projection| {
            matches!(
                projection.source,
                CheckedFloatProjectionSource::DirectCallResult(_)
            )
        })
        .expect("the imported ensures instantiates at the call site");
    let CheckedFloatProjectionSource::DirectCallResult(checked_result) = use_site_row.source else {
        unreachable!("found by source class")
    };
    assert_eq!(
        checked
            .typed
            .symbols
            .name(checked_result.use_site.owner_machine),
        "caller"
    );
    assert_eq!(checked_result.use_site.statement_index, 0);
    assert_eq!(checked_result.use_site.call_ordinal, 0);
    // The lowered scalar contract admits no meaning-equality clause yet, so
    // stage the join the pipeline produces: the caller's terminal machine
    // carries the exact Call operation, and the sidecar occurrence maps the
    // authored (state, statement, ordinal) coordinate onto it.
    let caller = checked
        .typed
        .machines()
        .iter()
        .find(|machine| checked.typed.symbols.name(machine.symbol) == "caller")
        .expect("caller machine");
    let helper = checked
        .typed
        .machines()
        .iter()
        .find(|machine| checked.typed.symbols.name(machine.symbol) == "helper")
        .expect("helper machine");
    let terminal_owner = MachineId::new(1).unwrap();
    let produced = semantic_vocabulary::ValueId::new(3).unwrap();
    let producer_id = semantic_vocabulary::OperationId::new(7).unwrap();
    let terminal_machine = TerminalMachine {
        id: terminal_owner,
        attachment: None,
        parameters: vec![terminal_psi::ValueDeclaration {
            id: semantic_vocabulary::ValueId::new(1).unwrap(),
            scalar_type: ScalarType::IeeeFloat(IeeeFloatFormat::Binary32),
            qualifications: semantic_vocabulary::ScalarQualificationSetId::ZERO,
        }],
        structural_parameters: Vec::new(),
        ranked_scc: None,
        result: TerminalMachineResult::Scalar(terminal_psi::ValueDeclaration {
            id: semantic_vocabulary::ValueId::new(4).unwrap(),
            scalar_type: ScalarType::IeeeFloat(IeeeFloatFormat::Binary32),
            qualifications: semantic_vocabulary::ScalarQualificationSetId::ZERO,
        }),
        structural_places: Vec::new(),
        entry_claims: Vec::new(),
        declared_service_reach: Vec::new(),
        closed_reach_application: None,
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: BlockId::new(1).unwrap(),
        blocks: vec![terminal_psi::Block {
            id: BlockId::new(1).unwrap(),
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            operations: vec![terminal_psi::Operation {
                static_reach_binding: None,
                id: producer_id,
                result: terminal_psi::OperationResult::Scalar(terminal_psi::ValueDeclaration {
                    id: produced,
                    scalar_type: ScalarType::IeeeFloat(IeeeFloatFormat::Binary32),
                    qualifications: semantic_vocabulary::ScalarQualificationSetId::ZERO,
                }),
                kind: terminal_psi::OperationKind::Call {
                    callee: MachineId::new(2).unwrap(),
                    arguments: vec![semantic_vocabulary::ValueId::new(1).unwrap()],
                    erased_arguments: Vec::new(),
                    erased_proof_arguments: Vec::new(),
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                },
            }],
            terminator: terminal_psi::Terminator::ReturnUnit {
                edge: semantic_vocabulary::EdgeId::new(1).unwrap(),
                trivial_affine_discards: Vec::new(),
            },
        }],
        contract: terminal_psi::MachineContract {
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            id: crate::terminal_identities::contract_id(1),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    };
    let occurrence = lowered_psi::LoweredSourceCallOccurrence {
        source_site: None,
        source_state: checked_result.use_site.owner_state,
        statement_index: checked_result.use_site.statement_index,
        call_ordinal: checked_result.use_site.call_ordinal,
        terminal_operation: producer_id,
        source_target: helper.symbol,
        source_values_before_call: Vec::new(),
    };
    let direct = resolve_direct_float_source_binding(
        &checked,
        &[(caller.symbol, terminal_owner)],
        std::slice::from_ref(&terminal_machine),
        &[],
        std::slice::from_ref(&occurrence),
        &[],
        use_site_row.clone(),
    )
    .expect("the emitted artifact admits the exact binding")
    .expect("the owner machine is emitted");
    assert_eq!(
        direct,
        FloatMeaningSource::DirectCallResult(DirectCallFloatResult {
            owner: terminal_owner,
            producer: producer_id,
            result: produced,
            format: IeeeFloatFormat::Binary32,
        })
    );
    let lowered = lower_float_meaning_projection(use_site_row.clone(), Some(direct.clone()))
        .expect("the resolved source lowers");
    assert_eq!(lowered.source, direct);

    // A non-call producer at the same coordinate rejects rather than rejoining
    // a result the call boundary does not own.
    let mut non_call_machine = terminal_machine.clone();
    non_call_machine.blocks[0].operations[0].kind =
        terminal_psi::OperationKind::BooleanConstant { value: true };
    assert!(
        resolve_direct_float_source_binding(
            &checked,
            &[(caller.symbol, terminal_owner)],
            &[non_call_machine],
            &[],
            std::slice::from_ref(&occurrence),
            &[],
            use_site_row.clone(),
        )
        .is_err(),
        "a non-call producer at the use-site coordinate must reject"
    );
    // An owner outside the emitted module, or an emitted owner with no
    // occurrence at the coordinate, retains the transitional fallback rather
    // than fabricating a producer.
    let unbound = resolve_direct_float_source_binding(
        &checked,
        &[],
        &[],
        &[],
        &[],
        &[],
        use_site_row.clone(),
    )
    .expect("an owner outside the emitted module is not an error");
    assert_eq!(unbound, None);
    let unjoined = resolve_direct_float_source_binding(
        &checked,
        &[(caller.symbol, terminal_owner)],
        &[terminal_machine],
        &[],
        &[],
        &[],
        use_site_row.clone(),
    )
    .expect("a missing occurrence is not an error");
    assert_eq!(unjoined, None);
}

/// Two call sites of the same callee produce two distinct call-result
/// identities; nothing collapses back to the callee's declaration row.
#[test]
fn transported_ensures_result_is_distinct_per_use_site() {
    let checked = checked_float_fixture(
        r#"
                machine helper(value: f32) -> f32
                ensures Float::meaning32(result) == Float::meaning32(result);
                { value }

                machine first(value: f32) -> f32
                ensures Float::meaning32(result) == Float::meaning32(result);
                { helper(value) }

                machine second(value: f32) -> f32
                ensures Float::meaning32(result) == Float::meaning32(result);
                { helper(value) }
            "#,
    );
    let sites = checked
        .facts
        .proof
        .float_meaning_projections
        .iter()
        .filter(|projection| {
            matches!(
                projection.source,
                CheckedFloatProjectionSource::DirectCallResult(_)
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(sites.len(), 2);
    assert_ne!(sites[0].source, sites[1].source);
    assert_ne!(sites[0].result.id, sites[1].result.id);
    let CheckedFloatProjectionSource::DirectCallResult(first) = sites[0].source else {
        unreachable!()
    };
    let CheckedFloatProjectionSource::DirectCallResult(second) = sites[1].source else {
        unreachable!()
    };
    assert_ne!(first.use_site.owner_machine, second.use_site.owner_machine);
}

/// A use-site coordinate with no emitted occurrence retains the transitional
/// fallback rather than synthesizing a producer it cannot rejoin.
#[test]
fn unjoined_call_result_falls_back_to_transitional_input() {
    let mut checked = checked_projection();
    checked.source = CheckedFloatProjectionSource::DirectCallResult(CheckedDirectCallFloatResult {
        use_site: CheckedFloatUseSite {
            owner_machine: symbols::SymbolHandle::from_arena_index(3),
            owner_state: symbols::SymbolHandle::from_arena_index(4),
            statement_index: 0,
            call_ordinal: 0,
        },
        fallback: CheckedFloatProjectionInput {
            id: CheckedFloatProjectionInputId(9),
            primitive: PrimitiveType::F64,
        },
    });
    let lowered = lower_float_meaning_projection(checked, None).unwrap();
    assert_eq!(
        lowered.source,
        FloatMeaningSource::TransitionalInput(FloatProjectionInput {
            id: FloatProjectionInputId(9),
            format: IeeeFloatFormat::Binary64,
        })
    );
}

fn transitional_projection_row(index: u32) -> FloatMeaningProjection {
    let contract = FloatProjectionOperation::Meaning32.contract_identity();
    FloatMeaningProjection {
        result: ProofValueDeclaration {
            id: ProofValueId(index),
            value_type: ProofOnlyValueType::FloatMeaning,
        },
        source: FloatMeaningSource::TransitionalInput(FloatProjectionInput {
            id: FloatProjectionInputId(index),
            format: IeeeFloatFormat::Binary32,
        }),
        operation: FloatMeaningProjectionOperation::Meaning32,
        contract: terminal_psi::FloatProjectionContractIdentity {
            format: contract.format,
            operation: contract.operation,
            declaration: contract.declaration,
            catalog_version: contract.catalog_version,
            commitment: contract.commitment,
        },
    }
}

fn add_application(
    result: u32,
    operands: Vec<CheckedFloatSemanticApplicationOperand>,
) -> CheckedFloatSemanticApplication {
    let add_row = numerics::float_semantics_catalog::FLOAT_SEMANTIC_OPERATIONS
        .iter()
        .find(|row| row.name == "add")
        .expect("the sealed add row exists");
    CheckedFloatSemanticApplication {
        result: CheckedProofValueId(result),
        contract: add_row.contract_identity(),
        format: IeeeFloatFormat::Binary32,
        operands,
    }
}

fn binary_add_operands() -> Vec<CheckedFloatSemanticApplicationOperand> {
    vec![
        CheckedFloatSemanticApplicationOperand::Format(IeeeFloatFormat::Binary32),
        CheckedFloatSemanticApplicationOperand::Meaning(CheckedProofValueId(0)),
        CheckedFloatSemanticApplicationOperand::Meaning(CheckedProofValueId(0)),
    ]
}

#[test]
fn semantic_application_rejoins_its_transitional_proof_value_row() {
    let mut projections = vec![
        transitional_projection_row(0),
        transitional_projection_row(1),
    ];
    rejoin_float_semantic_applications(
        &[add_application(1, binary_add_operands())],
        &mut projections,
    )
    .expect("rejoin");
    let FloatMeaningSource::SemanticApplication(application) = &projections[1].source else {
        panic!("the produced row carries the SemanticApplication carrier")
    };
    let expected = numerics::float_semantics_catalog::FLOAT_SEMANTIC_OPERATIONS
        .iter()
        .find(|row| row.name == "add")
        .expect("add row")
        .contract_identity();
    assert_eq!(application.contract.row, expected.row);
    assert_eq!(
        application.contract.catalog_version,
        expected.catalog_version
    );
    assert_eq!(application.contract.commitment, expected.commitment);
    assert_eq!(application.format, IeeeFloatFormat::Binary32);
    assert_eq!(
        application.operands,
        vec![
            FloatSemanticApplicationOperand::Format(IeeeFloatFormat::Binary32),
            FloatSemanticApplicationOperand::Meaning(ProofValueId(0)),
            FloatSemanticApplicationOperand::Meaning(ProofValueId(0)),
        ]
    );
    assert!(matches!(
        projections[0].source,
        FloatMeaningSource::TransitionalInput(_)
    ));
}

#[test]
fn semantic_application_rejoin_is_idempotent_for_a_shared_row() {
    let mut projections = vec![
        transitional_projection_row(0),
        transitional_projection_row(1),
    ];
    let application = add_application(1, binary_add_operands());
    rejoin_float_semantic_applications(
        &[application.clone(), application.clone()],
        &mut projections,
    )
    .expect("identical application rows rejoin idempotently");
    assert!(matches!(
        projections[1].source,
        FloatMeaningSource::SemanticApplication(_)
    ));
}

#[test]
fn semantic_application_rejects_a_row_outside_the_proof_value_table() {
    let mut projections = vec![transitional_projection_row(0)];
    let error = rejoin_float_semantic_applications(
        &[add_application(7, binary_add_operands())],
        &mut projections,
    )
    .expect_err("out-of-range result row fails");
    assert_eq!(
        error,
        FloatMeaningProjectionLoweringError::InvalidSemanticApplicationRow { result: 7 }
    );
}

/// The carrier's format is the row's source format, so an application whose
/// declared result format disagrees with the row's projection format is
/// refused by name at lowering rather than emitted for the verifier to
/// reject as a cross-format substitution.
#[test]
fn semantic_application_rejects_a_row_of_another_format() {
    let mut projections = vec![
        transitional_projection_row(0),
        transitional_projection_row(1),
    ];
    let mut application = add_application(1, binary_add_operands());
    application.format = IeeeFloatFormat::Binary64;
    application.operands[0] =
        CheckedFloatSemanticApplicationOperand::Format(IeeeFloatFormat::Binary64);
    assert_eq!(
        application.validate(),
        Ok(()),
        "the application replays on its own"
    );
    let error = rejoin_float_semantic_applications(&[application], &mut projections)
        .expect_err("a binary64 application cannot take a binary32 row");
    assert_eq!(
        error,
        FloatMeaningProjectionLoweringError::SemanticApplicationFormatMismatch { result: 1 }
    );
    assert!(
        matches!(
            projections[1].source,
            FloatMeaningSource::TransitionalInput(_)
        ),
        "the refused row keeps its transitional source"
    );
}

#[test]
fn semantic_application_rejects_a_row_with_a_resolved_source() {
    let mut projections = vec![
        transitional_projection_row(0),
        transitional_projection_row(1),
    ];
    projections[1].source = FloatMeaningSource::ExactBinary32Literal(0x3f800000);
    let error = rejoin_float_semantic_applications(
        &[add_application(1, binary_add_operands())],
        &mut projections,
    )
    .expect_err("a non-transitional row cannot take the application carrier");
    assert_eq!(
        error,
        FloatMeaningProjectionLoweringError::InvalidSemanticApplicationRow { result: 1 }
    );
}

#[test]
fn semantic_application_rejects_a_checked_row_that_fails_replay() {
    let mut projections = vec![
        transitional_projection_row(0),
        transitional_projection_row(1),
    ];
    let mut application = add_application(1, binary_add_operands());
    application.operands[1] =
        CheckedFloatSemanticApplicationOperand::Meaning(CheckedProofValueId(1));
    let error = rejoin_float_semantic_applications(&[application], &mut projections)
        .expect_err("a self-referential meaning operand fails catalog replay");
    assert!(matches!(
        error,
        FloatMeaningProjectionLoweringError::InvalidSemanticApplication { result: 1, .. }
    ));
}

const CORE_FLOAT_MEANING: &str = "data FloatMeaning { }";
const CORE_PROJECTIONS: &str = r#"
        operator Float::meaning32(value: f32) -> FloatMeaning;
        operator Float::meaning64(value: f64) -> FloatMeaning;
    "#;
/// `FloatMeaning` is public here because `FloatSemantics` machines name it in
/// their public interfaces.
const CORE_SEMANTIC_MEANING: &str = "pub data FloatMeaning { }";
/// The sealed toolchain `FloatFormat` record copied verbatim from
/// `source/library/core/float_format.omg`.
const CORE_FLOAT_FORMAT: &str = r#"
        pub data FloatSpecialValues [copy] {
            signed_zero: bool;
            subnormals: bool;
            infinity: bool;
            nan: bool;
        }
        pub data FloatFormat [copy] {
            radix: u32;
            precision: u32;
            minimum_normal_exponent: i32;
            maximum_normal_exponent: i32;
            minimum_subnormal_exponent: i32;
            specials: FloatSpecialValues;
            rounds_to_nearest_ties_to_even: bool;
        }
        pub const FloatFormat::BINARY32: FloatFormat = FloatFormat {
            radix: 2,
            precision: 24,
            minimum_normal_exponent: -126,
            maximum_normal_exponent: 127,
            minimum_subnormal_exponent: -149,
            specials: FloatSpecialValues {
                signed_zero: true,
                subnormals: true,
                infinity: true,
                nan: true,
            },
            rounds_to_nearest_ties_to_even: true,
        };
        pub const FloatFormat::BINARY64: FloatFormat = FloatFormat {
            radix: 2,
            precision: 53,
            minimum_normal_exponent: -1022,
            maximum_normal_exponent: 1023,
            minimum_subnormal_exponent: -1074,
            specials: FloatSpecialValues {
                signed_zero: true,
                subnormals: true,
                infinity: true,
                nan: true,
            },
            rounds_to_nearest_ties_to_even: true,
        };
    "#;
const CORE_SEMANTIC_PROJECTIONS: &str = r#"
        operator Float::meaning32(value: f32) -> FloatMeaning;
        operator Float::meaning64(value: f64) -> FloatMeaning;
        pub machine FloatSemantics::add(format: FloatFormat, left: FloatMeaning, right: FloatMeaning) -> FloatMeaning;
        pub machine FloatSemantics::multiply(format: FloatFormat, left: FloatMeaning, right: FloatMeaning) -> FloatMeaning;
    "#;

fn checked_semantic_fixture(source: &str) -> checked_trees::CheckedTrees {
    let mut sources = SourceMap::default();
    let meaning_source_id = sources
        .add_with_metadata(
            PathBuf::from("source/library/core/float_meaning.omg"),
            CORE_SEMANTIC_MEANING.to_owned(),
            PathBuf::from("source/library/core"),
            None,
            SourceOrigin::Toolchain,
        )
        .source_id;
    let format_source_id = sources
        .add_with_metadata(
            PathBuf::from("source/library/core/float_format.omg"),
            CORE_FLOAT_FORMAT.to_owned(),
            PathBuf::from("source/library/core"),
            None,
            SourceOrigin::Toolchain,
        )
        .source_id;
    let projection_source_id = sources
        .add_with_metadata(
            PathBuf::from("source/library/core/float_operations.omg"),
            CORE_SEMANTIC_PROJECTIONS.to_owned(),
            PathBuf::from("source/library/core"),
            None,
            SourceOrigin::Toolchain,
        )
        .source_id;
    let user_source_id = sources
        .add(
            PathBuf::from("tests/float_projection/main.omg"),
            source.to_owned(),
        )
        .source_id;
    let meaning_tokens = Lexer::new(CORE_SEMANTIC_MEANING)
        .tokenize()
        .expect("tokenize float meaning");
    let mut syntax = parse_syntax_trees_with_id(meaning_source_id, &meaning_tokens)
        .expect("parse float meaning");
    let format_tokens = Lexer::new(CORE_FLOAT_FORMAT)
        .tokenize()
        .expect("tokenize float format");
    parse_syntax_trees_into_with_id(&mut syntax, format_source_id, &format_tokens)
        .expect("parse float format");
    let projection_tokens = Lexer::new(CORE_SEMANTIC_PROJECTIONS)
        .tokenize()
        .expect("tokenize semantic projections");
    parse_syntax_trees_into_with_id(&mut syntax, projection_source_id, &projection_tokens)
        .expect("parse semantic projections");
    let user_tokens = Lexer::new(source).tokenize().expect("tokenize fixture");
    parse_syntax_trees_into_with_id(&mut syntax, user_source_id, &user_tokens)
        .expect("parse fixture");
    let resolved = resolve(ResolutionRequest {
        syntax: &syntax,
        sources: Some(Arc::new(sources)),
        top_level_bindings: Vec::new(),
    })
    .expect("resolve semantic fixture");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type semantic fixture");
    typed_trees_to_checked_trees::lower_typed_trees(typed).expect("check semantic fixture")
}

#[test]
fn semantic_application_lowers_to_the_terminal_carrier_end_to_end() {
    let checked = checked_semantic_fixture(
        r#"
            machine helper(left: f32, right: f32) -> f32
            requires
                Float::meaning32(left) == FloatSemantics::add(
                    FloatFormat::BINARY32,
                    Float::meaning32(left),
                    Float::meaning32(right)
                );
            { left }

            machine terminal_root(value: bool) -> bool
            requires
                true == true;
            ensures
                true == true;
            { value }
        "#,
    );
    assert_eq!(checked.facts.proof.float_semantic_applications.len(), 1);
    let lowered = crate::lower_machine(&checked, "terminal_root").expect("lower semantic fixture");
    let projections = &lowered.semantic_module.float_meaning_projections;
    let application_index = projections
        .iter()
        .position(|row| matches!(row.source, FloatMeaningSource::SemanticApplication(_)))
        .expect("the application's proof value carries the SemanticApplication source");
    let FloatMeaningSource::SemanticApplication(application) =
        &projections[application_index].source
    else {
        unreachable!()
    };
    let identity = numerics::float_semantics_catalog::FloatSemanticContractIdentity {
        row: application.contract.row,
        catalog_version: application.contract.catalog_version,
        commitment: application.contract.commitment,
    };
    let row =
        numerics::float_semantics_catalog::FloatSemanticOperation::for_contract_identity(&identity)
            .expect("the emitted contract rejoins the sealed catalog");
    assert_eq!(row.name, "add");
    assert_eq!(application.format, IeeeFloatFormat::Binary32);
    let [format_operand, left_operand, right_operand] = application.operands.as_slice() else {
        panic!("add spells its three catalog operands")
    };
    assert_eq!(
        *format_operand,
        FloatSemanticApplicationOperand::Format(IeeeFloatFormat::Binary32)
    );
    for operand in [left_operand, right_operand] {
        let FloatSemanticApplicationOperand::Meaning(value) = operand else {
            panic!("add's meaning operands name earlier proof rows")
        };
        assert!(
            (value.0 as usize) < application_index,
            "meaning operands stay well-founded behind the application row"
        );
    }
}

fn checked_float_fixture(source: &str) -> checked_trees::CheckedTrees {
    let mut sources = SourceMap::default();
    let meaning_source_id = sources
        .add_with_metadata(
            PathBuf::from("source/library/core/float_meaning.omg"),
            CORE_FLOAT_MEANING.to_owned(),
            PathBuf::from("source/library/core"),
            None,
            SourceOrigin::Toolchain,
        )
        .source_id;
    let projection_source_id = sources
        .add_with_metadata(
            PathBuf::from("source/library/core/float_operations.omg"),
            CORE_PROJECTIONS.to_owned(),
            PathBuf::from("source/library/core"),
            None,
            SourceOrigin::Toolchain,
        )
        .source_id;
    let user_source_id = sources
        .add(
            PathBuf::from("tests/float_projection/main.omg"),
            source.to_owned(),
        )
        .source_id;
    let meaning_tokens = Lexer::new(CORE_FLOAT_MEANING)
        .tokenize()
        .expect("tokenize float meaning");
    let mut syntax = parse_syntax_trees_with_id(meaning_source_id, &meaning_tokens)
        .expect("parse float meaning");
    let projection_tokens = Lexer::new(CORE_PROJECTIONS)
        .tokenize()
        .expect("tokenize projections");
    parse_syntax_trees_into_with_id(&mut syntax, projection_source_id, &projection_tokens)
        .expect("parse core projections");
    let user_tokens = Lexer::new(source).tokenize().expect("tokenize fixture");
    parse_syntax_trees_into_with_id(&mut syntax, user_source_id, &user_tokens)
        .expect("parse fixture");
    let resolved = resolve(ResolutionRequest {
        syntax: &syntax,
        sources: Some(Arc::new(sources)),
        top_level_bindings: Vec::new(),
    })
    .expect("resolve projection fixture");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type projection fixture");
    typed_trees_to_checked_trees::lower_typed_trees(typed).expect("check projection fixture")
}
