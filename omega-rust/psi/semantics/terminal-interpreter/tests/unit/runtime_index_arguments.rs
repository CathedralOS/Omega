//! Runtime-selected elements inside a structural call argument.
//!
//! A `RuntimeIndex { index, obligation }` segment names a runtime scalar and
//! an obligation the carrying call owns; nothing in the segment states a
//! bound. The verifier reconstructs `index < extent` from the array the
//! prefix resolves to and a certificate must discharge it from the facts at
//! the call — here the caller's published `requires`. These tests run the
//! serialized artifact: a field may follow the runtime element
//! (`self.rows[i].item`) and a second runtime index may follow the first
//! (`self.grid[i][j]`). A bound the facts do not prove, a missing
//! certificate, a selector that is not an integer operand, and a runtime
//! element in an operation that does not resolve runtime projections all
//! reject.

use super::{
    AcceptTerminalEffects, AdmissionProfile, BindingRelevance, Block, CertificateEnvelope,
    EvidenceIdentity, EvidenceRoute, IntegerSign, IntegerType, IntegerValue, ModuleError,
    ObligationEvidence, Operation, OperationKind, OperationResult, ProofBundle, ProofNode,
    ProofRule, ProofSystemMarker, Proposition, ScalarTerm, ScalarType, StructuralAccess,
    StructuralArgument, StructuralFieldDeclaration, StructuralFieldType, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralPathSegment, StructuralPlaceDeclaration,
    StructuralTypeDeclaration, StructuralTypeShape, TerminalExecution, TerminalExecutionResult,
    TerminalExecutionStatus, TerminalMachine, TerminalMachineResult, TerminalModule,
    TerminalScalarValue, TerminalStructuralInputs, TerminalStructuralValue, Terminator,
    ValueDeclaration, VerificationError, block_id, contract_id, decode_module, edge_id,
    empty_contract, encode_module, encode_proof_section, machine_id, obligation_id, operation_id,
    place_id, structural_field_id, structural_type_id, unit_module, value_id, verify_module,
};
use terminal_fuel::TerminalFuelMeter;
use terminal_interpreter::TerminalStructuralScalarFieldValue;

const ITEM: u64 = 90;
const PAIR: u64 = 91;
const ROW: u64 = 92;
const ROWS: u64 = 93;
const GRID: u64 = 94;
const OWNER: u64 = 95;
const CALLER_ROOT: u64 = 95;
const CALLEE_ROOT: u64 = 96;
const ROW_SELECTOR: u64 = 40;
const COLUMN_SELECTOR: u64 = 41;

fn u64_type() -> IntegerType {
    IntegerType::new(IntegerSign::Unsigned, 64).unwrap()
}

fn i32_type() -> ScalarType {
    ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap())
}

fn field(id: u64, identity: &str, field_type: StructuralFieldType) -> StructuralFieldDeclaration {
    StructuralFieldDeclaration {
        id: structural_field_id(id),
        identity: identity.into(),
        relevance: BindingRelevance::Relevant,
        field_type,
    }
}

fn selector(raw: u64) -> ScalarTerm {
    ScalarTerm::value(value_id(raw), ScalarType::Integer(u64_type()))
}

fn literal(value: u128) -> ScalarTerm {
    ScalarTerm::integer(u64_type(), IntegerValue::Unsigned(value)).unwrap()
}

/// `selector <= maximum`, the published requirement a proof cites.
fn at_most(raw: u64, maximum: u128) -> Proposition {
    Proposition::LessOrEqual(selector(raw), literal(maximum))
}

fn runtime(raw: u64, obligation: u64) -> StructuralPathSegment {
    StructuralPathSegment::RuntimeIndex {
        index: value_id(raw),
        obligation: obligation_id(obligation),
    }
}

/// `Owner { rows: [Row; 3], grid: [[Item; 2]; 3] }` with `Row { item: Item }`
/// and `Item { value: i32 }`. The entry caller receives the owner and two u64
/// selectors, lends `path` to a callee as a shared `Item`, and returns the
/// callee's read of `value`.
fn runtime_index_module(
    path: Vec<StructuralPathSegment>,
    requires: Vec<Proposition>,
) -> TerminalModule {
    let parameter = |place, structural_type, access| StructuralParameterDeclaration {
        place,
        position: 0,
        is_self: true,
        structural_type,
        multiplicity: StructuralMultiplicity::Unrestricted,
        access,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let place = |id| StructuralPlaceDeclaration {
        id,
        kind: semantic_vocabulary::StructuralPlaceKind::Parameter {
            position: 0,
            is_self: true,
        },
    };
    let array = |element, length| StructuralTypeShape::FixedArray {
        element: structural_type_id(element),
        length,
    };
    let mut module = unit_module();
    module.structural_types = vec![
        StructuralTypeDeclaration {
            id: structural_type_id(ITEM),
            identity: "test::Item".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![field(1, "value", StructuralFieldType::Scalar(i32_type()))],
            },
        },
        StructuralTypeDeclaration {
            id: structural_type_id(PAIR),
            identity: "test::Pair".into(),
            shape: array(ITEM, 2),
        },
        StructuralTypeDeclaration {
            id: structural_type_id(ROW),
            identity: "test::Row".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![field(
                    1,
                    "item",
                    StructuralFieldType::Structural(structural_type_id(ITEM)),
                )],
            },
        },
        StructuralTypeDeclaration {
            id: structural_type_id(ROWS),
            identity: "test::Rows".into(),
            shape: array(ROW, 3),
        },
        StructuralTypeDeclaration {
            id: structural_type_id(GRID),
            identity: "test::Grid".into(),
            shape: array(PAIR, 3),
        },
        StructuralTypeDeclaration {
            id: structural_type_id(OWNER),
            identity: "test::Owner".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![
                    field(
                        1,
                        "rows",
                        StructuralFieldType::Structural(structural_type_id(ROWS)),
                    ),
                    field(
                        2,
                        "grid",
                        StructuralFieldType::Structural(structural_type_id(GRID)),
                    ),
                ],
            },
        },
    ];
    let caller = &mut module.machines[0];
    caller.attachment = Some(structural_type_id(OWNER));
    caller.parameters = [ROW_SELECTOR, COLUMN_SELECTOR]
        .into_iter()
        .map(|raw| ValueDeclaration {
            qualifications: Default::default(),
            id: value_id(raw),
            scalar_type: ScalarType::Integer(u64_type()),
        })
        .collect();
    caller.structural_parameters = vec![parameter(
        place_id(CALLER_ROOT),
        structural_type_id(OWNER),
        StructuralAccess::SharedBorrow,
    )];
    caller.structural_places = vec![place(place_id(CALLER_ROOT))];
    caller.contract.requires = requires;
    caller.result = TerminalMachineResult::Scalar(ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(3),
        scalar_type: i32_type(),
    });
    caller.blocks[0].operations = vec![Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: operation_id(1),
        result: OperationResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: value_id(2),
            scalar_type: i32_type(),
        }),
        kind: OperationKind::CallStructuralScalar {
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            callee: machine_id(CALLEE_ROOT),
            arguments: Vec::new(),
            structural_arguments: vec![StructuralArgument {
                place: place_id(CALLER_ROOT),
                path,
                access: StructuralAccess::SharedBorrow,
            }],
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    }];
    caller.blocks[0].terminator = Terminator::Return {
        edge: edge_id(1),
        value: value_id(2),
        cleanup_actions: Vec::new(),
    };
    module.machines.push(TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: machine_id(CALLEE_ROOT),
        attachment: Some(structural_type_id(ITEM)),
        parameters: Vec::new(),
        structural_parameters: vec![parameter(
            place_id(CALLEE_ROOT),
            structural_type_id(ITEM),
            StructuralAccess::SharedBorrow,
        )],
        ranked_scc: None,
        result: TerminalMachineResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: value_id(5),
            scalar_type: i32_type(),
        }),
        structural_places: vec![place(place_id(CALLEE_ROOT))],
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: block_id(CALLEE_ROOT),
        blocks: vec![Block {
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: block_id(CALLEE_ROOT),
            parameters: Vec::new(),
            operations: vec![Operation {
                static_reach_binding: None,
                suspension_crossing: None,
                id: operation_id(4),
                result: OperationResult::Scalar(ValueDeclaration {
                    qualifications: Default::default(),
                    id: value_id(4),
                    scalar_type: i32_type(),
                }),
                kind: OperationKind::IntegerStructuralField {
                    path: Vec::new(),
                    source: place_id(CALLEE_ROOT),
                    field: structural_field_id(1),
                },
            }],
            terminator: Terminator::Return {
                edge: edge_id(CALLEE_ROOT),
                value: value_id(4),
                cleanup_actions: Vec::new(),
            },
        }],
        contract: empty_contract(contract_id(CALLEE_ROOT)),
    });
    module
}

/// `self.rows[i].item`: a field follows the runtime element.
fn row_item_module(requires: Vec<Proposition>) -> TerminalModule {
    runtime_index_module(
        vec![
            StructuralPathSegment::Field("rows".into()),
            runtime(ROW_SELECTOR, 1),
            StructuralPathSegment::Field("item".into()),
        ],
        requires,
    )
}

/// `self.grid[i][j]`: a second runtime element follows the first.
fn grid_cell_module(requires: Vec<Proposition>) -> TerminalModule {
    runtime_index_module(
        vec![
            StructuralPathSegment::Field("grid".into()),
            runtime(ROW_SELECTOR, 1),
            runtime(COLUMN_SELECTOR, 2),
        ],
        requires,
    )
}

/// Discharge every reconstructed runtime-index obligation `selector < extent`
/// from the published requirement `selector <= extent - 1`.
fn certificates(module: &TerminalModule) -> ProofBundle {
    let sites = terminal_verifier::reconstruct_terminal_obligations(module).unwrap();
    let evidence = sites
        .obligations()
        .iter()
        .map(|site| {
            assert!(site.canonical_certificate);
            let Proposition::LessThan(subject, _) = &site.obligation.proposition else {
                panic!(
                    "an unsigned selector's bound is index < extent: {:?}",
                    site.obligation.proposition
                )
            };
            let requirement = site
                .requirements
                .iter()
                .position(|requirement| {
                    matches!(requirement, Proposition::LessOrEqual(left, _) if left == subject)
                })
                .expect("the selector's published requirement");
            ObligationEvidence {
                obligation: site.obligation.id,
                route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                    identity: EvidenceIdentity::new(site.obligation.id.get()).unwrap(),
                    proof_system_marker: ProofSystemMarker::CURRENT,
                    proof: ProofNode {
                        conclusion: site.obligation.proposition.clone(),
                        rule: ProofRule::IntegerOrderDiscreteness {
                            relation: Box::new(ProofNode {
                                conclusion: site.requirements[requirement].clone(),
                                rule: ProofRule::Assumption { index: requirement },
                            }),
                        },
                    },
                }),
            }
        })
        .collect();
    ProofBundle {
        evidence,
        ..ProofBundle::default()
    }
}

/// Seed `rows[k].item.value = 10 + k` and `grid[a][b].value = 100 + 10a + b`.
fn seeded_fields() -> Vec<TerminalStructuralScalarFieldValue> {
    let value = |value: i128| TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Signed, 32).unwrap(),
        value: IntegerValue::Signed(value),
    };
    let mut fields = (0..3)
        .map(|row| TerminalStructuralScalarFieldValue {
            argument_index: 0,
            path: vec![
                StructuralPathSegment::Field("rows".into()),
                StructuralPathSegment::FixedIndex(row),
                StructuralPathSegment::Field("item".into()),
            ],
            field: structural_field_id(1),
            value: value(10 + i128::from(row)),
        })
        .collect::<Vec<_>>();
    for row in 0..3 {
        for column in 0..2 {
            fields.push(TerminalStructuralScalarFieldValue {
                argument_index: 0,
                path: vec![
                    StructuralPathSegment::Field("grid".into()),
                    StructuralPathSegment::FixedIndex(row),
                    StructuralPathSegment::FixedIndex(column),
                ],
                field: structural_field_id(1),
                value: value(100 + 10 * i128::from(row) + i128::from(column)),
            });
        }
    }
    fields
}

/// Encode, decode and verify the artifact, then run it with the selectors.
fn run(module: &TerminalModule, bundle: &ProofBundle, row: u64, column: u64) -> i128 {
    let semantic = encode_module(module).unwrap();
    assert_eq!(decode_module(&semantic).unwrap(), *module);
    let proof = encode_proof_section(module, bundle).unwrap();
    let selectors = [row, column].map(|value| TerminalScalarValue::Integer {
        scalar_type: u64_type(),
        value: IntegerValue::Unsigned(u128::from(value)),
    });
    let fields = seeded_fields();
    let mut execution = TerminalExecution::start_artifact(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &selectors,
        TerminalStructuralInputs {
            arguments: &[TerminalStructuralValue {
                opaque_identity: 700,
                structural_type: structural_type_id(OWNER),
                qualifications: Vec::new(),
                path: Vec::new(),
            }],
            scalar_fields: &fields,
            ..Default::default()
        },
    )
    .expect("the verified artifact starts");
    match execution
        .resume(
            &mut TerminalFuelMeter::unbounded(),
            &mut AcceptTerminalEffects,
        )
        .unwrap()
    {
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(
            TerminalScalarValue::Integer {
                value: IntegerValue::Signed(value),
                ..
            },
        )) => value,
        other => panic!("the call returns the selected element's value: {other:?}"),
    }
}

fn rejection(module: &TerminalModule, bundle: &ProofBundle) -> VerificationError {
    verify_module(module, bundle, &AdmissionProfile::default())
        .expect_err("the runtime-index module rejects")
}

#[test]
fn a_field_follows_a_runtime_element() {
    let module = row_item_module(vec![at_most(ROW_SELECTOR, 2)]);
    let bundle = certificates(&module);
    verify_module(&module, &bundle, &AdmissionProfile::default()).unwrap();
    for row in 0..3 {
        assert_eq!(run(&module, &bundle, row, 0), 10 + i128::from(row));
    }
}

#[test]
fn a_runtime_element_follows_a_runtime_element() {
    let module = grid_cell_module(vec![Proposition::Conjunction(vec![
        at_most(ROW_SELECTOR, 2),
        at_most(COLUMN_SELECTOR, 1),
    ])]);
    // One obligation per segment, each bounding its own array's extent.
    let sites = terminal_verifier::reconstruct_terminal_obligations(&module).unwrap();
    let mut bounds = sites
        .obligations()
        .iter()
        .map(|site| (site.obligation.id, site.obligation.proposition.clone()))
        .collect::<Vec<_>>();
    bounds.sort_by_key(|(id, _)| *id);
    assert_eq!(
        bounds,
        vec![
            (
                obligation_id(1),
                Proposition::LessThan(selector(ROW_SELECTOR), literal(3))
            ),
            (
                obligation_id(2),
                Proposition::LessThan(selector(COLUMN_SELECTOR), literal(2))
            ),
        ]
    );
}

#[test]
fn nested_runtime_elements_execute_from_the_serialized_artifact() {
    let mut module = grid_cell_module(vec![at_most(ROW_SELECTOR, 2), at_most(COLUMN_SELECTOR, 1)]);
    module.machines[0].contract.requires.sort();
    let bundle = certificates(&module);
    for row in 0..3 {
        for column in 0..2 {
            assert_eq!(
                run(&module, &bundle, row, column),
                100 + 10 * i128::from(row) + i128::from(column)
            );
        }
    }
}

#[test]
fn a_bound_the_facts_do_not_prove_rejects() {
    // `i <= 3` admits a fourth row that does not exist: the only certificate
    // the facts support concludes `i < 4`, not the reconstructed `i < 3`.
    let module = row_item_module(vec![at_most(ROW_SELECTOR, 3)]);
    let sites = terminal_verifier::reconstruct_terminal_obligations(&module).unwrap();
    let [site] = sites.obligations() else {
        panic!("one runtime-index obligation")
    };
    let overstated = ProofBundle {
        evidence: vec![ObligationEvidence {
            obligation: site.obligation.id,
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(1).unwrap(),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: ProofNode {
                    conclusion: Proposition::LessThan(selector(ROW_SELECTOR), literal(4)),
                    rule: ProofRule::IntegerOrderDiscreteness {
                        relation: Box::new(ProofNode {
                            conclusion: site.requirements[0].clone(),
                            rule: ProofRule::Assumption { index: 0 },
                        }),
                    },
                },
            }),
        }],
        ..ProofBundle::default()
    };
    rejection(&module, &overstated);
}

#[test]
fn a_runtime_element_without_evidence_rejects() {
    let module = row_item_module(vec![at_most(ROW_SELECTOR, 2)]);
    rejection(&module, &ProofBundle::default());
}

#[test]
fn bounds_on_another_value_never_transfer() {
    // The contract bounds the column selector; the row selector owes its own.
    let module = row_item_module(vec![at_most(COLUMN_SELECTOR, 1)]);
    let sites = terminal_verifier::reconstruct_terminal_obligations(&module).unwrap();
    assert!(sites.obligations().iter().all(|site| site
        .requirements
        .iter()
        .all(|requirement| !matches!(requirement, Proposition::LessOrEqual(left, _) if *left == selector(ROW_SELECTOR)))));
    rejection(&module, &ProofBundle::default());
}

#[test]
fn a_selector_must_be_a_defined_integer_operand() {
    let invalid = |module: &TerminalModule| match verify_module(
        module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    ) {
        Err(VerificationError::Module(error)) => error,
        other => panic!("the selector rejects before evidence: {other:?}"),
    };
    // Not defined anywhere in the caller.
    let undefined = runtime_index_module(
        vec![
            StructuralPathSegment::Field("rows".into()),
            runtime(77, 1),
            StructuralPathSegment::Field("item".into()),
        ],
        Vec::new(),
    );
    assert!(matches!(
        invalid(&undefined),
        ModuleError::ValueUsedBeforeDefinition(_)
    ));
    // A Boolean selects no element.
    let mut boolean = row_item_module(Vec::new());
    boolean.machines[0].parameters[0].scalar_type = ScalarType::Boolean;
    assert!(matches!(
        invalid(&boolean),
        ModuleError::InvalidRuntimeIndex { .. }
    ));
}

#[test]
fn only_runtime_projecting_operations_carry_a_runtime_element() {
    // A static field store names one exact place; a runtime element in its
    // carrier path rejects instead of being read as some element.
    let mut module = row_item_module(vec![at_most(ROW_SELECTOR, 2)]);
    let caller = &mut module.machines[0];
    caller.structural_parameters[0].access = StructuralAccess::MutableBorrow;
    caller.blocks[0].operations.insert(
        0,
        Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: operation_id(9),
            result: OperationResult::Unit,
            kind: OperationKind::StructuralScalarFieldStore {
                destination: place_id(CALLER_ROOT),
                path: vec![
                    StructuralPathSegment::Field("rows".into()),
                    runtime(ROW_SELECTOR, 9),
                    StructuralPathSegment::Field("item".into()),
                ],
                field: structural_field_id(1),
                value: value_id(ROW_SELECTOR),
                range_obligation: None,
            },
        },
    );
    assert!(matches!(
        verify_module(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default()
        ),
        Err(VerificationError::Module(_))
    ));
}
