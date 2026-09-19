//! Float meaning projection tests.

use super::{
    BlockId, CheckedFloatMeaningProjection, CheckedFloatMeaningProjectionError,
    CheckedFloatProjectionSource, CheckedProofOnlyValueType, DirectBlockFloatParameter,
    DirectMachineFloatParameter, DirectMachineFloatResult, FloatMeaningProjectionLoweringError,
    FloatMeaningProjectionOperation, FloatMeaningSource, FloatProjectionInput,
    FloatProjectionInputId, IeeeFloatFormat, MachineId, PrimitiveType, ProofValueId, ScalarType,
    TerminalMachine, TerminalMachineResult, lower_float_meaning_projection,
    resolve_direct_float_source_binding,
};
use checked_trees::{
    CheckedDirectBlockFloatParameter, CheckedDirectMachineFloatParameter,
    CheckedDirectMachineFloatResult, CheckedFloatProjectionInput, CheckedFloatProjectionInputId,
    CheckedProofValueDeclaration, CheckedProofValueId,
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

const CORE_FLOAT_MEANING: &str = "data FloatMeaning { }";
const CORE_PROJECTIONS: &str = r#"
        operator Float::meaning32(value: f32) -> FloatMeaning;
        operator Float::meaning64(value: f64) -> FloatMeaning;
    "#;

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
