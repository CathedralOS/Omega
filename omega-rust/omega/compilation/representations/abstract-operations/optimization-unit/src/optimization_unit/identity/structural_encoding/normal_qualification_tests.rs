//! Whole-unit identity must retain normal preservation as well as entry demands.
//! This catalog-only seed exercises identity, not optimizer or execution admission.

use crate::{PsiOptimizationUnit, recompute_psi_optimization_unit_identity};
use optimization_core::OptimizationUnitIdentity;
use semantic_vocabulary::{
    BoundaryMachineId, FuelScheduleIdentity, MachineId, PlaceId, StructuralDomainId,
    StructuralTypeId,
};
use std::sync::Arc;
use terminal_psi::{
    BoundaryCompletion, BoundaryMachineDeclaration, BoundaryMachineResult, BoundaryParameterKind,
    BoundaryRequirementKind, NominalDeclarationIdentity, SemanticFingerprint, StructuralAccess,
    StructuralDomainRequirement, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralPathSegment, TerminalIdentityIndex, TerminalIdentityTableBuilder,
    TerminalPsiIdentity, VocabularyMarker,
};

fn unit() -> PsiOptimizationUnit {
    let mut identities = TerminalIdentityTableBuilder::default();
    for position in 1..=4 {
        identities.intern(&format!("test::normal-preservation::{position}"));
    }
    let boundary = BoundaryMachineDeclaration {
        id: BoundaryMachineId::new(1).unwrap(),
        declaration: None,
        role: terminal_psi::BoundaryDeclarationRole::Callable,
        kind: BoundaryRequirementKind::Machine,
        completion: BoundaryCompletion::NormalReturn,
        unconditional_termination: false,
        identity: TerminalIdentityIndex(1),
        owner: NominalDeclarationIdentity::new(TerminalIdentityIndex(2)),
        attachment: None,
        parameter_order: vec![BoundaryParameterKind::Structural],
        scalar_parameters: Vec::new(),
        crash_routes: Vec::new(),
        structural_parameters: vec![StructuralParameterDeclaration {
            place: PlaceId::new(1).unwrap(),
            position: 0,
            structural_type: StructuralTypeId::new(1).unwrap(),
            multiplicity: StructuralMultiplicity::Affine,
            access: StructuralAccess::MutableBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        }],
        result: BoundaryMachineResult::Unit,
        requires: vec![requirement(Vec::new())],
        normal_qualifications: Vec::new(),
        scalar_domain_requirements: Vec::new(),
        scalar_requires: Vec::new(),
        scalar_ensures: Vec::new(),
        reference_outputs: Vec::new(),
        program_local_root_introductions: Vec::new(),
        content_guarantees: Vec::new(),
        fixed_service_reach: Vec::new(),
        owning_service: None,
        published_service_ceiling: Vec::new(),
    };
    PsiOptimizationUnit {
        identity: OptimizationUnitIdentity::from_canonical_bytes(b"uncomputed test seed"),
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([7; 32]),
        },
        fuel_schedule: FuelScheduleIdentity::new(1).unwrap(),
        entry: MachineId::new(1).unwrap(),
        structural_types: Vec::new().into(),
        structural_domains: Arc::new([]),
        services: Arc::new([]),
        root_service_reach: Default::default(),
        boundary_machines: vec![boundary],
        provider_candidates: Vec::new(),
        dynamic_dispatch: Default::default(),
        accepted_obligation_facts: Vec::new(),
        proof_questions: Vec::new(),
        ownership_frontier_facts: Vec::new(),
        pruned_machines: Vec::new(),
        identities: identities.build(),
        runtime_evidence: None,
        functions: Vec::new(),
    }
}

fn requirement(path: Vec<StructuralPathSegment>) -> StructuralDomainRequirement {
    StructuralDomainRequirement {
        argument_index: 0,
        path,
        domain: StructuralDomainId::new(1).unwrap(),
    }
}

#[test]
fn optimization_unit_identity_commits_normal_boundary_rows_and_exact_scope() {
    let mut unit = unit();
    let absent = recompute_psi_optimization_unit_identity(&unit);
    unit.boundary_machines[0]
        .normal_qualifications
        .push(requirement(Vec::new()));
    let root = recompute_psi_optimization_unit_identity(&unit);
    assert_ne!(
        root, absent,
        "normal-only guarantees must enter the unit identity"
    );
    unit.boundary_machines[0].normal_qualifications[0].path =
        vec![StructuralPathSegment::Field(TerminalIdentityIndex(3))];
    let field = recompute_psi_optimization_unit_identity(&unit);
    unit.boundary_machines[0].normal_qualifications[0].path = vec![
        StructuralPathSegment::Referent,
        StructuralPathSegment::AllElements,
    ];
    let elements = recompute_psi_optimization_unit_identity(&unit);
    assert_ne!(field, root);
    assert_ne!(elements, field);
    assert_ne!(elements, root);
}

#[test]
fn optimization_unit_identity_commits_required_boundary_scope() {
    let mut unit = unit();
    let root = recompute_psi_optimization_unit_identity(&unit);
    unit.boundary_machines[0].requires[0].path =
        vec![StructuralPathSegment::Field(TerminalIdentityIndex(3))];
    let field = recompute_psi_optimization_unit_identity(&unit);
    assert_ne!(field, root);
    unit.boundary_machines[0].requires[0].path[0] =
        StructuralPathSegment::Field(TerminalIdentityIndex(4));
    assert_ne!(recompute_psi_optimization_unit_identity(&unit), field);
}
