#[path = "support/source.rs"]
mod source;

use omega_lowering::{BlockTerminator, CleanupAction, LoweredMachine, LoweredPackage, Parameter, PlaceId, PlaceProjection, ReferenceAccess, TypeShape};

const HANDLE: &str = "data Handle dropped by release{mark:u8;} machine release(h:Handle){let Handle{mark as _}=h;}";

fn machine<'a>(
    package: &'a LoweredPackage,
    name: &str,
) -> &'a LoweredMachine {
    package
        .machines
        .iter()
        .find(|machine| machine.declaration.path == name)
        .unwrap()
}

/// The owned formal and the cleanup scheduled on the single normal return.
fn finish(package: &LoweredPackage) -> (PlaceId, &[CleanupAction]) {
    let body = &machine(package, "finish").body;
    let [Parameter::Structural(root)] = body.parameters[..] else {
        panic!("one owned holder expected")
    };
    let mut returns = body.blocks.iter().filter_map(|block| match &block.terminator {
        BlockTerminator::Return { cleanup, .. } => Some(cleanup.as_slice()),
        _ => None,
    });
    let cleanup = returns.next().expect("normal return expected");
    assert!(returns.next().is_none());
    (root, cleanup)
}

fn invokes_anything(package: &LoweredPackage) -> bool {
    let body = &machine(package, "finish").body;
    body.blocks.iter().any(|block| match &block.terminator {
        BlockTerminator::Return { cleanup, .. } | BlockTerminator::TerminalCall { cleanup, .. } => cleanup
            .iter()
            .any(|action| matches!(action, CleanupAction::Invoke { .. })),
        BlockTerminator::Jump(edge) => edge
            .cleanup
            .iter()
            .any(|action| matches!(action, CleanupAction::Invoke { .. })),
        BlockTerminator::Conditional { when_true, when_false, .. } => [when_true, when_false].iter().any(|edge| {
            edge.cleanup
                .iter()
                .any(|action| matches!(action, CleanupAction::Invoke { .. }))
        }),
        BlockTerminator::ExternalExit(_) => false,
    })
}

#[test]
fn a_plain_record_with_a_shared_reference_is_discarded_whole() {
    for (prefix, field) in [
        ("", "@'r u8"),
        ("data Token{v:u8;}", "@'r Token"),
        (HANDLE, "@'r Handle"),
    ] {
        let text = format!("{prefix} data Holder<'r>{{r:{field};}} machine finish<'r>(h:Holder<'r>){{}}");
        let package = source::lower(&[("embedded.omg", &text)]).unwrap();
        let (root, cleanup) = finish(&package);
        assert_eq!(cleanup, [CleanupAction::DiscardRoot(root)], "{field}");
        // Ending the loan never disposes its referent, even a resource with a cleanup machine.
        assert!(!invokes_anything(&package), "{field}");
    }
}

#[test]
fn a_shared_reference_beside_a_resource_is_a_separate_residual() {
    let text = format!("{HANDLE} data Holder<'r>{{t:Handle;r:@'r u8;}} machine finish<'r>(h:Holder<'r>){{}}");
    let package = source::lower(&[("embedded.omg", &text)]).unwrap();
    let (root, cleanup) = finish(&package);
    let release = machine(&package, "release").id;
    let [
        CleanupAction::DiscardResidual { place: loan, ty: loan_ty },
        CleanupAction::Invoke { place: owned, target, .. },
    ] = cleanup
    else {
        panic!("reverse declaration order: discard the loan, then release the handle: {cleanup:?}")
    };
    assert_eq!(loan.root, root);
    assert!(matches!(loan.path.as_slice(), [PlaceProjection::Field(f)] if f.path == "Holder::r"));
    assert!(matches!(
        package.types.get(*loan_ty),
        Some(TypeShape::ScalarReference {
            access: ReferenceAccess::Shared,
            ..
        })
    ));
    assert_eq!(owned.root, root);
    assert!(matches!(owned.path.as_slice(), [PlaceProjection::Field(f)] if f.path == "Holder::t"));
    assert_eq!(*target, release);
}

/// An exclusive loan stored in a record ends when the record is discarded, as an
/// overwritten field's carried loans end (wiki/spec/language/lifetimes.md#carried-loan-transport).
/// Lowering publishes the ordinary whole discard; the lowered-form checker judges
/// when the lender regains access. The referent is never disposed, even a resource.
#[test]
fn an_embedded_exclusive_loan_ends_when_its_record_is_discarded() {
    use omega_lowering::ReferenceAccess::{Mutable, Shared};
    let cases = [
        ("data Token{v:u8;}", "r:@'r mutable Token;", vec![("Holder::r", Mutable)]),
        ("data Token{v:u8;}", "n:u8;r:@'r mutable Token;", vec![("Holder::r", Mutable)]),
        ("", "r:@'r mutable u8;", vec![("Holder::r", Mutable)]),
        ("", "r:@'r mutable [u32];", vec![("Holder::r", Mutable)]),
        (HANDLE, "r:@'r mutable Handle;", vec![("Holder::r", Mutable)]),
        ("data Token{v:u8;}", "r:@'r mutable [Token];", vec![("Holder::r", Mutable)]),
        ("", "r:@'r [u8];s:@'r mutable u8;", vec![("Holder::r", Shared), ("Holder::s", Mutable)]),
    ];
    let observed: Vec<_> = cases
        .iter()
        .map(|(prefix, fields, _)| {
            let text = format!("{prefix} data Holder<'r>{{{fields}}} machine finish<'r>(h:Holder<'r>){{}}");
            let package = source::lower(&[("embedded.omg", &text)]).unwrap_or_else(|error| panic!("{fields}: {error:?}"));
            psi_handoff_consumer::discarded_loan_fields(&package, "finish").map(|loans| {
                loans
                    .into_iter()
                    .map(|(field, access)| (field.to_string(), access))
                    .collect::<Vec<_>>()
            })
        })
        .collect();
    let expected: Vec<Result<Vec<(String, omega_lowering::ReferenceAccess)>, &str>> = cases
        .iter()
        .map(|(_, _, loans)| {
            Ok(loans
                .iter()
                .map(|(field, access)| (field.to_string(), *access))
                .collect())
        })
        .collect();
    spec_corpus::observe_handoff!(psi "psi-output/embedded-exclusive-loan-discard", assert_eq!(observed, expected));
}

/// Beside an owned resource, the exclusive loan is its own residual and ends
/// before the resource's cleanup machine runs, in reverse declaration order.
#[test]
fn an_exclusive_reference_beside_a_resource_is_a_separate_residual() {
    let text = format!("{HANDLE} data Holder<'r>{{t:Handle;r:@'r mutable u8;}} machine finish<'r>(h:Holder<'r>){{}}");
    let package = source::lower(&[("embedded.omg", &text)]).unwrap();
    let (root, cleanup) = finish(&package);
    let release = machine(&package, "release").id;
    let [
        CleanupAction::DiscardResidual { place: loan, ty: loan_ty },
        CleanupAction::Invoke { place: owned, target, .. },
    ] = cleanup
    else {
        panic!("discard the loan, then release the handle: {cleanup:?}")
    };
    assert_eq!(loan.root, root);
    assert!(matches!(loan.path.as_slice(), [PlaceProjection::Field(f)] if f.path == "Holder::r"));
    assert!(matches!(
        package.types.get(*loan_ty),
        Some(TypeShape::ScalarReference {
            access: ReferenceAccess::Mutable,
            ..
        })
    ));
    assert!(matches!(owned.path.as_slice(), [PlaceProjection::Field(f)] if f.path == "Holder::t"));
    assert_eq!(*target, release);
}

#[test]
fn imported_drop_specializes_the_normal_return_edge_for_the_callers_resource() {
    let text = format!(r"use omega\core\drop; {HANDLE} machine finish(h:Handle){{drop(h);}}");
    let package = source::lower(&[("client.omg", &text)]).unwrap();
    let drop = machine(&package, r"core\drop");
    assert_eq!(drop.declaration.package, omega_toolchain::TOOLCHAIN_PACKAGE);
    let [Parameter::Structural(formal)] = drop.body.parameters[..] else { panic!("owned handle") };
    let BlockTerminator::Return { cleanup, .. } = &drop.body.blocks[0].terminator else { panic!("normal return") };
    assert!(matches!(&cleanup[..], [CleanupAction::Invoke { place, target, .. }]
        if place.root == formal && place.path.is_empty() && *target == machine(&package, "release").id));
    assert!(!invokes_anything(&package), "the caller has transferred its handle");
}

#[test]
fn an_exported_empty_generic_body_keeps_its_formals_on_the_open_return_edge() {
    use omega_lowering::{TemplateStep, TypeExpressionStep};
    let text = "public machine finish<A,B>(first:A,second:B) {} machine use_it(x:u8,y:u16){finish(x,y);}";
    let package = source::lower(&[("client.omg", text)]).unwrap();
    let template = package.machine_templates.iter().find(|t| t.declaration.path == "finish").unwrap();
    assert!(package.machines.iter().any(|m| m.declaration == template.declaration));
    assert_eq!(template.body.len(), 2);
    for (step, index) in template.body.iter().zip([1, 0]) {
        let TemplateStep::DisposeFormal { formal, ty, site } = step else { panic!("formal disposition") };
        assert_eq!(*formal, index);
        assert_eq!(site.file, "client.omg");
        assert_eq!(ty, &template.signature.parameters[index]);
        assert!(matches!(&ty.0[..], [TypeExpressionStep::GenericParameter { owner, index: binder }]
            if owner == &template.declaration && *binder == index));
    }
}

#[test]
fn an_imported_drop_leaves_an_unsettled_linear_discard_visible() {
    let text = r"use omega\core\drop; data Claim [linear] { serial:u8; } machine finish(c:Claim){drop(c);}";
    let package = source::lower(&[("client.omg", text)]).unwrap();
    let claim = package.data.iter().find(|d| d.declaration.path == "Claim").unwrap();
    assert_eq!(claim.multiplicity, omega_lowering::Multiplicity::Linear);
    assert!(claim.cleanup_target.is_none());
    let drop = machine(&package, r"core\drop");
    let [Parameter::Structural(formal)] = drop.body.parameters[..] else { panic!("owned linear claim") };
    let BlockTerminator::Return { cleanup, .. } = &drop.body.blocks[0].terminator else { panic!("normal return") };
    assert_eq!(cleanup, &[CleanupAction::DiscardRoot(formal)]);
}
