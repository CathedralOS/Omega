use super::*;
use language_core::sealed_intrinsics::ActivationOperation;
use terminal_psi::CheckedBoundaryExecution;

fn checked(program: &str) -> terminal_psi::TerminalModule {
    let file = language_core::bundled_files::BundledFile::CoreActivation;
    let source = std::str::from_utf8(bundled_declarations::bytes(file)).unwrap();
    let typed = super::build_purpose::frontier_with_core(
        program,
        "machine build_entry() {}",
        source_provenance::DependencyScope::Product,
        &[("activation.omg", "Activation", source)],
    );
    checked_trees(typed)
}

fn checked_trees(typed: crate::source_state::TypedTrees) -> terminal_psi::TerminalModule {
    let prepared = crate::prepare_fixture(typed);
    let entry = super::build_purpose::symbol(prepared.trees(), "main");
    let lowered = crate::lowering::prepare_compilation(
        &prepared,
        super::build_purpose::roster(
            &[entry],
            &[Some(super::build_purpose::package())],
            source_provenance::DependencyScope::Product,
        ),
        terminal_codec::TerminalPublication::Invocation,
    ).expect("source park must lower");
    let verdicts = crate::judge_compilation(lowered.program);
    let [Ok(RouteVerdict::Accepted(accepted))] = verdicts.as_slice() else {
        panic!("expected checked source park: {verdicts:?}");
    };
    accepted.elaboration.module.clone()
}

#[test]
fn sourced_reply_pair_keeps_the_two_borrows_of_its_publication_slot() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../../../source/library/core");
    let files = ["reply.omg", "access/access.omg", "layout.omg", "extent.omg", "reflect/reflect.omg",
        "foundation.omg", "content.omg", "device.omg", "run.omg", "nat.omg", "option.omg", "algebra.omg"];
    let sources: Vec<_> = files.iter().map(|file| std::fs::read_to_string(root.join(file)).unwrap()).collect();
    let core: Vec<_> = files.iter().zip(&sources).map(|(file,source)| (*file,"",source.as_str())).collect();
    let test = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/owned_activation/positive/issue_pair/main.omg")).unwrap();
    let typed = super::build_purpose::frontier_with_core(&test,
        "machine build_entry() {}", source_provenance::DependencyScope::Product, &core);
    let module = checked_trees(typed);
    let published = terminal_codec::ClosedTerminalModule::new(module).unwrap();
    terminal_codec::ClosedTerminalModule::from_semantic_bytes(published.semantic_bytes()).unwrap();
}

#[test]
fn owned_activation_park_publishes_the_reply_of_its_exact_request() {
    let module = checked(r"use {omega}/language/core/activation.omg;
machine main(request: |activation|::ParkRequest<u8>) -> u8
reaches none crashes none suspends {
    suspend |activation|::park::<u8>(request)
}");
    let park = module.interfaces.boundary_machines.iter().find(|row| row.declaration.as_ref()
        .is_some_and(|decl| decl.execution == CheckedBoundaryExecution::OwnedActivation(ActivationOperation::Park)))
        .expect("actual sealed park, not a host provider");
    assert!(park.declaration.as_ref().unwrap().source.specialization.is_some());
    assert!(module.operations().any(|op| matches!(op.kind, terminal_psi::OperationKind::BoundaryCall { boundary, .. } if boundary == park.id) && op.suspension_crossing.is_some()));
    let published = terminal_codec::ClosedTerminalModule::new(module.clone()).unwrap();
    terminal_codec::ClosedTerminalModule::from_semantic_bytes(published.semantic_bytes()).unwrap();
    let mut changed = park.clone();
    changed.result = terminal_psi::BoundaryMachineResult::Unit;
    assert_eq!(terminal_semantics::calls::owned_activation::validate_park(&module, &changed),
        Err("park result differs from its reply"));
}

#[test]
fn owned_activation_park_reply_pair_owns_both_exact_handles() {
    let module = checked(r"use {omega}/language/core/activation.omg;
machine main() -> |activation|::ReplyPair<u8>
reaches none crashes none {
    |activation|::new_reply_pair::<u8>()
}");
    let pair = module.interfaces.boundary_machines.iter().find(|row| row.declaration.as_ref().is_some_and(|decl|
        decl.execution == CheckedBoundaryExecution::OwnedActivation(ActivationOperation::NewReplyPair))).unwrap();
    let shape = terminal_semantics::calls::owned_activation::reply_pair_shape(&module, pair).unwrap();
    let pair_type = module.structural_type(shape.result).unwrap();
    let terminal_psi::StructuralTypeShape::Record { fields, .. } = &pair_type.shape else { unreachable!() };
    let field_name = |id| fields.iter().find(|field| field.id == id).unwrap().identity;
    let call = module.operations().find(|op| matches!(op.kind, terminal_psi::OperationKind::BoundaryCall { boundary, .. } if boundary == pair.id)).unwrap();
    let terminal_psi::OperationResult::Structural(result) = &call.result else { panic!("issuance must return the pair"); };
    assert_eq!(result.claims.len(), 2);
    assert_ne!(result.claims[0].claim, result.claims[1].claim);
    for field in [shape.request, shape.permit] {
        assert!(result.claims.iter().any(|claim| matches!(claim.path.as_slice(),
            [terminal_psi::StructuralPathSegment::Field(name)] if *name == field_name(field))));
    }
    let mut changed = module.clone();
    let record = changed.types.structural_types.iter_mut().find(|ty| ty.id == shape.result).unwrap();
    let terminal_psi::StructuralTypeShape::Record { fields, .. } = &mut record.shape else { unreachable!() };
    fields.iter_mut().find(|field| field.id == shape.permit).unwrap().field_type = terminal_psi::StructuralFieldType::Structural(shape.request_type);
    assert!(matches!(terminal_semantics::calls::owned_activation::reply_pair_shape(&changed, pair),
        Err("reply pair requires distinct linear opaque handles")));
    let published = terminal_codec::ClosedTerminalModule::new(module).unwrap();
    terminal_codec::ClosedTerminalModule::from_semantic_bytes(published.semantic_bytes()).unwrap();
}
