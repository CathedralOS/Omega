//! Raw Unit inputs exercise the real catalog, CFG, operation and ownership
//! validators. They supply declarations, not canonical or runtime admission.
use super::*;
use abstract_operations::{AbstractParameter, AbstractResult};
use semantic_vocabulary::{DomainSemanticId, Proposition, ScalarTerm, StructuralDomainId, ValueId};
use terminal_psi::{StructuralDomainDeclaration, StructuralDomainPredicate};

fn unit(
    writes: &[Option<bool>],
    entry_qualified: bool,
    result_qualified: bool,
) -> optimization_unit::PsiOptimizationUnit {
    let mut names = TerminalIdentityTableBuilder::default();
    let boolean = names.intern("formation-unit::Boolean");
    let reference = names.intern("formation-unit::MutableBooleanReference");
    let domain_name = names.intern("formation-unit::True");
    let domain = StructuralDomainId::new(1).unwrap();
    let types = vec![
        StructuralTypeDeclaration {
            id: ty(1),
            identity: boolean,
            shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
            quotient_carrier: None,
            owns_linear_claim: false,
            copyable: true,
            no_code_affine_cleanup: true,
            descriptor_cleanup: false,
            scalar_restriction: None,
        },
        StructuralTypeDeclaration {
            id: ty(2),
            identity: reference,
            shape: StructuralTypeShape::Reference {
                referent: ty(1),
                access: StructuralAccess::MutableBorrow,
            },
            quotient_carrier: None,
            owns_linear_claim: false,
            copyable: false,
            no_code_affine_cleanup: false,
            descriptor_cleanup: true,
            scalar_restriction: None,
        },
    ];
    let source = StructuralParameterDeclaration {
        place: place(1),
        position: 0,
        structural_type: if entry_qualified { ty(2) } else { ty(1) },
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::MutableBorrow,
        qualifications: Vec::new(),
        projected_qualifications: entry_qualified
            .then_some(terminal_psi::StructuralPathQualification {
                path: vec![StructuralPathSegment::Referent],
                domain,
            })
            .into_iter()
            .collect(),
    };
    let unknown = ValueId::new(1).unwrap();
    // An unrelated true value cannot establish a predicate on source storage.
    let mut operations = vec![O::BooleanConstant {
        psi_operation: op(1),
        result: ValueId::new(2).unwrap(),
        value: true,
    }];
    for (index, value) in writes.iter().enumerate() {
        let ordinal = u64::try_from(index).unwrap();
        let value = if let Some(value) = value {
            let result = ValueId::new(ordinal + 3).unwrap();
            operations.push(O::BooleanConstant {
                psi_operation: op(10 + ordinal * 3),
                result,
                value: *value,
            });
            result
        } else {
            unknown
        };
        operations.push(O::WriteOnlyPrimitiveStore {
            psi_operation: op(11 + ordinal * 3),
            destination: source.clone().into(),
            path: Vec::new(),
            value: AbstractResult {
                value,
                scalar_type: ScalarType::Boolean,
            },
            range_obligation: None,
        });
    }
    operations.extend([
        O::EstablishReference {
            psi_operation: op(500),
            source: StructuralArgument {
                place: source.place,
                path: if entry_qualified {
                    vec![StructuralPathSegment::Referent]
                } else {
                    Vec::new()
                },
                access: source.access,
            },
            storage: None,
            result: StructuralOperationResult {
                place: place(10),
                structural_type: ty(2),
                multiplicity: StructuralMultiplicity::Affine,
                qualifications: Vec::new(),
                projected_qualifications: result_qualified
                    .then_some(terminal_psi::StructuralPathQualification {
                        path: vec![StructuralPathSegment::Referent],
                        domain,
                    })
                    .into_iter()
                    .collect(),
                qualification_establishments: Vec::new(),
                claims: Vec::new(),
            },
        },
        O::ReleaseReference {
            psi_operation: op(501),
            source: place(10),
            path: Vec::new(),
        },
        O::ReturnUnit {
            psi_edge: EdgeId::new(1).unwrap(),
            cleanup_actions: Vec::new(),
        },
    ]);
    let function = AbstractFunction {
        machine: machine(1),
        attachment: None,
        nominal_cleanup_subject: None,
        entry: BlockId::new(1).unwrap(),
        parameters: vec![AbstractParameter {
            value: unknown,
            scalar_type: ScalarType::Boolean,
        }],
        structural_parameters: vec![source],
        result: AbstractFunctionResult::Unit,
        entry_claims: Vec::new(),
        block_entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        block_entries: vec![AbstractBlockEntry {
            block: BlockId::new(1).unwrap(),
            operation_offset: 0,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
        }],
        operations,
    };
    let plan = AbstractOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([43; 32]),
        },
        entry: machine(1),
        structural_types: types.into(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        dynamic_dispatch: TerminalDynamicDispatchCatalog::default(),
        identities: names.build(),
        functions: vec![function],
    };
    let mut unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
        &plan,
        terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
    )
    .expect("build actual scalar definitions, structural roles and CFG/fact indexes");
    unit.structural_domains = vec![StructuralDomainDeclaration {
        id: domain,
        semantic_domain: DomainSemanticId::new(1).unwrap(),
        identity: domain_name,
        carrier: ty(1),
        content_projection: None,
        byte_predicate: None,
        required_memberships: Some(Vec::new()),
        complete_predicate_theory: true,
        predicate: Some(StructuralDomainPredicate {
            subject: place(77),
            proposition: Proposition::Equal(
                ScalarTerm::Boolean(true),
                ScalarTerm::boolean_field_path(place(77), Vec::new()),
            ),
        }),
        establishment_routes: Vec::new(),
    }]
    .into();
    assert!(unit.runtime_evidence.is_none());
    unit.identity = optimization_unit::recompute_psi_optimization_unit_identity(&unit);
    unit
}

fn accepts(
    unit: &optimization_unit::PsiOptimizationUnit,
) -> Result<(), crate::OptimizationUnitValidationError> {
    crate::reconstruct_ownership_frontier_facts_with_admitted_cycle_machines(unit, &[]).map(|_| ())
}

fn rejects_formation(unit: &optimization_unit::PsiOptimizationUnit) {
    assert!(
        matches!(accepts(unit), Err(crate::OptimizationUnitValidationError::StructuralCatalogMismatch {
        machine: Some(actual),
    }) if actual == machine(1))
    );
}

#[test]
fn fresh_reference_uses_current_write_and_refuses_false_unknown_or_missing_write() {
    accepts(&unit(&[Some(true)], false, true))
        .expect("actual true write establishes fresh referent predicate");
    for writes in [vec![Some(false)], vec![None], Vec::new()] {
        accepts(&unit(&writes, false, false))
            .expect("the same writes and borrow are valid without the requested predicate");
        rejects_formation(&unit(&writes, false, true));
    }
}

#[test]
fn fresh_reference_cannot_reuse_a_previous_storage_version() {
    accepts(&unit(&[Some(true)], false, true)).expect("positive current write");
    accepts(&unit(&[Some(true), Some(false)], false, false))
        .expect("both writes are valid before an unqualified borrow");
    rejects_formation(&unit(&[Some(true), Some(false)], false, true));
}

#[test]
fn mutable_reference_retains_every_declared_source_duty() {
    accepts(&unit(&[], true, true))
        .expect("entry contract independently supplies its complete duty");
    rejects_formation(&unit(&[], true, false));
}
