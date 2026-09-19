//! Authored borrows retain reference ABI placement after canonical Psi checking.

use super::{
    AbstractBlockEntry, AbstractFunction, AbstractFunctionResult, AbstractOperation,
    AbstractOperationPlan, AbstractParameter, BlockId, EdgeId, IntegerSign, IntegerType,
    IntegerValue, ScalarType, StructuralAccess, StructuralArgument, StructuralFieldDeclaration,
    StructuralFieldId, StructuralFieldType, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralTypeDeclaration, StructuralTypeShape, ValueId,
};
use calling_conventions::{ValueClass, ValueLocation};
use proof_admission::AdmissionProfile;
use semantic_vocabulary::{MachineId, OperationId, PlaceId, StructuralTypeId};
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
use target::NativeTarget;
use target_operations::{TargetOperationPlan, TargetStructuralArgument, TargetUnitOperation};
use terminal_codec::{encode_module, encode_proof_section};
use terminal_psi_to_abstract_operations::lower_artifact;
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees_to_checked_trees::lower_typed_trees;

pub(super) fn source_plan(source: &str) -> abstract_operations::AbstractOperationPlan {
    let tokens = Lexer::new(source).tokenize().expect("tokenize source");
    let syntax = parse_syntax_trees(&tokens).expect("parse source");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve source");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type source");
    let checked = lower_typed_trees(typed).expect("check source");
    let terminal =
        checked_trees_to_lowered_psi::lower_machine(&checked, "Main::run").expect("lower source");
    lower_artifact(
        terminal_psi_to_abstract_operations::ArtifactSections {
            semantic_bytes: &encode_module(&terminal.semantic_module).expect("encode semantics"),
            proof_bytes: &encode_proof_section(&terminal.semantic_module, &terminal.proof_bundle)
                .expect("encode proof"),
            obligation_ledger_bytes: None,
        },
        &AdmissionProfile::default(),
    )
    .and_then(|admitted| admitted.try_into_plan())
    .expect("verify and lower canonical artifact")
}

#[test]
fn borrowed_unit_call_preserves_verified_requirement_obligations() {
    borrowed_call_requirement_custody(false);
}

#[test]
fn borrowed_scalar_call_preserves_verified_requirement_obligations() {
    borrowed_call_requirement_custody(true);
}

fn borrowed_call_requirement_custody(scalar_result: bool) {
    let source = source_plan(
        r#"
            data Main { value: u64; }
            machine Main::put(&mut self, value: u64)
            requires
                1 <= value;
                value <= 7;
            { self.value = value; }
            machine Main::get(&self, value: u64) -> u64
            requires
                1 <= value;
                value <= 7;
            { self.value }
            machine Main::run(&mut self) {
                self.put(3);
                let observed: u64 = self.get(3);
            }
        "#,
    );
    let (operation, obligations) = source
        .functions
        .iter()
        .flat_map(|function| &function.operations)
        .find_map(|operation| match operation {
            AbstractOperation::CallUnit {
                psi_operation,
                requirement_obligations,
                ..
            } if !scalar_result => Some((*psi_operation, requirement_obligations)),
            AbstractOperation::CallStructuralScalar {
                psi_operation,
                requirement_obligations,
                ..
            } if scalar_result => Some((*psi_operation, requirement_obligations)),
            _ => None,
        })
        .expect("authored borrowed call");
    assert!(obligations.len() >= 2, "both authored requirements survive");
    assert_ne!(obligations[0], obligations[1]);
    let select = |candidate: &TargetUnitOperation| match candidate {
        TargetUnitOperation::Call { psi_operation, .. } if !scalar_result => {
            *psi_operation == operation
        }
        TargetUnitOperation::StructuralScalarCall { psi_operation, .. } if scalar_result => {
            *psi_operation == operation
        }
        _ => false,
    };
    for native in [NativeTarget::macos_arm64(), NativeTarget::windows_x64()] {
        let target =
            crate::lower_to_target_operations(&source, crate::TargetLoweringRequest::new(native))
                .expect("verified receiver requirements lower");
        crate::validate_abstract_to_target_translation(&source, native, &target).unwrap();
        for mutation in 0..3 {
            let changed = mutate_call_row(&target, select, |call| {
                let (TargetUnitOperation::Call {
                    requirement_obligations,
                    ..
                }
                | TargetUnitOperation::StructuralScalarCall {
                    requirement_obligations,
                    ..
                }) = call
                else {
                    unreachable!("selected borrowed call")
                };
                assert_eq!(requirement_obligations, obligations);
                match mutation {
                    0 => requirement_obligations.clear(),
                    1 => {
                        requirement_obligations[0] = semantic_vocabulary::ObligationId::new(
                            obligations
                                .iter()
                                .map(|obligation| obligation.get())
                                .max()
                                .unwrap()
                                + 1,
                        )
                        .unwrap()
                    }
                    _ => requirement_obligations.swap(0, 1),
                }
            });
            assert!(
                crate::validate_abstract_to_target_translation(&source, native, &changed).is_err(),
                "{native:?}, scalar result {scalar_result}, obligation mutation {mutation}"
            );
        }
    }
}

#[test]
fn relevant_erased_record_fields_have_no_runtime_layout_or_scalar_access() {
    use semantic_vocabulary::StructuralFieldId;
    use terminal_psi::{
        BindingRelevance, StructuralFieldDeclaration, StructuralFieldType, StructuralTypeShape,
    };
    for scalar_storage in [true, false] {
        let mut source = source_plan(
            "data Main { value: i32; } machine Main::run(&mut self) { self.value = 65; }",
        );
        // This stage test retains an explicit receiver occurrence. Source-level
        // removal of an unused empty self is a separate correspondence contract.
        if !scalar_storage {
            let entry = source
                .functions
                .iter_mut()
                .find(|function| function.machine == source.entry)
                .unwrap();
            entry.operations.retain(|operation| {
                matches!(
                    operation,
                    abstract_operations::AbstractOperation::ReturnUnit { .. }
                )
            });
        }
        let entry = source
            .functions
            .iter()
            .find(|function| function.machine == source.entry)
            .unwrap();
        let receiver = entry.structural_parameters[0].structural_type;
        let erased_field = StructuralFieldId::new(99).unwrap();
        let declaration = source
            .structural_types
            .make_mut()
            .iter_mut()
            .find(|declaration| declaration.id == receiver)
            .unwrap();
        let StructuralTypeShape::Record { fields } = &mut declaration.shape else {
            panic!("receiver record");
        };
        if !scalar_storage {
            fields.clear();
        }
        fields.insert(
            0,
            StructuralFieldDeclaration {
                id: erased_field,
                identity: "service".into(),
                relevance: BindingRelevance::Relevant,
                field_type: StructuralFieldType::Erased {
                    type_identity: "FusedService".into(),
                },
            },
        );
        for target in [NativeTarget::macos_arm64(), NativeTarget::windows_x64()] {
            let lowered = crate::lower_to_target_operations(
                &source,
                crate::TargetLoweringRequest::new(target),
            )
            .expect("erased carrier occupies no bytes");
            crate::validate_abstract_to_target_translation(&source, target, &lowered)
                .expect("independent receiver shape replay");
            let entry = lowered
                .functions
                .iter()
                .find(|function| function.machine == source.entry)
                .unwrap();
            assert_eq!(
                entry.graph.parameters[0].shape.byte_size,
                if scalar_storage { 4 } else { 0 }
            );
            assert_eq!(
                entry.graph.parameters[0].shape.class,
                ValueClass::BorrowedReference
            );
            assert!(!entry.graph.parameters[0].placement.locations.is_empty());
            let mut changed = lowered.clone();
            let entry = changed
                .functions
                .iter_mut()
                .find(|function| function.machine == source.entry)
                .unwrap();
            entry.graph.parameters[0].shape.byte_size += 1;
            assert!(
                crate::validate_abstract_to_target_translation(&source, target, &changed).is_err()
            );
        }
        if scalar_storage {
            let entry = source
                .functions
                .iter_mut()
                .find(|function| function.machine == source.entry)
                .unwrap();
            let store = entry
                .operations
                .iter_mut()
                .find_map(|operation| match operation {
                    abstract_operations::AbstractOperation::StructuralScalarFieldStore {
                        field,
                        ..
                    } => Some(field),
                    _ => None,
                })
                .expect("scalar field store");
            *store = erased_field;
            assert!(
                crate::lower_to_target_operations(
                    &source,
                    crate::TargetLoweringRequest::new(NativeTarget::macos_arm64())
                )
                .is_err(),
                "runtime erasure grants no scalar access"
            );
        }
    }
}

fn shared_source() -> abstract_operations::AbstractOperationPlan {
    source_plan(
        r#"
            trait Read { machine read(&self) -> i32; }
            data Item { value: i32; }
            Reader: Item satisfies Read {
                machine read(&self) -> i32 { transition { _ -> self.value } }
            }
            data Main { item: Item; }
            machine Main::run(&self) {
                let reader: &dyn Read = &self.item as &dyn Item::Reader;
                let value: i32 = reader.read();
            }
        "#,
    )
}

fn projected_field_borrow_plan() -> abstract_operations::AbstractOperationPlan {
    let tally = StructuralTypeId::new(1).unwrap();
    let main = StructuralTypeId::new(2).unwrap();
    let unsigned = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let function = |raw, structural_type| AbstractFunction {
        machine: MachineId::new(raw).unwrap(),
        attachment: None,
        entry: BlockId::new(raw).unwrap(),
        parameters: Vec::new(),
        structural_parameters: vec![StructuralParameterDeclaration {
            place: PlaceId::new(raw).unwrap(),
            position: 0,
            is_self: false,
            structural_type,
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::MutableBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        }],
        result: AbstractFunctionResult::Unit,
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        block_entries: vec![AbstractBlockEntry {
            structural_parameters: Vec::new(),
            block: BlockId::new(raw).unwrap(),
            parameters: Vec::new(),
            operation_offset: 0,
        }],
        operations: vec![AbstractOperation::ReturnUnit {
            psi_edge: EdgeId::new(raw).unwrap(),
            cleanup_actions: Vec::new(),
        }],
    };
    let mut caller = function(1, main);
    let callee = function(2, tally);
    caller.operations.insert(
        0,
        AbstractOperation::CallUnit {
            psi_operation: OperationId::new(1).unwrap(),
            callee: callee.machine,
            arguments: Vec::new(),
            structural_arguments: vec![StructuralArgument {
                place: caller.structural_parameters[0].place,
                access: StructuralAccess::MutableBorrow,
                path: vec![terminal_psi::StructuralPathSegment::Field("tally".into())],
            }],
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    );
    AbstractOperationPlan {
        psi: super::support::identity(),
        entry: caller.machine,
        structural_types: vec![
            StructuralTypeDeclaration {
                id: tally,
                identity: "Tally".into(),
                shape: StructuralTypeShape::Record {
                    fields: vec![StructuralFieldDeclaration {
                        id: StructuralFieldId::new(1).unwrap(),
                        identity: "calls".into(),
                        relevance: terminal_psi::BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Scalar(unsigned),
                    }],
                },
            },
            StructuralTypeDeclaration {
                id: main,
                identity: "Main".into(),
                shape: StructuralTypeShape::Record {
                    fields: vec![StructuralFieldDeclaration {
                        id: StructuralFieldId::new(1).unwrap(),
                        identity: "tally".into(),
                        relevance: terminal_psi::BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Structural(tally),
                    }],
                },
            },
        ]
        .into(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![caller, callee],
    }
}

/// The same projected-field borrow with one runtime scalar argument, so the
/// retained call exercises the scalar prefix of the callee's plan too.
fn projected_field_borrow_scalar_argument_plan() -> abstract_operations::AbstractOperationPlan {
    let mut plan = projected_field_borrow_plan();
    let unsigned = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let caller_value = ValueId::new(40).unwrap();
    plan.functions[0].parameters.push(AbstractParameter {
        value: caller_value,
        scalar_type: unsigned,
    });
    plan.functions[1].parameters.push(AbstractParameter {
        value: ValueId::new(41).unwrap(),
        scalar_type: unsigned,
    });
    for operation in &mut plan.functions[0].operations {
        if let AbstractOperation::CallUnit { arguments, .. } = operation {
            arguments.push(caller_value);
        }
    }
    plan
}

/// The same projected borrow rooted at a caller-established record home
/// rather than a machine parameter. The referent's declaration comes from
/// the `EstablishRecord` results, so the retained argument must replay
/// against that home's own root type and byte offset.
fn established_home_borrow_plan() -> abstract_operations::AbstractOperationPlan {
    let mut plan = projected_field_borrow_plan();
    let tally = StructuralTypeId::new(1).unwrap();
    let main = StructuralTypeId::new(2).unwrap();
    let unsigned = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let tally_place = PlaceId::new(11).unwrap();
    let main_place = PlaceId::new(12).unwrap();
    let callee = plan.functions[1].machine;
    let caller = &mut plan.functions[0];
    caller.structural_parameters.clear();
    caller.operations = vec![
        AbstractOperation::IntegerConstant {
            psi_operation: OperationId::new(10).unwrap(),
            result: ValueId::new(10).unwrap(),
            scalar_type: unsigned,
            value: IntegerValue::Unsigned(7),
        },
        AbstractOperation::EstablishRecord {
            psi_operation: OperationId::new(11).unwrap(),
            result: terminal_psi::StructuralOperationResult {
                place: tally_place,
                structural_type: tally,
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            },
            fields: vec![terminal_psi::RecordFieldInitializer {
                field: StructuralFieldId::new(1).unwrap(),
                value: terminal_psi::RecordFieldValue::Scalar {
                    value: ValueId::new(10).unwrap(),
                    range_obligation: None,
                },
            }],
        },
        AbstractOperation::EstablishRecord {
            psi_operation: OperationId::new(12).unwrap(),
            result: terminal_psi::StructuralOperationResult {
                place: main_place,
                structural_type: main,
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            },
            fields: vec![terminal_psi::RecordFieldInitializer {
                field: StructuralFieldId::new(1).unwrap(),
                value: terminal_psi::RecordFieldValue::Structural(StructuralArgument {
                    place: tally_place,
                    access: StructuralAccess::Owned,
                    path: Vec::new(),
                }),
            }],
        },
        AbstractOperation::CallUnit {
            psi_operation: OperationId::new(13).unwrap(),
            callee,
            arguments: Vec::new(),
            structural_arguments: vec![StructuralArgument {
                place: main_place,
                access: StructuralAccess::MutableBorrow,
                path: vec![terminal_psi::StructuralPathSegment::Field("tally".into())],
            }],
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
        AbstractOperation::ReturnUnit {
            psi_edge: EdgeId::new(1).unwrap(),
            cleanup_actions: Vec::new(),
        },
    ];
    plan
}

/// A shared borrow of one element inside an owned caller array. The indexed
/// projection retains the root array's extent and element stride beside the
/// projected byte offset — metadata the validator must reconstruct from the
/// referent's declaration rather than accept from the retained row.
fn indexed_element_borrow_plan() -> abstract_operations::AbstractOperationPlan {
    let element = StructuralTypeId::new(1).unwrap();
    let array = StructuralTypeId::new(2).unwrap();
    let unsigned = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let function = |raw, structural_type, access, multiplicity| AbstractFunction {
        machine: MachineId::new(raw).unwrap(),
        attachment: None,
        entry: BlockId::new(raw).unwrap(),
        parameters: Vec::new(),
        structural_parameters: vec![StructuralParameterDeclaration {
            place: PlaceId::new(raw).unwrap(),
            position: 0,
            is_self: false,
            structural_type,
            multiplicity,
            access,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        }],
        result: AbstractFunctionResult::Unit,
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        block_entries: vec![AbstractBlockEntry {
            structural_parameters: Vec::new(),
            block: BlockId::new(raw).unwrap(),
            parameters: Vec::new(),
            operation_offset: 0,
        }],
        operations: vec![AbstractOperation::ReturnUnit {
            psi_edge: EdgeId::new(raw).unwrap(),
            cleanup_actions: Vec::new(),
        }],
    };
    let mut caller = function(
        1,
        array,
        StructuralAccess::Owned,
        StructuralMultiplicity::Affine,
    );
    let callee = function(
        2,
        element,
        StructuralAccess::SharedBorrow,
        StructuralMultiplicity::Unrestricted,
    );
    caller.operations.insert(
        0,
        AbstractOperation::CallUnit {
            psi_operation: OperationId::new(1).unwrap(),
            callee: callee.machine,
            arguments: Vec::new(),
            structural_arguments: vec![StructuralArgument {
                place: caller.structural_parameters[0].place,
                access: StructuralAccess::SharedBorrow,
                path: vec![terminal_psi::StructuralPathSegment::FixedIndex(2)],
            }],
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    );
    AbstractOperationPlan {
        psi: super::support::identity(),
        entry: caller.machine,
        structural_types: vec![
            StructuralTypeDeclaration {
                id: element,
                identity: "u64".into(),
                shape: StructuralTypeShape::PrimitiveScalar(unsigned),
            },
            StructuralTypeDeclaration {
                id: array,
                identity: "[u64; 4]".into(),
                shape: StructuralTypeShape::FixedArray { element, length: 4 },
            },
        ]
        .into(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![caller, callee],
    }
}

/// A caller that owns an aggregate actual, invokes a callee returning the
/// same structural type, and returns that result itself. The retained
/// `StructuralResultCall` must carry the declared result identity plus the
/// independently derived durable home.
fn structural_result_call_plan() -> abstract_operations::AbstractOperationPlan {
    let primitive = StructuralTypeId::new(1).unwrap();
    let array = StructuralTypeId::new(2).unwrap();
    let unsigned = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let caller_machine = MachineId::new(1).unwrap();
    let callee_machine = MachineId::new(2).unwrap();
    let result = |identity| terminal_psi::StructuralOperationResult {
        place: PlaceId::new(identity).unwrap(),
        structural_type: array,
        multiplicity: StructuralMultiplicity::Unrestricted,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        claims: Vec::new(),
    };
    let declaration = terminal_psi::StructuralResultDeclaration {
        reference_sources: Vec::new(),
        place: PlaceId::new(99).unwrap(),
        structural_type: array,
        multiplicity: StructuralMultiplicity::Unrestricted,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let return_structural = |source| AbstractOperation::ReturnStructural {
        psi_edge: EdgeId::new(1).unwrap(),
        source: PlaceId::new(source).unwrap(),
        returned_claims: Vec::new(),
        trivial_affine_locals: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    let call = |identity, source| AbstractOperation::CallStructural {
        psi_operation: OperationId::new(identity).unwrap(),
        result: result(identity),
        callee: callee_machine,
        arguments: Vec::new(),
        structural_arguments: vec![StructuralArgument {
            place: PlaceId::new(source).unwrap(),
            path: Vec::new(),
            access: StructuralAccess::Owned,
        }],
        claim_transfers: Vec::new(),
        returned_claim_transfers: Vec::new(),
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
        selected_evidence: Vec::new(),
    };
    let block = |raw| AbstractBlockEntry {
        structural_parameters: Vec::new(),
        block: BlockId::new(raw).unwrap(),
        parameters: Vec::new(),
        operation_offset: 0,
    };
    AbstractOperationPlan {
        psi: super::support::identity(),
        entry: caller_machine,
        structural_types: vec![
            StructuralTypeDeclaration {
                id: primitive,
                identity: "u64".into(),
                shape: StructuralTypeShape::PrimitiveScalar(unsigned),
            },
            StructuralTypeDeclaration {
                id: array,
                identity: "[u64; 2]".into(),
                shape: StructuralTypeShape::FixedArray {
                    element: primitive,
                    length: 2,
                },
            },
        ]
        .into(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![
            AbstractFunction {
                machine: caller_machine,
                attachment: None,
                entry: BlockId::new(1).unwrap(),
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
                result: AbstractFunctionResult::Structural(declaration.clone()),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![block(1)],
                operations: vec![
                    AbstractOperation::IntegerConstant {
                        psi_operation: OperationId::new(10).unwrap(),
                        result: ValueId::new(10).unwrap(),
                        scalar_type: unsigned,
                        value: IntegerValue::Unsigned(7),
                    },
                    AbstractOperation::EstablishScalarArray {
                        psi_operation: OperationId::new(11).unwrap(),
                        result: result(11),
                        elements: vec![ValueId::new(10).unwrap(), ValueId::new(10).unwrap()],
                    },
                    call(12, 11),
                    return_structural(12),
                ],
            },
            AbstractFunction {
                machine: callee_machine,
                attachment: None,
                entry: BlockId::new(2).unwrap(),
                parameters: Vec::new(),
                structural_parameters: vec![StructuralParameterDeclaration {
                    place: PlaceId::new(20).unwrap(),
                    position: 0,
                    is_self: false,
                    structural_type: array,
                    multiplicity: StructuralMultiplicity::Unrestricted,
                    access: StructuralAccess::Owned,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                }],
                result: AbstractFunctionResult::Structural(declaration),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![block(2)],
                operations: vec![return_structural(20)],
            },
        ],
    }
}

/// A caller that owns a record carrier with one reference leaf, moves it into
/// a callee that returns the same carrier, and returns the result itself. The
/// callee's declared result maps its `reference` leaf back to the owned
/// ingress parameter, so the retained `StructuralResultCall` must carry one
/// `reference_results` row resolved through caller custody: path
/// `[reference]`, root the caller's parameter place.
fn reference_result_call_plan() -> abstract_operations::AbstractOperationPlan {
    let primitive = StructuralTypeId::new(1).unwrap();
    let reference = StructuralTypeId::new(2).unwrap();
    let carrier = StructuralTypeId::new(3).unwrap();
    let unsigned = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let caller_machine = MachineId::new(1).unwrap();
    let callee_machine = MachineId::new(2).unwrap();
    let caller_place = PlaceId::new(1).unwrap();
    let callee_place = PlaceId::new(20).unwrap();
    let result_place = PlaceId::new(12).unwrap();
    let leaf_path = || {
        vec![terminal_psi::StructuralPathSegment::Field(
            "reference".into(),
        )]
    };
    let ingress = |place| StructuralArgument {
        place,
        path: vec![
            terminal_psi::StructuralPathSegment::Field("reference".into()),
            terminal_psi::StructuralPathSegment::Referent,
        ],
        access: StructuralAccess::MutableBorrow,
    };
    let parameter = |place| StructuralParameterDeclaration {
        place,
        position: 0,
        is_self: false,
        structural_type: carrier,
        multiplicity: StructuralMultiplicity::Affine,
        access: StructuralAccess::Owned,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let declaration = |place, source| terminal_psi::StructuralResultDeclaration {
        place,
        structural_type: carrier,
        multiplicity: StructuralMultiplicity::Affine,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        reference_sources: vec![terminal_psi::StructuralReferenceResultSource {
            path: leaf_path(),
            source,
        }],
    };
    let result = |place| terminal_psi::StructuralOperationResult {
        place,
        structural_type: carrier,
        multiplicity: StructuralMultiplicity::Affine,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        claims: Vec::new(),
    };
    let block = |raw| AbstractBlockEntry {
        structural_parameters: Vec::new(),
        block: BlockId::new(raw).unwrap(),
        parameters: Vec::new(),
        operation_offset: 0,
    };
    let return_structural = |raw, source| AbstractOperation::ReturnStructural {
        psi_edge: EdgeId::new(raw).unwrap(),
        source,
        returned_claims: Vec::new(),
        trivial_affine_locals: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    AbstractOperationPlan {
        psi: super::support::identity(),
        entry: caller_machine,
        structural_types: vec![
            StructuralTypeDeclaration {
                id: primitive,
                identity: "u64".into(),
                shape: StructuralTypeShape::PrimitiveScalar(unsigned),
            },
            StructuralTypeDeclaration {
                id: reference,
                identity: "&mut u64".into(),
                shape: StructuralTypeShape::Reference {
                    referent: primitive,
                    access: StructuralAccess::MutableBorrow,
                },
            },
            StructuralTypeDeclaration {
                id: carrier,
                identity: "Carrier".into(),
                shape: StructuralTypeShape::Record {
                    fields: vec![
                        StructuralFieldDeclaration {
                            id: StructuralFieldId::new(1).unwrap(),
                            identity: "payload".into(),
                            relevance: terminal_psi::BindingRelevance::Relevant,
                            field_type: StructuralFieldType::Scalar(unsigned),
                        },
                        StructuralFieldDeclaration {
                            id: StructuralFieldId::new(2).unwrap(),
                            identity: "reference".into(),
                            relevance: terminal_psi::BindingRelevance::Relevant,
                            field_type: StructuralFieldType::Structural(reference),
                        },
                    ],
                },
            },
        ]
        .into(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![
            AbstractFunction {
                machine: caller_machine,
                attachment: None,
                entry: BlockId::new(1).unwrap(),
                parameters: Vec::new(),
                structural_parameters: vec![parameter(caller_place)],
                result: AbstractFunctionResult::Structural(declaration(
                    PlaceId::new(97).unwrap(),
                    ingress(caller_place),
                )),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![block(1)],
                operations: vec![
                    AbstractOperation::CallStructural {
                        psi_operation: OperationId::new(12).unwrap(),
                        result: result(result_place),
                        callee: callee_machine,
                        arguments: Vec::new(),
                        structural_arguments: vec![StructuralArgument {
                            place: caller_place,
                            path: Vec::new(),
                            access: StructuralAccess::Owned,
                        }],
                        claim_transfers: Vec::new(),
                        returned_claim_transfers: Vec::new(),
                        requirement_obligations: Vec::new(),
                        crash_continuations: Vec::new(),
                        selected_evidence: Vec::new(),
                    },
                    return_structural(1, result_place),
                ],
            },
            AbstractFunction {
                machine: callee_machine,
                attachment: None,
                entry: BlockId::new(2).unwrap(),
                parameters: Vec::new(),
                structural_parameters: vec![parameter(callee_place)],
                result: AbstractFunctionResult::Structural(declaration(
                    PlaceId::new(98).unwrap(),
                    ingress(callee_place),
                )),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![block(2)],
                operations: vec![return_structural(2, callee_place)],
            },
        ],
    }
}

/// A descriptor stored into an aggregate field and later called through it.
/// The retained rows are one `StoreDynamicDescriptor` plus one
/// `StoredDynamicScalarCall` whose plan and source argument replay the
/// selected realization's single-parameter signature.
fn stored_descriptor_source() -> abstract_operations::AbstractOperationPlan {
    source_plan(
        r#"
            trait Measure { machine measure(&self) -> bool; }
            data Item [copy] { value: bool; }
            Primary: Item satisfies Measure {
                machine measure(&self) -> bool { transition { _ -> self.value } }
            }
            data Holder<'item> { handler: &'item dyn Measure; }
            data Main [copy] { item: Item; }
            machine Main::run<'item>(&self) {
                let erased: &'item dyn Measure = &self.item as &dyn Item::Primary;
                let holder: Holder<'item> = Holder { handler: erased };
                let result: bool = holder.handler.measure();
            }
        "#,
    )
}

/// A rebound descriptor call: the retained `DynamicScalarCall` carries both
/// the initializer and the latest source against the same realization plan.
fn rebound_descriptor_source() -> abstract_operations::AbstractOperationPlan {
    source_plan(
        r#"
            trait Measure {
                machine measure(&self) -> i32;
                machine alternate(&self) -> i32;
            }
            data Item { value: i32; }
            Primary: Item satisfies Measure {
                machine measure(&self) -> i32 { transition { _ -> self.value } }
                machine alternate(&self) -> i32 { transition { _ -> self.value } }
            }
            data Main { decoy: Item; selected: Item; }
            machine Main::run(&mut self) {
                let mut erased: &dyn Measure = &self.decoy as &dyn Item::Primary;
                erased = &self.selected as &dyn Item::Primary;
                let result: i32 = erased.measure();
            }
        "#,
    )
}

fn mutate_call_plan(
    target: &TargetOperationPlan,
    f: impl Fn(&mut calling_conventions::CallPlan),
) -> TargetOperationPlan {
    let mut target = target.clone();
    for function in &mut target.functions {
        for block in &mut function.graph.blocks {
            for operation in &mut block.operations {
                let call_plan = match operation {
                    TargetUnitOperation::Call { call_plan, .. }
                    | TargetUnitOperation::StructuralScalarCall { call_plan, .. }
                    | TargetUnitOperation::StructuralResultCall { call_plan, .. } => call_plan,
                    _ => continue,
                };
                f(call_plan);
            }
        }
    }
    target
}

fn mutate_scalar_arguments(
    target: &TargetOperationPlan,
    f: impl Fn(&mut Vec<target_operations::TargetUnitScalarCallArgument>),
) -> TargetOperationPlan {
    let mut target = target.clone();
    for function in &mut target.functions {
        for block in &mut function.graph.blocks {
            for operation in &mut block.operations {
                let arguments = match operation {
                    TargetUnitOperation::Call {
                        scalar_arguments, ..
                    }
                    | TargetUnitOperation::StructuralScalarCall {
                        scalar_arguments, ..
                    }
                    | TargetUnitOperation::StructuralResultCall {
                        scalar_arguments, ..
                    } => scalar_arguments,
                    _ => continue,
                };
                f(arguments);
            }
        }
    }
    target
}

fn mutate_call_arguments(
    target: &TargetOperationPlan,
    f: impl Fn(&mut TargetStructuralArgument),
) -> TargetOperationPlan {
    let mut target = target.clone();
    for function in &mut target.functions {
        for block in &mut function.graph.blocks {
            for operation in &mut block.operations {
                let arguments = match operation {
                    TargetUnitOperation::Call { arguments, .. }
                    | TargetUnitOperation::StructuralScalarCall { arguments, .. }
                    | TargetUnitOperation::StructuralResultCall { arguments, .. } => arguments,
                    _ => continue,
                };
                for argument in arguments {
                    f(argument);
                }
            }
        }
    }
    target
}

/// Mutate the first retained operation matching `select`. Call rows that the
/// generic argument/plan helpers do not destructure are edited in place.
fn mutate_call_row(
    target: &TargetOperationPlan,
    select: impl Fn(&TargetUnitOperation) -> bool,
    f: impl FnOnce(&mut TargetUnitOperation),
) -> TargetOperationPlan {
    let mut target = target.clone();
    for function in &mut target.functions {
        for block in &mut function.graph.blocks {
            if let Some(operation) = block
                .operations
                .iter_mut()
                .find(|operation| select(operation))
            {
                f(operation);
                return target;
            }
        }
    }
    panic!("no matching retained call row");
}

/// Append one forged operation to the end of one function's first block.
fn appended_call_row(
    target: &TargetOperationPlan,
    machine: MachineId,
    row: TargetUnitOperation,
) -> TargetOperationPlan {
    let mut target = target.clone();
    let function = target
        .functions
        .iter_mut()
        .find(|function| function.machine == machine)
        .expect("owning function");
    function.graph.blocks[0].operations.push(row);
    target
}

#[test]
fn projected_field_borrow_retains_borrowed_reference_and_validates() {
    for native in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let source = projected_field_borrow_plan();
        let target =
            crate::lower_to_target_operations(&source, crate::TargetLoweringRequest::new(native))
                .unwrap();
        crate::validate_abstract_to_target_translation(&source, native, &target).unwrap();
        let argument = target.functions[0]
            .graph
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .find_map(|operation| match operation {
                TargetUnitOperation::Call { arguments, .. } => arguments.first(),
                _ => None,
            })
            .unwrap();
        assert_eq!(
            argument.access,
            terminal_psi::StructuralAccess::MutableBorrow
        );
        assert_eq!(argument.shape.class, ValueClass::BorrowedReference);
        assert_eq!(argument.structural_type, StructuralTypeId::new(1).unwrap());
        assert_eq!(
            argument.root_structural_type,
            StructuralTypeId::new(2).unwrap()
        );
        assert_eq!(
            argument.path,
            vec![terminal_psi::StructuralPathSegment::Field("tally".into())]
        );
        assert_eq!(argument.source_byte_offset, 0);
        assert!(!argument.destination.locations.is_empty());
        assert!(
            argument
                .destination
                .locations
                .iter()
                .all(|location| matches!(location, ValueLocation::Indirect { .. }))
        );
    }
}

#[test]
fn projected_field_borrow_rejects_substituted_identity_access_shape_and_placement() {
    let source = projected_field_borrow_plan();
    let expected =
        crate::AbstractToTargetTranslationValidationError::StructuralCallArgumentMismatch {
            machine: MachineId::new(1).unwrap(),
            operation: OperationId::new(1).unwrap(),
        };
    for native in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target =
            crate::lower_to_target_operations(&source, crate::TargetLoweringRequest::new(native))
                .unwrap();
        for mutation in [
            Box::new(|argument: &mut TargetStructuralArgument| {
                argument.access = terminal_psi::StructuralAccess::SharedBorrow
            }) as Box<dyn Fn(&mut TargetStructuralArgument)>,
            Box::new(|argument: &mut TargetStructuralArgument| argument.path.clear()),
            Box::new(|argument: &mut TargetStructuralArgument| {
                argument.structural_type = argument.root_structural_type
            }),
            Box::new(|argument: &mut TargetStructuralArgument| {
                argument.shape.class = ValueClass::Integer
            }),
            Box::new(|argument: &mut TargetStructuralArgument| {
                argument.place = PlaceId::new(7).unwrap()
            }),
            Box::new(|argument: &mut TargetStructuralArgument| argument.source_byte_offset = 8),
            Box::new(|argument: &mut TargetStructuralArgument| {
                argument.root_structural_type = StructuralTypeId::new(1).unwrap()
            }),
            Box::new(|argument: &mut TargetStructuralArgument| {
                argument.destination.locations.clear()
            }),
        ] {
            let mutated = mutate_call_arguments(&target, mutation);
            assert_eq!(
                crate::validate_abstract_to_target_translation(&source, native, &mutated),
                Err(expected.clone())
            );
        }
    }
}

#[test]
fn projected_field_borrow_rejects_substituted_home_identity() {
    let source = projected_field_borrow_plan();
    let expected =
        crate::AbstractToTargetTranslationValidationError::StructuralCallArgumentMismatch {
            machine: MachineId::new(1).unwrap(),
            operation: OperationId::new(1).unwrap(),
        };
    for native in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target =
            crate::lower_to_target_operations(&source, crate::TargetLoweringRequest::new(native))
                .unwrap();
        for mutation in [
            Box::new(|argument: &mut TargetStructuralArgument| {
                argument.source =
                    target_operations::TargetStructuralArgumentSource::StructuralHome {
                        psi_operation: OperationId::new(1).unwrap(),
                    }
            }) as Box<dyn Fn(&mut TargetStructuralArgument)>,
            Box::new(|argument: &mut TargetStructuralArgument| {
                argument.source =
                    target_operations::TargetStructuralArgumentSource::EstablishedPrimitiveLocal {
                        psi_operation: OperationId::new(1).unwrap(),
                    }
            }),
            Box::new(|argument: &mut TargetStructuralArgument| {
                argument.source =
                    target_operations::TargetStructuralArgumentSource::EstablishedByteView {
                        psi_operation: OperationId::new(77).unwrap(),
                    }
            }),
            Box::new(|argument: &mut TargetStructuralArgument| {
                argument.source =
                    target_operations::TargetStructuralArgumentSource::BlockParameter {
                        block: semantic_vocabulary::BlockId::new(1).unwrap(),
                        place: argument.place,
                    }
            }),
            Box::new(|argument: &mut TargetStructuralArgument| {
                let target_operations::TargetStructuralArgumentSource::Placement(placement) =
                    &argument.source
                else {
                    panic!("caller parameter transport");
                };
                let mut forged = placement.clone();
                forged.locations.clear();
                argument.source = forged.into();
            }),
        ] {
            let mutated = mutate_call_arguments(&target, mutation);
            assert_eq!(
                crate::validate_abstract_to_target_translation(&source, native, &mutated),
                Err(expected.clone()),
                "substituted argument source"
            );
        }
    }
}

#[test]
fn established_home_borrow_retains_borrowed_reference_and_validates() {
    for native in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let source = established_home_borrow_plan();
        let target =
            crate::lower_to_target_operations(&source, crate::TargetLoweringRequest::new(native))
                .unwrap();
        crate::validate_abstract_to_target_translation(&source, native, &target).unwrap();
        let argument = target.functions[0]
            .graph
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .find_map(|operation| match operation {
                TargetUnitOperation::Call { arguments, .. } => arguments.first(),
                _ => None,
            })
            .unwrap();
        assert_eq!(
            argument.access,
            terminal_psi::StructuralAccess::MutableBorrow
        );
        assert_eq!(argument.shape.class, ValueClass::BorrowedReference);
        assert_eq!(
            argument.source,
            target_operations::TargetStructuralArgumentSource::StructuralHome {
                psi_operation: OperationId::new(12).unwrap()
            }
        );
        assert_eq!(
            argument.root_structural_type,
            StructuralTypeId::new(2).unwrap()
        );
        assert_eq!(argument.structural_type, StructuralTypeId::new(1).unwrap());
        assert_eq!(argument.source_byte_offset, 0);
    }
}

#[test]
fn established_home_borrow_rejects_substituted_root_projection_and_home() {
    let source = established_home_borrow_plan();
    let expected =
        crate::AbstractToTargetTranslationValidationError::StructuralCallArgumentMismatch {
            machine: MachineId::new(1).unwrap(),
            operation: OperationId::new(13).unwrap(),
        };
    for native in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target =
            crate::lower_to_target_operations(&source, crate::TargetLoweringRequest::new(native))
                .unwrap();
        for mutation in [
            // Referent root type substitution.
            Box::new(|argument: &mut TargetStructuralArgument| {
                argument.root_structural_type = StructuralTypeId::new(1).unwrap()
            }) as Box<dyn Fn(&mut TargetStructuralArgument)>,
            // A different byte offset inside the same established home.
            Box::new(|argument: &mut TargetStructuralArgument| argument.source_byte_offset = 8),
            // Forged array-transport metadata on a plain pointer carrier.
            Box::new(|argument: &mut TargetStructuralArgument| argument.element_stride = Some(8)),
            // A sibling establishment is not this referent's producer.
            Box::new(|argument: &mut TargetStructuralArgument| {
                argument.source =
                    target_operations::TargetStructuralArgumentSource::StructuralHome {
                        psi_operation: OperationId::new(11).unwrap(),
                    }
            }),
            // An established home is not a block arrival.
            Box::new(|argument: &mut TargetStructuralArgument| {
                argument.source =
                    target_operations::TargetStructuralArgumentSource::BlockParameter {
                        block: semantic_vocabulary::BlockId::new(1).unwrap(),
                        place: argument.place,
                    }
            }),
        ] {
            let mutated = mutate_call_arguments(&target, mutation);
            assert_eq!(
                crate::validate_abstract_to_target_translation(&source, native, &mutated),
                Err(expected.clone()),
                "substituted argument identity"
            );
        }
    }
}

#[test]
fn indexed_element_borrow_retains_array_transport_and_validates() {
    for native in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let source = indexed_element_borrow_plan();
        let target =
            crate::lower_to_target_operations(&source, crate::TargetLoweringRequest::new(native))
                .unwrap();
        crate::validate_abstract_to_target_translation(&source, native, &target).unwrap();
        let argument = target.functions[0]
            .graph
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .find_map(|operation| match operation {
                TargetUnitOperation::Call { arguments, .. } => arguments.first(),
                _ => None,
            })
            .unwrap();
        assert_eq!(
            argument.access,
            terminal_psi::StructuralAccess::SharedBorrow
        );
        // Borrowed references stay pointer-classified; the referent element's
        // extent and alignment ride the shared classifier, never a copy.
        assert_eq!(argument.shape.class, ValueClass::BorrowedReference);
        assert_eq!(argument.structural_type, StructuralTypeId::new(1).unwrap());
        assert_eq!(
            argument.root_structural_type,
            StructuralTypeId::new(2).unwrap()
        );
        assert_eq!(
            argument.path,
            vec![terminal_psi::StructuralPathSegment::FixedIndex(2)]
        );
        assert_eq!(argument.source_byte_offset, 16);
        assert_eq!(argument.fixed_array_length, Some(4));
        assert_eq!(argument.element_stride, Some(8));
        assert_eq!(
            argument.source,
            target_operations::TargetStructuralArgumentSource::Placement(
                target.functions[0].graph.parameters[0].placement.clone()
            )
        );
        assert!(!argument.destination.locations.is_empty());
    }
}

#[test]
fn indexed_element_borrow_rejects_substituted_projection_and_transport() {
    let source = indexed_element_borrow_plan();
    let expected =
        crate::AbstractToTargetTranslationValidationError::StructuralCallArgumentMismatch {
            machine: MachineId::new(1).unwrap(),
            operation: OperationId::new(1).unwrap(),
        };
    for native in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target =
            crate::lower_to_target_operations(&source, crate::TargetLoweringRequest::new(native))
                .unwrap();
        for mutation in [
            // A different element inside the same owned array home.
            Box::new(|argument: &mut TargetStructuralArgument| argument.source_byte_offset = 0)
                as Box<dyn Fn(&mut TargetStructuralArgument)>,
            Box::new(|argument: &mut TargetStructuralArgument| argument.source_byte_offset = 24),
            // The referent root is the array, not its element.
            Box::new(|argument: &mut TargetStructuralArgument| {
                argument.root_structural_type = StructuralTypeId::new(1).unwrap()
            }),
            // The projected carrier is the element, not the root array.
            Box::new(|argument: &mut TargetStructuralArgument| {
                argument.structural_type = StructuralTypeId::new(2).unwrap()
            }),
            // Dropped or fabricated array transport metadata.
            Box::new(|argument: &mut TargetStructuralArgument| argument.fixed_array_length = None),
            Box::new(|argument: &mut TargetStructuralArgument| {
                argument.fixed_array_length = Some(8)
            }),
            Box::new(|argument: &mut TargetStructuralArgument| argument.element_stride = None),
            Box::new(|argument: &mut TargetStructuralArgument| argument.element_stride = Some(16)),
            // Access, ABI class, and destination substitutions still reject.
            Box::new(|argument: &mut TargetStructuralArgument| {
                argument.access = terminal_psi::StructuralAccess::MutableBorrow
            }),
            Box::new(|argument: &mut TargetStructuralArgument| {
                argument.shape.class = ValueClass::Integer
            }),
            Box::new(|argument: &mut TargetStructuralArgument| {
                argument.destination.locations.clear()
            }),
            // An owned array parameter is not an operation-established home.
            Box::new(|argument: &mut TargetStructuralArgument| {
                argument.source =
                    target_operations::TargetStructuralArgumentSource::StructuralHome {
                        psi_operation: OperationId::new(1).unwrap(),
                    }
            }),
        ] {
            let mutated = mutate_call_arguments(&target, mutation);
            assert_eq!(
                crate::validate_abstract_to_target_translation(&source, native, &mutated),
                Err(expected.clone()),
                "substituted indexed projection"
            );
        }
    }
}

#[test]
fn projected_field_borrow_rejects_substituted_callee_plan() {
    let source = projected_field_borrow_plan();
    let expected =
        crate::AbstractToTargetTranslationValidationError::StructuralCallArgumentMismatch {
            machine: MachineId::new(1).unwrap(),
            operation: OperationId::new(1).unwrap(),
        };
    for native in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target =
            crate::lower_to_target_operations(&source, crate::TargetLoweringRequest::new(native))
                .unwrap();
        for mutation in [
            Box::new(|plan: &mut calling_conventions::CallPlan| {
                plan.stack_alignment = plan.stack_alignment.wrapping_add(8);
            }) as Box<dyn Fn(&mut calling_conventions::CallPlan)>,
            Box::new(|plan: &mut calling_conventions::CallPlan| {
                plan.result = Some(plan.parameters[0].clone());
            }),
            Box::new(|plan: &mut calling_conventions::CallPlan| {
                plan.parameters.push(plan.parameters[0].clone());
            }),
            Box::new(|plan: &mut calling_conventions::CallPlan| {
                plan.policy = calling_conventions::CallingPolicy::MicrosoftX64;
            }),
            Box::new(|plan: &mut calling_conventions::CallPlan| {
                plan.shadow_bytes = plan.shadow_bytes.wrapping_add(4);
            }),
        ] {
            let mutated = mutate_call_plan(&target, mutation);
            assert_eq!(
                crate::validate_abstract_to_target_translation(&source, native, &mutated),
                Err(expected.clone()),
                "embedded callee plan detail"
            );
        }
    }
}

#[test]
fn call_scalar_arguments_reject_substituted_identity_and_placement() {
    let source = projected_field_borrow_scalar_argument_plan();
    let expected =
        crate::AbstractToTargetTranslationValidationError::StructuralCallArgumentMismatch {
            machine: MachineId::new(1).unwrap(),
            operation: OperationId::new(1).unwrap(),
        };
    for native in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target =
            crate::lower_to_target_operations(&source, crate::TargetLoweringRequest::new(native))
                .unwrap();
        crate::validate_abstract_to_target_translation(&source, native, &target)
            .expect("honest scalar argument transport");
        for mutation in [
            Box::new(
                |arguments: &mut Vec<target_operations::TargetUnitScalarCallArgument>| {
                    arguments.clear();
                },
            ) as Box<dyn Fn(&mut Vec<target_operations::TargetUnitScalarCallArgument>)>,
            Box::new(
                |arguments: &mut Vec<target_operations::TargetUnitScalarCallArgument>| {
                    arguments[0].parameter_index = 9;
                },
            ),
            Box::new(
                |arguments: &mut Vec<target_operations::TargetUnitScalarCallArgument>| {
                    arguments[0].placement.locations.clear();
                },
            ),
            Box::new(
                |arguments: &mut Vec<target_operations::TargetUnitScalarCallArgument>| {
                    let target_operations::TargetUnitScalarArgumentSource::Parameter {
                        scalar_type,
                        ..
                    } = arguments[0].source
                    else {
                        panic!("caller parameter transport");
                    };
                    arguments[0].source =
                        target_operations::TargetUnitScalarArgumentSource::Parameter {
                            parameter_index: 0,
                            source_value: semantic_vocabulary::ValueId::new(99).unwrap(),
                            scalar_type,
                        };
                },
            ),
            Box::new(
                |arguments: &mut Vec<target_operations::TargetUnitScalarCallArgument>| {
                    arguments.push(arguments[0].clone());
                },
            ),
        ] {
            let mutated = mutate_scalar_arguments(&target, mutation);
            assert_eq!(
                crate::validate_abstract_to_target_translation(&source, native, &mutated),
                Err(expected.clone()),
                "scalar argument identity"
            );
        }
    }
}

#[test]
fn every_borrow_mode_uses_register_and_stack_pointers_without_value_copies() {
    use crate::lowering::structural_signature::StructuralCallSignature;
    use calling_conventions::IndirectPointerLocation;
    use std::collections::{BTreeMap, BTreeSet};
    use terminal_psi::StructuralAccess;

    let source = shared_source();
    let receiver = source
        .functions
        .iter()
        .flat_map(|function| &function.structural_parameters)
        .next()
        .unwrap();
    let declarations = source
        .structural_types
        .iter()
        .map(|declaration| (declaration.id, declaration))
        .collect::<BTreeMap<_, _>>();
    for access in [
        StructuralAccess::Owned,
        StructuralAccess::SharedBorrow,
        StructuralAccess::MutableBorrow,
        StructuralAccess::WriteOnlyBorrow,
    ] {
        let parameters = (0..12)
            .map(|position| {
                let mut parameter = receiver.clone();
                parameter.position = position;
                parameter.place =
                    semantic_vocabulary::PlaceId::new(u64::from(position) + 1).unwrap();
                parameter.is_self = false;
                parameter.access = access;
                parameter
            })
            .collect::<Vec<_>>();
        let signature = StructuralCallSignature::derive(
            &[],
            &parameters,
            None,
            &declarations,
            &mut BTreeMap::new(),
            &mut BTreeSet::new(),
        )
        .unwrap();
        for target in [
            NativeTarget::linux_x64(),
            NativeTarget::linux_arm64(),
            NativeTarget::windows_x64(),
            NativeTarget::macos_arm64(),
        ] {
            let plan = signature.plan(target).unwrap();
            let mut register_pointers = 0;
            let mut stack_pointers = 0;
            for placement in &plan.parameters {
                if access == StructuralAccess::Owned {
                    assert_eq!(placement.shape.class, ValueClass::Integer);
                    continue;
                }
                assert_eq!(placement.shape.class, ValueClass::BorrowedReference);
                match placement.locations.as_slice() {
                    [
                        ValueLocation::Indirect {
                            pointer,
                            copy_stack_byte_offset: None,
                            ..
                        },
                    ] => match pointer {
                        IndirectPointerLocation::Register(_) => register_pointers += 1,
                        IndirectPointerLocation::Stack { .. } => stack_pointers += 1,
                    },
                    locations => panic!("borrow must remain a pointer: {locations:?}"),
                }
            }
            if access != StructuralAccess::Owned {
                assert!(register_pointers > 0 && stack_pointers > 0, "{target:?}");
            }
        }
    }
}

#[test]
fn structural_result_call_replays_result_identity_home_and_plan() {
    for native in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let source = structural_result_call_plan();
        let target =
            crate::lower_to_target_operations(&source, crate::TargetLoweringRequest::new(native))
                .unwrap();
        crate::validate_abstract_to_target_translation(&source, native, &target).unwrap();
        let call = target.functions[0]
            .graph
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .find(|operation| matches!(operation, TargetUnitOperation::StructuralResultCall { .. }))
            .expect("retained structural result call");
        let TargetUnitOperation::StructuralResultCall {
            result,
            callee_result,
            result_home: Some(home),
            call_plan,
            ..
        } = call
        else {
            unreachable!("structural result row");
        };
        assert_eq!(result.place, PlaceId::new(12).unwrap());
        assert_eq!(result.structural_type, StructuralTypeId::new(2).unwrap());
        assert_eq!(callee_result.place, PlaceId::new(99).unwrap());
        assert_eq!(
            home.operation_result().map(|(operation, _)| operation),
            Some(OperationId::new(12).unwrap())
        );
        assert!(matches!(
            home.layout,
            target_operations::TargetStructuralHomeLayout::Aggregate(_)
        ));
        // One owned aggregate parameter occupies the only plan destination,
        // and the structural result still reserves a plan result placement.
        assert_eq!(call_plan.parameters.len(), 1);
        assert!(call_plan.result.is_some());
    }
}

#[test]
fn structural_result_call_rejects_substituted_result_home_and_plan() {
    let source = structural_result_call_plan();
    let expected =
        crate::AbstractToTargetTranslationValidationError::StructuralCallArgumentMismatch {
            machine: MachineId::new(1).unwrap(),
            operation: OperationId::new(12).unwrap(),
        };
    let is_result_call = |operation: &TargetUnitOperation| {
        matches!(operation, TargetUnitOperation::StructuralResultCall { .. })
    };
    let forged_declaration = || StructuralParameterDeclaration {
        place: PlaceId::new(9).unwrap(),
        position: 0,
        is_self: false,
        structural_type: StructuralTypeId::new(2).unwrap(),
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::Owned,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    for native in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target =
            crate::lower_to_target_operations(&source, crate::TargetLoweringRequest::new(native))
                .unwrap();
        for mutation in [
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::StructuralResultCall { callee, .. } = operation else {
                    unreachable!()
                };
                *callee = MachineId::new(77).unwrap();
            }) as Box<dyn FnOnce(&mut TargetUnitOperation)>,
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::StructuralResultCall { result, .. } = operation else {
                    unreachable!()
                };
                result.place = PlaceId::new(42).unwrap();
            }),
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::StructuralResultCall { result, .. } = operation else {
                    unreachable!()
                };
                result.structural_type = StructuralTypeId::new(1).unwrap();
            }),
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::StructuralResultCall { result, .. } = operation else {
                    unreachable!()
                };
                result.multiplicity = StructuralMultiplicity::Affine;
            }),
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::StructuralResultCall { callee_result, .. } = operation
                else {
                    unreachable!()
                };
                callee_result.structural_type = StructuralTypeId::new(1).unwrap();
            }),
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::StructuralResultCall { callee_result, .. } = operation
                else {
                    unreachable!()
                };
                callee_result.multiplicity = StructuralMultiplicity::Affine;
            }),
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::StructuralResultCall { result_home, .. } = operation
                else {
                    unreachable!()
                };
                *result_home = None;
            }),
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::StructuralResultCall { result_home, .. } = operation
                else {
                    unreachable!()
                };
                let home = result_home.as_mut().unwrap();
                home.origin = target_operations::TargetStructuralHomeOrigin::BlockParameter {
                    block: BlockId::new(9).unwrap(),
                    declaration: forged_declaration(),
                };
            }),
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::StructuralResultCall { result_home, .. } = operation
                else {
                    unreachable!()
                };
                let home = result_home.as_mut().unwrap();
                home.layout = target_operations::TargetStructuralHomeLayout::Aggregate(
                    calling_conventions::ValueShape::integer(4, 4),
                );
            }),
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::StructuralResultCall { arguments, .. } = operation else {
                    unreachable!()
                };
                arguments[0].access = StructuralAccess::SharedBorrow;
            }),
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::StructuralResultCall {
                    scalar_arguments,
                    call_plan,
                    ..
                } = operation
                else {
                    unreachable!()
                };
                scalar_arguments.push(target_operations::TargetUnitScalarCallArgument {
                    parameter_index: 0,
                    source: target_operations::TargetUnitScalarArgumentSource::Parameter {
                        parameter_index: 0,
                        source_value: ValueId::new(50).unwrap(),
                        scalar_type: ScalarType::Boolean,
                    },
                    placement: call_plan.parameters[0].clone(),
                });
            }),
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::StructuralResultCall {
                    requirement_obligations,
                    ..
                } = operation
                else {
                    unreachable!()
                };
                requirement_obligations.push(semantic_vocabulary::ObligationId::new(7).unwrap());
            }),
        ] {
            let mutated = mutate_call_row(&target, is_result_call, mutation);
            assert_eq!(
                crate::validate_abstract_to_target_translation(&source, native, &mutated),
                Err(expected.clone()),
                "substituted structural result row"
            );
        }
        for plan_mutation in [
            Box::new(|plan: &mut calling_conventions::CallPlan| {
                plan.stack_alignment = plan.stack_alignment.wrapping_add(8);
            }) as Box<dyn Fn(&mut calling_conventions::CallPlan)>,
            // A structural-result callee always has a result placement;
            // dropping it is not an equivalent plan on any target.
            Box::new(|plan: &mut calling_conventions::CallPlan| {
                plan.result = None;
            }),
            Box::new(|plan: &mut calling_conventions::CallPlan| {
                plan.policy = calling_conventions::CallingPolicy::MicrosoftX64;
            }),
        ] {
            let mutated = mutate_call_plan(&target, plan_mutation);
            assert_eq!(
                crate::validate_abstract_to_target_translation(&source, native, &mutated),
                Err(expected.clone()),
                "embedded callee plan detail"
            );
        }
    }
}

#[test]
fn structural_result_call_replays_reference_result_custody() {
    for native in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let source = reference_result_call_plan();
        let target =
            crate::lower_to_target_operations(&source, crate::TargetLoweringRequest::new(native))
                .unwrap();
        crate::validate_abstract_to_target_translation(&source, native, &target).unwrap();
        let call = target.functions[0]
            .graph
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .find(|operation| matches!(operation, TargetUnitOperation::StructuralResultCall { .. }))
            .expect("retained structural result call");
        let TargetUnitOperation::StructuralResultCall {
            result,
            reference_results,
            ..
        } = call
        else {
            unreachable!("structural result row");
        };
        assert_eq!(result.place, PlaceId::new(12).unwrap());
        // The moved owned carrier's `reference` leaf lands at the declared
        // result path and still suspends the caller's ingress root.
        assert_eq!(
            reference_results.as_slice(),
            [target_operations::TargetReferenceResult {
                path: vec![terminal_psi::StructuralPathSegment::Field(
                    "reference".into()
                )],
                root: PlaceId::new(1).unwrap(),
            }]
            .as_slice()
        );
    }
}

#[test]
fn structural_result_call_rejects_forged_reference_result_rows() {
    let source = reference_result_call_plan();
    let expected =
        crate::AbstractToTargetTranslationValidationError::StructuralCallArgumentMismatch {
            machine: MachineId::new(1).unwrap(),
            operation: OperationId::new(12).unwrap(),
        };
    let is_result_call = |operation: &TargetUnitOperation| {
        matches!(operation, TargetUnitOperation::StructuralResultCall { .. })
    };
    for native in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target =
            crate::lower_to_target_operations(&source, crate::TargetLoweringRequest::new(native))
                .unwrap();
        crate::validate_abstract_to_target_translation(&source, native, &target).unwrap();
        for mutation in [
            // A different suspended root: the caller parameter place is the
            // only honest referent identity.
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::StructuralResultCall {
                    reference_results, ..
                } = operation
                else {
                    unreachable!()
                };
                reference_results[0].root = PlaceId::new(77).unwrap();
            }) as Box<dyn FnOnce(&mut TargetUnitOperation)>,
            // A different leaf path inside the same result carrier.
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::StructuralResultCall {
                    reference_results, ..
                } = operation
                else {
                    unreachable!()
                };
                reference_results[0].path =
                    vec![terminal_psi::StructuralPathSegment::Field("payload".into())];
            }),
            // A deeper projection that no declared result leaf occupies.
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::StructuralResultCall {
                    reference_results, ..
                } = operation
                else {
                    unreachable!()
                };
                reference_results[0]
                    .path
                    .push(terminal_psi::StructuralPathSegment::FixedIndex(0));
            }),
            // A second forged row beside the honest one.
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::StructuralResultCall {
                    reference_results, ..
                } = operation
                else {
                    unreachable!()
                };
                let forged = reference_results[0].clone();
                reference_results.push(forged);
            }),
            // Dropping the roster entirely is not an equivalent row.
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::StructuralResultCall {
                    reference_results, ..
                } = operation
                else {
                    unreachable!()
                };
                reference_results.clear();
            }),
        ] {
            let mutated = mutate_call_row(&target, is_result_call, mutation);
            assert_eq!(
                crate::validate_abstract_to_target_translation(&source, native, &mutated),
                Err(expected.clone()),
                "forged reference result row"
            );
        }
    }
}

#[test]
fn embedded_scalar_calls_replay_declared_signature_rows() {
    let source = super::unit_scalar_calls::attached_unit_scalar_call_plan();
    let expected =
        crate::AbstractToTargetTranslationValidationError::StructuralCallArgumentMismatch {
            machine: MachineId::new(1).unwrap(),
            operation: OperationId::new(11).unwrap(),
        };
    let is_scalar_call = |operation: &TargetUnitOperation| {
        matches!(operation, TargetUnitOperation::ScalarCall { .. })
    };
    for native in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target =
            crate::lower_to_target_operations(&source, crate::TargetLoweringRequest::new(native))
                .unwrap();
        crate::validate_abstract_to_target_translation(&source, native, &target).unwrap();
        let calls = target
            .functions
            .iter()
            .flat_map(|function| &function.graph.blocks)
            .flat_map(|block| &block.operations)
            .filter(|operation| matches!(operation, TargetUnitOperation::ScalarCall { .. }))
            .count();
        assert_eq!(calls, 2);
        for mutation in [
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::ScalarCall { callee, .. } = operation else {
                    unreachable!()
                };
                *callee = MachineId::new(77).unwrap();
            }) as Box<dyn FnOnce(&mut TargetUnitOperation)>,
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::ScalarCall { call_plan, .. } = operation else {
                    unreachable!()
                };
                call_plan.stack_alignment = call_plan.stack_alignment.wrapping_add(8);
            }),
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::ScalarCall { arguments, .. } = operation else {
                    unreachable!()
                };
                arguments[0].parameter_index = 9;
            }),
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::ScalarCall { arguments, .. } = operation else {
                    unreachable!()
                };
                arguments[0].placement.locations.clear();
            }),
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::ScalarCall { arguments, .. } = operation else {
                    unreachable!()
                };
                let scalar_type = arguments[0].source.scalar_type();
                arguments[0].source =
                    target_operations::TargetUnitScalarArgumentSource::Parameter {
                        parameter_index: 0,
                        source_value: ValueId::new(99).unwrap(),
                        scalar_type,
                    };
            }),
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::ScalarCall { result_home, .. } = operation else {
                    unreachable!()
                };
                result_home.source_value = ValueId::new(99).unwrap();
            }),
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::ScalarCall { result_home, .. } = operation else {
                    unreachable!()
                };
                result_home.defining_operation = OperationId::new(10).unwrap();
            }),
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::ScalarCall { result_home, .. } = operation else {
                    unreachable!()
                };
                result_home.scalar_type = ScalarType::Boolean;
            }),
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::ScalarCall { result_home, .. } = operation else {
                    unreachable!()
                };
                result_home.shape = calling_conventions::ValueShape::integer(8, 8);
            }),
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::ScalarCall {
                    requirement_obligations,
                    ..
                } = operation
                else {
                    unreachable!()
                };
                requirement_obligations.push(semantic_vocabulary::ObligationId::new(7).unwrap());
            }),
            // A structural call row under a scalar call's key is the wrong role
            // even when its embedded plan is identical.
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::ScalarCall {
                    psi_operation,
                    callee,
                    call_plan,
                    arguments,
                    requirement_obligations,
                    crash_continuations,
                    ..
                } = operation.clone()
                else {
                    unreachable!()
                };
                *operation = TargetUnitOperation::Call {
                    origin: target_operations::NativeCallOrigin::Authored,
                    psi_operation,
                    callee,
                    call_plan,
                    scalar_arguments: arguments,
                    arguments: Vec::new(),
                    claim_transfers: Vec::new(),
                    requirement_obligations,
                    crash_continuations,
                };
            }),
        ] {
            let mutated = mutate_call_row(&target, is_scalar_call, mutation);
            assert_eq!(
                crate::validate_abstract_to_target_translation(&source, native, &mutated),
                Err(expected.clone()),
                "substituted scalar call row"
            );
        }
        // The embedded call replays against the callee's published entrance: a
        // callee that never published one cannot satisfy a retained call row.
        let mut unpublished = target.clone();
        unpublished
            .functions
            .iter_mut()
            .find(|function| function.machine == MachineId::new(2).unwrap())
            .unwrap()
            .scalar_abi = None;
        assert_eq!(
            crate::validate_abstract_to_target_translation(&source, native, &unpublished),
            Err(expected.clone())
        );
    }
}

#[test]
fn embedded_calls_reject_unbound_and_duplicate_forged_rows() {
    let source = super::unit_scalar_calls::attached_unit_scalar_call_plan();
    for native in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target =
            crate::lower_to_target_operations(&source, crate::TargetLoweringRequest::new(native))
                .unwrap();
        let retained = target
            .functions
            .iter()
            .flat_map(|function| &function.graph.blocks)
            .flat_map(|block| &block.operations)
            .find_map(|operation| match operation {
                TargetUnitOperation::ScalarCall { .. } => Some(operation.clone()),
                _ => None,
            })
            .unwrap();
        let TargetUnitOperation::ScalarCall {
            callee,
            call_plan,
            arguments,
            requirement_obligations,
            crash_continuations,
            ..
        } = retained
        else {
            unreachable!()
        };
        let forged = |psi_operation| TargetUnitOperation::Call {
            origin: target_operations::NativeCallOrigin::Authored,
            psi_operation,
            callee,
            call_plan: call_plan.clone(),
            scalar_arguments: arguments.clone(),
            arguments: Vec::new(),
            claim_transfers: Vec::new(),
            requirement_obligations: requirement_obligations.clone(),
            crash_continuations: crash_continuations.clone(),
        };
        // A call row keyed to the IntegerConstant operation binds to no source
        // call at all.
        let unbound = appended_call_row(
            &target,
            MachineId::new(1).unwrap(),
            forged(OperationId::new(10).unwrap()),
        );
        assert_eq!(
            crate::validate_abstract_to_target_translation(&source, native, &unbound),
            Err(
                crate::AbstractToTargetTranslationValidationError::StructuralCallArgumentMismatch {
                    machine: MachineId::new(1).unwrap(),
                    operation: OperationId::new(10).unwrap(),
                }
            )
        );
        // A second row under an existing call's key can only shadow the honest
        // replay.
        let duplicated = appended_call_row(
            &target,
            MachineId::new(1).unwrap(),
            forged(OperationId::new(11).unwrap()),
        );
        assert_eq!(
            crate::validate_abstract_to_target_translation(&source, native, &duplicated),
            Err(
                crate::AbstractToTargetTranslationValidationError::StructuralCallArgumentMismatch {
                    machine: MachineId::new(1).unwrap(),
                    operation: OperationId::new(11).unwrap(),
                }
            )
        );
    }
}

#[test]
fn stored_descriptor_calls_replay_unique_custody_plan_and_result() {
    let source = stored_descriptor_source();
    let entry = source
        .functions
        .iter()
        .find(|function| function.machine == source.entry)
        .unwrap();
    let store_operation = entry
        .operations
        .iter()
        .find_map(|operation| match operation {
            AbstractOperation::StoreDynamicDescriptor { psi_operation, .. } => Some(*psi_operation),
            _ => None,
        })
        .expect("stored descriptor operation");
    let call_operation = entry
        .operations
        .iter()
        .find_map(|operation| match operation {
            AbstractOperation::CallStoredDynamicScalar { psi_operation, .. } => {
                Some(*psi_operation)
            }
            _ => None,
        })
        .expect("stored dynamic call operation");
    for native in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target =
            crate::lower_to_target_operations(&source, crate::TargetLoweringRequest::new(native))
                .unwrap();
        crate::validate_abstract_to_target_translation(&source, native, &target).unwrap();
        let operations = target
            .functions
            .iter()
            .find(|function| function.machine == source.entry)
            .unwrap()
            .graph
            .blocks
            .iter()
            .flat_map(|block| &block.operations);
        let mut store = None;
        let mut call = None;
        for operation in operations {
            match operation {
                TargetUnitOperation::StoreDynamicDescriptor { .. } => store = Some(operation),
                TargetUnitOperation::StoredDynamicScalarCall { .. } => call = Some(operation),
                _ => {}
            }
        }
        let TargetUnitOperation::StoreDynamicDescriptor {
            source_argument, ..
        } = store.expect("stored descriptor row")
        else {
            unreachable!()
        };
        assert_eq!(source_argument.access, StructuralAccess::SharedBorrow);
        assert_eq!(source_argument.shape.class, ValueClass::BorrowedReference);
        let TargetUnitOperation::StoredDynamicScalarCall {
            call_plan,
            result_home,
            source_argument,
            ..
        } = call.expect("stored dynamic call row")
        else {
            unreachable!()
        };
        assert_eq!(source_argument.access, StructuralAccess::SharedBorrow);
        assert_eq!(source_argument.shape.class, ValueClass::BorrowedReference);
        assert_eq!(call_plan.parameters.len(), 1);
        assert_eq!(result_home.defining_operation, call_operation);
    }
    for native in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target =
            crate::lower_to_target_operations(&source, crate::TargetLoweringRequest::new(native))
                .unwrap();
        let call_expected =
            crate::AbstractToTargetTranslationValidationError::StructuralCallArgumentMismatch {
                machine: source.entry,
                operation: call_operation,
            };
        let is_stored_call = |operation: &TargetUnitOperation| {
            matches!(
                operation,
                TargetUnitOperation::StoredDynamicScalarCall { .. }
            )
        };
        for mutation in [
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::StoredDynamicScalarCall {
                    dynamic_dispatch, ..
                } = operation
                else {
                    unreachable!()
                };
                dynamic_dispatch.dispatch.realization = MachineId::new(77).unwrap();
            }) as Box<dyn FnOnce(&mut TargetUnitOperation)>,
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::StoredDynamicScalarCall { result, .. } = operation else {
                    unreachable!()
                };
                result.value = ValueId::new(99).unwrap();
            }),
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::StoredDynamicScalarCall { call_plan, .. } = operation
                else {
                    unreachable!()
                };
                call_plan.stack_alignment = call_plan.stack_alignment.wrapping_add(8);
            }),
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::StoredDynamicScalarCall {
                    source_argument, ..
                } = operation
                else {
                    unreachable!()
                };
                source_argument.access = if source_argument.access == StructuralAccess::Owned {
                    StructuralAccess::MutableBorrow
                } else {
                    StructuralAccess::Owned
                };
            }),
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::StoredDynamicScalarCall {
                    source_argument, ..
                } = operation
                else {
                    unreachable!()
                };
                source_argument.source_byte_offset =
                    source_argument.source_byte_offset.wrapping_add(8);
            }),
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::StoredDynamicScalarCall {
                    source_argument, ..
                } = operation
                else {
                    unreachable!()
                };
                source_argument.path.clear();
            }),
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::StoredDynamicScalarCall {
                    source_argument, ..
                } = operation
                else {
                    unreachable!()
                };
                source_argument.destination.locations.clear();
            }),
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::StoredDynamicScalarCall {
                    source_argument, ..
                } = operation
                else {
                    unreachable!()
                };
                source_argument.source =
                    target_operations::TargetStructuralArgumentSource::StructuralHome {
                        psi_operation: OperationId::new(1).unwrap(),
                    };
            }),
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::StoredDynamicScalarCall { result_home, .. } = operation
                else {
                    unreachable!()
                };
                result_home.source_value = ValueId::new(99).unwrap();
            }),
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::StoredDynamicScalarCall { result_home, .. } = operation
                else {
                    unreachable!()
                };
                result_home.shape = calling_conventions::ValueShape::integer(8, 8);
            }),
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::StoredDynamicScalarCall {
                    requirement_obligations,
                    ..
                } = operation
                else {
                    unreachable!()
                };
                requirement_obligations.push(semantic_vocabulary::ObligationId::new(7).unwrap());
            }),
        ] {
            let mutated = mutate_call_row(&target, is_stored_call, mutation);
            assert_eq!(
                crate::validate_abstract_to_target_translation(&source, native, &mutated),
                Err(call_expected.clone()),
                "substituted stored-call row"
            );
        }
        let store_expected =
            crate::AbstractToTargetTranslationValidationError::StructuralCallArgumentMismatch {
                machine: source.entry,
                operation: store_operation,
            };
        let is_store = |operation: &TargetUnitOperation| {
            matches!(
                operation,
                TargetUnitOperation::StoreDynamicDescriptor { .. }
            )
        };
        for mutation in [
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::StoreDynamicDescriptor { stored, .. } = operation else {
                    unreachable!()
                };
                stored.descriptor.field_identity = "forged".into();
            }) as Box<dyn FnOnce(&mut TargetUnitOperation)>,
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::StoreDynamicDescriptor {
                    source_argument, ..
                } = operation
                else {
                    unreachable!()
                };
                source_argument.source_byte_offset =
                    source_argument.source_byte_offset.wrapping_add(8);
            }),
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::StoreDynamicDescriptor {
                    source_argument, ..
                } = operation
                else {
                    unreachable!()
                };
                source_argument.destination.locations.clear();
            }),
        ] {
            let mutated = mutate_call_row(&target, is_store, mutation);
            assert_eq!(
                crate::validate_abstract_to_target_translation(&source, native, &mutated),
                Err(store_expected.clone()),
                "substituted descriptor-store row"
            );
        }
    }
}

#[test]
fn rebound_dynamic_calls_replay_both_instance_projections() {
    let source = rebound_descriptor_source();
    let entry = source
        .functions
        .iter()
        .find(|function| function.machine == source.entry)
        .unwrap();
    let call_operation = entry
        .operations
        .iter()
        .find_map(|operation| match operation {
            AbstractOperation::CallDynamicScalar { psi_operation, .. } => Some(*psi_operation),
            _ => None,
        })
        .expect("rebound dynamic call operation");
    for native in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target =
            crate::lower_to_target_operations(&source, crate::TargetLoweringRequest::new(native))
                .unwrap();
        crate::validate_abstract_to_target_translation(&source, native, &target).unwrap();
        let call = target
            .functions
            .iter()
            .find(|function| function.machine == source.entry)
            .unwrap()
            .graph
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .find(|operation| matches!(operation, TargetUnitOperation::DynamicScalarCall { .. }))
            .expect("retained rebound call");
        let TargetUnitOperation::DynamicScalarCall {
            call_plan,
            result_home,
            initial_argument,
            rebound_argument,
            ..
        } = call
        else {
            unreachable!()
        };
        for argument in [initial_argument, rebound_argument] {
            assert_eq!(argument.access, StructuralAccess::SharedBorrow);
            assert_eq!(argument.shape.class, ValueClass::BorrowedReference);
        }
        // Both projections land on the realization's single parameter slot.
        assert_eq!(call_plan.parameters.len(), 1);
        assert_eq!(initial_argument.destination, rebound_argument.destination);
        assert_eq!(result_home.defining_operation, call_operation);
    }
    let expected =
        crate::AbstractToTargetTranslationValidationError::StructuralCallArgumentMismatch {
            machine: source.entry,
            operation: call_operation,
        };
    let is_rebound_call = |operation: &TargetUnitOperation| {
        matches!(operation, TargetUnitOperation::DynamicScalarCall { .. })
    };
    for native in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target =
            crate::lower_to_target_operations(&source, crate::TargetLoweringRequest::new(native))
                .unwrap();
        for mutation in [
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::DynamicScalarCall {
                    dynamic_dispatch, ..
                } = operation
                else {
                    unreachable!()
                };
                dynamic_dispatch.dispatch.realization = MachineId::new(77).unwrap();
            }) as Box<dyn FnOnce(&mut TargetUnitOperation)>,
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::DynamicScalarCall { call_plan, .. } = operation else {
                    unreachable!()
                };
                call_plan.stack_alignment = call_plan.stack_alignment.wrapping_add(8);
            }),
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::DynamicScalarCall {
                    initial_argument, ..
                } = operation
                else {
                    unreachable!()
                };
                initial_argument.source_byte_offset =
                    initial_argument.source_byte_offset.wrapping_add(8);
            }),
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::DynamicScalarCall {
                    initial_argument, ..
                } = operation
                else {
                    unreachable!()
                };
                initial_argument.access = if initial_argument.access == StructuralAccess::Owned {
                    StructuralAccess::MutableBorrow
                } else {
                    StructuralAccess::Owned
                };
            }),
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::DynamicScalarCall {
                    rebound_argument, ..
                } = operation
                else {
                    unreachable!()
                };
                rebound_argument.path.clear();
            }),
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::DynamicScalarCall {
                    rebound_argument, ..
                } = operation
                else {
                    unreachable!()
                };
                rebound_argument.destination.locations.clear();
            }),
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::DynamicScalarCall { result_home, .. } = operation else {
                    unreachable!()
                };
                result_home.source_value = ValueId::new(99).unwrap();
            }),
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::DynamicScalarCall { result, .. } = operation else {
                    unreachable!()
                };
                result.scalar_type = ScalarType::Boolean;
            }),
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::DynamicScalarCall {
                    requirement_obligations,
                    ..
                } = operation
                else {
                    unreachable!()
                };
                requirement_obligations.push(semantic_vocabulary::ObligationId::new(7).unwrap());
            }),
            // Dropping the scalar result row is a different call role, not a
            // cheaper presentation of the same call.
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::DynamicScalarCall {
                    psi_operation,
                    dynamic_dispatch,
                    call_plan,
                    initial_argument,
                    rebound_argument,
                    requirement_obligations,
                    crash_continuations,
                    ..
                } = operation.clone()
                else {
                    unreachable!()
                };
                *operation = TargetUnitOperation::DynamicUnitCall {
                    psi_operation,
                    dynamic_dispatch,
                    call_plan,
                    initial_argument,
                    rebound_argument,
                    requirement_obligations,
                    crash_continuations,
                };
            }),
        ] {
            let mutated = mutate_call_row(&target, is_rebound_call, mutation);
            assert_eq!(
                crate::validate_abstract_to_target_translation(&source, native, &mutated),
                Err(expected.clone()),
                "substituted rebound-call row"
            );
        }
    }
}
