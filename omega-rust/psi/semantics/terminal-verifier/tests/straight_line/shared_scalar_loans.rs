//! Primitive-scalar shared-borrow joins at block parameters. A `&T` selection
//! over a primitive leaf joins the canonical `PrimitiveScalar` type — the same
//! referent shape a shared primitive signature parameter already carries — so
//! the block parameter admits it under `SharedBorrow` while every other access
//! or custody class stays closed. The successor lane still resolves each
//! edge's authored projection to the exact leaf type, the pinned referent root
//! cannot be disturbed while the view is bound, and the joined view forwards
//! whole to a shared formal. An in-block scalar read of the joined place has
//! no admitted operand spelling yet and stays rejected.

use super::{
    AdmissionProfile, Block, IntegerSign, IntegerType, MachineContract, ModuleError, Operation,
    OperationKind, OperationResult, ProofBundle, ScalarType, StructuralAccess, StructuralArgument,
    StructuralFieldDeclaration, StructuralFieldType, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralPlaceDeclaration, StructuralPlaceKind,
    StructuralTypeDeclaration, StructuralTypeShape, TerminalMachine, TerminalMachineResult,
    TerminalModule, Terminator, ValueDeclaration, unit_module, validate_module, verify_module,
};
use semantic_vocabulary::PsiSemanticId;
use terminal_psi::{BindingRelevance, StructuralPathSegment};

const PAYLOAD_TYPE: u64 = 1;
const SCALAR_TYPE: u64 = 2;
const ROOT: u64 = 1;
const VIEW: u64 = 2;
const CALL_VALUE: u64 = 3;
const READ_VALUE: u64 = 4;
const RESULT_DECL: u64 = 5;
const CALLEE_RESULT_DECL: u64 = 6;
const CALLEE_ROOT: u64 = 10;
const INTEGER_VALUE: u64 = 50;
const JOIN_BLOCK: u64 = 901;
const CALLEE: u64 = 901;
const CALLEE_BLOCK: u64 = 910;

fn id<T: PsiSemanticId>(raw: u64) -> T {
    T::new(raw).unwrap()
}

fn i32_type() -> IntegerType {
    IntegerType::new(IntegerSign::Signed, 32).unwrap()
}

fn i32_scalar() -> ScalarType {
    ScalarType::Integer(i32_type())
}

fn field(raw: u64, identity: &str, field_type: StructuralFieldType) -> StructuralFieldDeclaration {
    StructuralFieldDeclaration {
        id: id(raw),
        identity: identity.to_owned(),
        relevance: BindingRelevance::Relevant,
        field_type,
    }
}

fn payload_type() -> StructuralTypeDeclaration {
    StructuralTypeDeclaration {
        id: id(PAYLOAD_TYPE),
        identity: "test::Payload".to_owned(),
        shape: StructuralTypeShape::Record {
            fields: vec![
                field(1, "value", StructuralFieldType::Scalar(i32_scalar())),
                field(2, "flag", StructuralFieldType::Scalar(ScalarType::Boolean)),
            ],
        },
    }
}

/// The canonical leaf declaration a `&i32` referent resolves to. Projection
/// resolution finds it by shape, so exactly one declaration may carry it.
fn scalar_type() -> StructuralTypeDeclaration {
    StructuralTypeDeclaration {
        id: id(SCALAR_TYPE),
        identity: "test::Scalar".to_owned(),
        shape: StructuralTypeShape::PrimitiveScalar(i32_scalar()),
    }
}

fn loan_argument(root: u64, path: &[&str]) -> StructuralArgument {
    StructuralArgument {
        place: id(root),
        path: path
            .iter()
            .map(|identity| StructuralPathSegment::Field((*identity).to_owned()))
            .collect(),
        access: StructuralAccess::SharedBorrow,
    }
}

fn shared_parameter(
    raw: u64,
    position: u32,
    structural_type: u64,
) -> StructuralParameterDeclaration {
    StructuralParameterDeclaration {
        place: id(raw),
        position,
        is_self: false,
        structural_type: id(structural_type),
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::SharedBorrow,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    }
}

fn return_unit(edge: u64, discards: &[u64]) -> Terminator {
    Terminator::ReturnUnit {
        edge: id(edge),
        trivial_affine_discards: discards.iter().map(|raw| id(*raw)).collect(),
    }
}

/// One machine whose owned `Payload` parameter loans the scalar field `value`
/// into a second block that binds it as a shared `PrimitiveScalar` view. The
/// joined view is left unread: the only admitted consumers are onward shared
/// loans, and the declaration alone is what the join needs.
fn scalar_loan_module(multiplicity: StructuralMultiplicity) -> TerminalModule {
    let mut module = unit_module();
    module.structural_types = vec![payload_type(), scalar_type()];
    let machine = &mut module.machines[0];
    machine
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place: id(ROOT),
            position: 0,
            is_self: false,
            structural_type: id(PAYLOAD_TYPE),
            multiplicity,
            access: StructuralAccess::Owned,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
    machine.structural_places = vec![
        StructuralPlaceDeclaration {
            id: id(ROOT),
            kind: StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        },
        StructuralPlaceDeclaration {
            id: id(VIEW),
            kind: StructuralPlaceKind::BlockParameter {
                block: id(JOIN_BLOCK),
                position: 0,
            },
        },
    ];
    machine.blocks[0].terminator = Terminator::Jump {
        structural_arguments: vec![loan_argument(ROOT, &["value"])],
        edge: id(901),
        target: id(JOIN_BLOCK),
        arguments: Vec::new(),
        erased_arguments: Vec::new(),
        residual_affine_discards: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    machine.blocks.push(Block {
        erased_scalar_formals: Vec::new(),
        structural_parameters: vec![shared_parameter(VIEW, 0, SCALAR_TYPE)],
        id: id(JOIN_BLOCK),
        parameters: Vec::new(),
        operations: Vec::new(),
        terminator: return_unit(
            902,
            match multiplicity {
                StructuralMultiplicity::Affine => &[ROOT],
                _ => &[],
            },
        ),
    });
    module
}

/// A `&i32` signature parameter is itself the borrowed carrier: the entry
/// edge re-loans it whole into the block parameter with no projection.
fn scalar_reborrow_module() -> TerminalModule {
    let mut module = scalar_loan_module(StructuralMultiplicity::Unrestricted);
    let machine = &mut module.machines[0];
    machine.structural_parameters[0].structural_type = id(SCALAR_TYPE);
    machine.structural_parameters[0].access = StructuralAccess::SharedBorrow;
    let Terminator::Jump {
        structural_arguments,
        ..
    } = &mut machine.blocks[0].terminator
    else {
        unreachable!()
    };
    structural_arguments[0].path = Vec::new();
    module
}

/// The joined scalar view forwards whole to `read`'s shared formal — the
/// consumer the primitive carrier exists for. `read` observes its own
/// signature parameter through a `PrimitiveScalarRead`.
fn scalar_call_module() -> TerminalModule {
    let mut module = scalar_loan_module(StructuralMultiplicity::Unrestricted);
    module.machines[0].result = TerminalMachineResult::Scalar(ValueDeclaration {
        qualifications: Default::default(),
        id: id(RESULT_DECL),
        scalar_type: i32_scalar(),
    });
    let join = &mut module.machines[0].blocks[1];
    join.operations.push(Operation {
        static_reach_binding: None,
        id: id(902),
        result: OperationResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: id(CALL_VALUE),
            scalar_type: i32_scalar(),
        }),
        kind: OperationKind::CallStructuralScalar {
            callee: id(CALLEE),
            arguments: Vec::new(),
            erased_arguments: Vec::new(),
            structural_arguments: vec![StructuralArgument {
                place: id(VIEW),
                path: Vec::new(),
                access: StructuralAccess::SharedBorrow,
            }],
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    });
    join.terminator = Terminator::Return {
        edge: id(903),
        value: id(CALL_VALUE),
        cleanup_actions: Vec::new(),
    };
    module.machines.push(TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: id(CALLEE),
        attachment: None,
        structural_parameters: vec![shared_parameter(CALLEE_ROOT, 0, SCALAR_TYPE)],
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        parameters: Vec::new(),
        ranked_scc: None,
        result: TerminalMachineResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: id(CALLEE_RESULT_DECL),
            scalar_type: i32_scalar(),
        }),
        structural_places: vec![StructuralPlaceDeclaration {
            id: id(CALLEE_ROOT),
            kind: StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        }],
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: id(CALLEE_BLOCK),
        blocks: vec![Block {
            erased_scalar_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: id(CALLEE_BLOCK),
            parameters: Vec::new(),
            operations: vec![Operation {
                static_reach_binding: None,
                id: id(910),
                result: OperationResult::Scalar(ValueDeclaration {
                    qualifications: Default::default(),
                    id: id(READ_VALUE),
                    scalar_type: i32_scalar(),
                }),
                kind: OperationKind::PrimitiveScalarRead {
                    source: id(CALLEE_ROOT),
                    path: Vec::new(),
                },
            }],
            terminator: Terminator::Return {
                edge: id(911),
                value: id(READ_VALUE),
                cleanup_actions: Vec::new(),
            },
        }],
        contract: MachineContract {
            erased_scalar_formals: Vec::new(),
            id: id(911),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    });
    module
}

fn argument(module: &mut TerminalModule) -> &mut StructuralArgument {
    let Terminator::Jump {
        structural_arguments,
        ..
    } = &mut module.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    &mut structural_arguments[0]
}

fn view_parameter(module: &mut TerminalModule) -> &mut StructuralParameterDeclaration {
    &mut module.machines[0].blocks[1].structural_parameters[0]
}

#[test]
fn primitive_shared_loan_joins_the_scalar_leaf() {
    for multiplicity in [
        StructuralMultiplicity::Unrestricted,
        StructuralMultiplicity::Affine,
    ] {
        let module = scalar_loan_module(multiplicity);
        validate_module(&module).unwrap_or_else(|error| panic!("{multiplicity:?}: {error}"));
        verify_module(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default(),
        )
        .unwrap_or_else(|error| panic!("{multiplicity:?}: {error}"));
    }
}

#[test]
fn primitive_shared_loan_reborrows_a_signature_parameter_whole() {
    let module = scalar_reborrow_module();
    validate_module(&module).unwrap_or_else(|error| panic!("{error}"));
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("a shared scalar signature parameter re-loans whole into a block");
}

#[test]
fn primitive_shared_loan_forwards_the_view_to_a_shared_formal() {
    let module = scalar_call_module();
    validate_module(&module).unwrap_or_else(|error| panic!("{error}"));
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("the joined scalar view reaches the callee's shared formal");
}

#[test]
fn primitive_shared_loan_rejects_substituted_custody_path_or_leaf() {
    for mutation in 0..17 {
        let mut module = scalar_loan_module(StructuralMultiplicity::Unrestricted);
        match mutation {
            // An owned or exclusive presentation cannot satisfy the shared
            // loan even when root and path are exact.
            0 => argument(&mut module).access = StructuralAccess::Owned,
            1 => argument(&mut module).access = StructuralAccess::MutableBorrow,
            2 => argument(&mut module).access = StructuralAccess::WriteOnlyBorrow,
            // The whole root is the record, not the scalar leaf; a missing
            // field or a leaf of another scalar type fails the path's exact
            // resolution.
            3 => argument(&mut module).path = Vec::new(),
            4 => {
                argument(&mut module).path =
                    vec![StructuralPathSegment::Field("missing".to_owned())]
            }
            5 => argument(&mut module).path = vec![StructuralPathSegment::Field("flag".to_owned())],
            // An unknown place or the not-yet-bound target parameter cannot
            // supply the view.
            6 => argument(&mut module).place = id(99),
            7 => argument(&mut module).place = id(VIEW),
            // The joined parameter carries no affine or linear custody, and
            // no other access mode reads as shared.
            8 => view_parameter(&mut module).multiplicity = StructuralMultiplicity::Affine,
            9 => view_parameter(&mut module).multiplicity = StructuralMultiplicity::Linear,
            10 => view_parameter(&mut module).access = StructuralAccess::Owned,
            11 => view_parameter(&mut module).access = StructuralAccess::MutableBorrow,
            12 => view_parameter(&mut module).access = StructuralAccess::WriteOnlyBorrow,
            // The parameter's declared type is the exact leaf: the record
            // root itself is a different referent type.
            13 => view_parameter(&mut module).structural_type = id(PAYLOAD_TYPE),
            // A self-rooted or signature-shaped place breaks the parameter's
            // placement evidence.
            14 => view_parameter(&mut module).is_self = true,
            15 => {
                module.machines[0].structural_places[1].kind = StructuralPlaceKind::Parameter {
                    position: 1,
                    is_self: false,
                }
            }
            // The leaf declaration is canonical: without it the parameter's
            // type is unknown.
            16 => module
                .structural_types
                .retain(|declaration| declaration.id != id(SCALAR_TYPE)),
            _ => unreachable!(),
        }
        assert!(validate_module(&module).is_err(), "mutation {mutation}");
    }
}

#[test]
fn primitive_shared_loan_keeps_the_root_pinned_and_the_leaf_unread_in_block() {
    // Writing through the loaned root while the scalar view observes it
    // disturbs the pin, even though the store's own access authority is
    // otherwise legal.
    let mut module = scalar_loan_module(StructuralMultiplicity::Affine);
    module.machines[0].parameters.push(ValueDeclaration {
        qualifications: Default::default(),
        id: id(INTEGER_VALUE),
        scalar_type: i32_scalar(),
    });
    module.machines[0].blocks[1].operations.push(Operation {
        static_reach_binding: None,
        id: id(903),
        result: OperationResult::Unit,
        kind: OperationKind::StructuralScalarFieldStore {
            destination: id(ROOT),
            path: Vec::new(),
            field: id(1),
            value: id(INTEGER_VALUE),
            range_obligation: None,
        },
    });
    assert!(
        matches!(
            validate_module(&module),
            Err(ModuleError::SharedStructuralLoanDisturbed { place, .. })
                if place == id::<semantic_vocabulary::PlaceId>(ROOT)
        ),
        "a store through the pinned root must fail"
    );

    // The operand lane that could read a joined primitive in place has no
    // admitted spelling yet: an empty-path `PrimitiveScalarRead` reaches only
    // signature parameters and established primitive locals, never a block
    // parameter.
    let mut module = scalar_loan_module(StructuralMultiplicity::Unrestricted);
    module.machines[0].blocks[1].operations.push(Operation {
        static_reach_binding: None,
        id: id(903),
        result: OperationResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: id(READ_VALUE),
            scalar_type: i32_scalar(),
        }),
        kind: OperationKind::PrimitiveScalarRead {
            source: id(VIEW),
            path: Vec::new(),
        },
    });
    assert!(
        matches!(
            validate_module(&module),
            Err(ModuleError::InvalidPrimitiveScalarRead { place, .. })
                if place == id::<semantic_vocabulary::PlaceId>(VIEW)
        ),
        "an in-block read of the joined scalar stays closed"
    );
}
