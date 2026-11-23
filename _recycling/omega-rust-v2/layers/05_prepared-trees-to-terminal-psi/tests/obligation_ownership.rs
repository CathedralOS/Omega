//! Region slots express the promise; the returned descriptor supplies independent backing evidence.
#[allow(dead_code)]
#[path = "obligation_outputs/pipeline.rs"]
mod pipeline;
#[allow(dead_code)]
#[path = "support/source.rs"]
mod sources;

use omega_lowering::{MachineRegion, OperationKind, OperationResult, Parameter, PlaceProjection, ReferenceAccess, ReturnEdge, ScalarType, TypeShape};
use omega_typed::{ObligationKind, ValueType};

#[test]
fn returned_borrow_binds_the_descriptor_to_the_declared_machine_region() {
    let bound = "data ByteCell {value:u8;} machine forward<'a>(input:@'a ByteCell)->@'a u8 {@input.value}";
    let second = "data ByteCell {value:u8;} machine forward<'a,'b>(input:@'b ByteCell,other:@'a ByteCell)->@'b u8 {@input.value}";
    // Same actual backing, different promise. Lowering must not change one to
    // justify the other: the checker needs both facts to detect the mismatch.
    let unrelated = second.replace("->@'b", "->@'a");
    let elided = bound.replace("<'a>", "").replace("@'a ", "@");
    for source in [bound, second, &unrelated, &elided] {
        let lowered = pipeline::lower(source);
        let machine = lowered.machine("forward");
        let body = &machine.body;
        let loans = lowered.obligations("ReturnBorrow");
        let [loan] = loans.as_slice() else { panic!("{loans:?}") };
        assert_eq!(lowered.spelling(&loan.site), "@input.value");
        let borrow = pipeline::operation_at(body, &loan.site);
        let OperationResult::Structural(descriptor) = borrow.result else { panic!("{:?}", borrow.result) };
        let OperationKind::BorrowPlace { source: backing, access } = &borrow.kind else { panic!("{:?}", borrow.kind) };
        assert_eq!(Parameter::Structural(backing.root), body.parameters[0]);
        assert_eq!(*access, ReferenceAccess::Shared);
        assert_eq!(backing.path, [PlaceProjection::Referent, PlaceProjection::Field(lowered.package.data[0].fields[0].declaration.clone())]);
        assert_eq!(lowered.package.data[0].fields[0].declaration.path, "ByteCell::value");
        let returns = pipeline::returns(body);
        let [ReturnEdge::Structural { source: returned, result }] = returns.as_slice() else { panic!("{returns:?}") };
        assert_eq!(*returned, descriptor);
        assert_eq!(Some(body.places[result.index()].ty), machine.result_type);
        assert_eq!(body.places[result.index()].ty, body.places[descriptor.index()].ty);
        assert_eq!(lowered.package.types.get(body.places[descriptor.index()].ty), Some(&TypeShape::ScalarReference {
            access: ReferenceAccess::Shared,
            referent: ScalarType::Integer { signed: false, width: 8 },
        }));
        assert_eq!(lowered.package.types.get(body.places[backing.root.index()].ty), Some(&TypeShape::NominalReference {
            access: ReferenceAccess::Shared,
            referent: lowered.package.data[0].id,
        }));
        assert!(machine.arguments.is_empty());
        let bindings = lowered.obligations("BindLifetime");
        if source == elided {
            assert!(bindings.is_empty());
            spec_corpus::observe_handoff!(psi "psi-output/elided-return-borrow", assert_eq!((machine.regions.as_slice(), body.lifetime_bindings.as_slice()), (&[][..], &[][..])));
            continue;
        }
        let [binding] = bindings.as_slice() else { panic!("{bindings:?}") };
        assert_eq!((&binding.site, binding.node), (&loan.site, loan.node));
        let ObligationKind::BindLifetime { reference, destination: ValueType::AtLifetime(inner, region) } = &binding.kind else { panic!("{:?}", binding.kind) };
        assert_eq!(inner.as_ref(), reference);
        let ValueType::Parameter { owner, index } = region.as_ref() else { panic!("{region:?}") };
        assert!(matches!(&loan.kind, ObligationKind::ReturnBorrow { machine } if machine == owner.as_ref()));
        assert_eq!((&owner.package, &owner.path), (&machine.declaration.package, &machine.declaration.path));
        assert_eq!(*index, usize::from(source == second));
        if source == bound {
            spec_corpus::observe_handoff!(psi "psi-output/region-bound-return", assert_eq!(
                (machine.regions.as_slice(), body.lifetime_bindings.as_slice(), *returned, Parameter::Structural(backing.root)),
                (&[MachineRegion { reference_parameters: vec![0], reference_result: true, ..Default::default() }][..], &[(descriptor, 0)][..], descriptor, body.parameters[0]),
            ));
        } else if source == second {
            spec_corpus::observe_handoff!(psi "psi-output/second-region-return", assert_eq!(
                (machine.regions.as_slice(), body.lifetime_bindings.as_slice()),
                (&[MachineRegion { reference_parameters: vec![1], reference_result: false, ..Default::default() },
                   MachineRegion { reference_parameters: vec![0], reference_result: true, ..Default::default() }][..], &[(descriptor, 1)][..]),
            ));
        } else {
            spec_corpus::observe_handoff!(psi "psi-output/region-promise-not-backing", assert_eq!(
                (machine.regions.as_slice(), body.lifetime_bindings.as_slice(), Parameter::Structural(backing.root)),
                (&[MachineRegion { reference_parameters: vec![1], reference_result: true, ..Default::default() },
                   MachineRegion { reference_parameters: vec![0], reference_result: false, ..Default::default() }][..], &[(*returned, 0)][..], body.parameters[0]),
            ));
        }
    }
}
