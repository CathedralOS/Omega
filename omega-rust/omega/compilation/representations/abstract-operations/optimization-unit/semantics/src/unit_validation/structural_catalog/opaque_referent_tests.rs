//! A pointer may name registered opaque data without declaring its storage.
//! The unit validator must still reject unknown targets and owned opaque data.

use crate::{
    OptimizationUnitValidationError,
    reconstruct_ownership_frontier_facts_with_admitted_cycle_machines,
};
use abstract_operations::{
    AbstractBlockEntry, AbstractFunction, AbstractFunctionResult, AbstractOperation,
    AbstractOperationPlan, StructuralTypeCatalog,
};
use optimization_unit::PsiOptimizationUnit;
use semantic_vocabulary::{BlockId, EdgeId, MachineId, StructuralFieldId, StructuralTypeId};
use terminal_psi::{
    ArrayExtent, BindingRelevance, SemanticFingerprint, StructuralAccess,
    StructuralFieldDeclaration, StructuralFieldType, StructuralRecordStorage,
    StructuralReferentIdentity, StructuralTypeDeclaration, StructuralTypeShape,
    TerminalDynamicDispatchCatalog, TerminalIdentityTableBuilder, TerminalPsiIdentity,
    VocabularyMarker,
};

fn ty(raw: u64) -> StructuralTypeId {
    StructuralTypeId::new(raw).unwrap()
}

fn no_op_unit(shapes: Vec<StructuralTypeShape>, registered: bool) -> PsiOptimizationUnit {
    let machine = MachineId::new(1).unwrap();
    let block = BlockId::new(1).unwrap();
    let mut names = TerminalIdentityTableBuilder::default();
    names.intern("body");
    let declarations = shapes
        .into_iter()
        .enumerate()
        .map(|(index, shape)| StructuralTypeDeclaration {
            id: ty(index as u64 + 1),
            identity: names.intern(&format!("test::Carrier{index}")),
            shape,
            quotient_carrier: None,
            owns_linear_claim: false,
            copyable: false,
            no_code_affine_cleanup: false,
            descriptor_cleanup: false,
            scalar_restriction: None,
        })
        .collect();
    let referents = if registered {
        vec![StructuralReferentIdentity {
            id: ty(99),
            identity: names.intern("test::Opaque"),
            foreign_descriptor: false,
            abstract_opaque: true,
        }]
    } else {
        Vec::new()
    };
    let plan = AbstractOperationPlan {
        // A raw unit label; this does not claim canonical Terminal admission.
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([8; 32]),
        },
        entry: machine,
        structural_types: StructuralTypeCatalog::new(declarations, referents),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        dynamic_dispatch: TerminalDynamicDispatchCatalog::default(),
        identities: names.build(),
        functions: vec![AbstractFunction {
            machine,
            attachment: None,
            nominal_cleanup_subject: None,
            entry: block,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            result: AbstractFunctionResult::Unit,
            entry_claims: Vec::new(),
            block_entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            block_entries: vec![AbstractBlockEntry {
                block,
                operation_offset: 0,
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
            }],
            operations: vec![AbstractOperation::ReturnUnit {
                psi_edge: EdgeId::new(1).unwrap(),
                cleanup_actions: Vec::new(),
            }],
        }],
    };
    optimization_unit::reconstruct_psi_optimization_unit_seed(
        &plan,
        terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
    )
    .expect("construct the real no-op unit and its structural catalog")
}

fn reference(access: StructuralAccess) -> StructuralTypeShape {
    StructuralTypeShape::Reference {
        referent: ty(99),
        access,
    }
}

#[test]
fn registered_opaque_reference_targets_survive_unit_ownership_validation() {
    let unit = no_op_unit(
        vec![
            reference(StructuralAccess::SharedBorrow),
            reference(StructuralAccess::MutableBorrow),
        ],
        true,
    );
    assert!(super::index_structural_catalogs(&unit).is_ok());
    reconstruct_ownership_frontier_facts_with_admitted_cycle_machines(&unit, &[])
        .expect("opaque pointer targets must not demand concrete storage declarations");
}

#[test]
fn unregistered_reference_targets_remain_invalid() {
    let unit = no_op_unit(vec![reference(StructuralAccess::SharedBorrow)], false);
    assert!(matches!(
        super::index_structural_catalogs(&unit),
        Err(OptimizationUnitValidationError::InvalidStructuralTypeIdentity(id)) if id == ty(1)
    ));
}

#[test]
fn registered_opaque_targets_cannot_supply_owned_storage() {
    let record = StructuralTypeShape::Record {
        storage: StructuralRecordStorage::Packed,
        fields: vec![StructuralFieldDeclaration {
            id: StructuralFieldId::new(1).unwrap(),
            identity: terminal_psi::TerminalIdentityIndex(1),
            relevance: BindingRelevance::Relevant,
            field_type: StructuralFieldType::Structural(ty(99)),
            storage: None,
            inline_carrier: None,
        }],
    };
    for owned in [
        record,
        StructuralTypeShape::FixedArray {
            element: ty(99),
            length: ArrayExtent::Literal(1),
        },
        StructuralTypeShape::ElementView {
            element: ty(99),
            access: None,
        },
    ] {
        let unit = no_op_unit(vec![reference(StructuralAccess::SharedBorrow), owned], true);
        assert!(matches!(
            super::index_structural_catalogs(&unit),
            Err(OptimizationUnitValidationError::UnknownStructuralType(id)) if id == ty(99)
        ));
    }
}

#[test]
fn registered_opaque_targets_do_not_enable_write_only_references() {
    let unit = no_op_unit(vec![reference(StructuralAccess::WriteOnlyBorrow)], true);
    assert!(matches!(
        super::index_structural_catalogs(&unit),
        Err(OptimizationUnitValidationError::InvalidStructuralTypeIdentity(id)) if id == ty(1)
    ));
}

#[test]
fn concrete_reference_cycles_remain_invalid() {
    let unit = no_op_unit(
        vec![
            StructuralTypeShape::FixedArray {
                element: ty(2),
                length: ArrayExtent::Literal(1),
            },
            StructuralTypeShape::Reference {
                referent: ty(1),
                access: StructuralAccess::SharedBorrow,
            },
        ],
        true,
    );
    assert!(matches!(
        super::index_structural_catalogs(&unit),
        Err(OptimizationUnitValidationError::RecursiveStructuralType(_))
    ));
}
