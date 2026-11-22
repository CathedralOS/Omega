#[path = "support/source.rs"]
mod source;

use omega_psi::{CallSite, EmbeddedLoanOrigin, EmbeddedLoanSource, OperationKind, Parameter, PlaceOrigin, PlaceProjection, ReferenceAccess, ValueOperand};
use psi_handoff_consumer::parked_copyable_homes;

#[test]
fn reborrowing_through_a_copyable_home_keeps_the_stored_parent() {
    for (binding, actual) in [("", "@a.link.key"), ("let loan:@u64 = @a.link.key;", "loan")] {
        let program = format!(
            "data Inner [copy] {{key:u64;}}
             data Outer<'r> [copy] {{key:u64;link:@'r Inner;}}
             machine wait(k:@u64) suspends {{}}
             machine forward<'r>(a:Outer<'r>,other:Outer<'r>) suspends {{{binding} suspend wait({actual});}}"
        );
        let package = source::lower(&[("reborrow.omg", &program)]).unwrap();
        let body = &package.machines.iter().find(|machine| machine.declaration.path == "forward").unwrap().body;
        let [Parameter::Structural(a), Parameter::Structural(_)] = body.parameters.as_slice() else { panic!("two record formals") };
        let [site] = body.suspension_sites.as_slice() else { panic!("one suspension") };
        let CallSite::Operation(call) = site.call else { panic!("ordinary call") };
        let OperationKind::Call { arguments, .. } = &body.operations[call.index()].kind else { panic!("call operation") };
        let [ValueOperand::Borrow { place, access: ReferenceAccess::Shared }] = arguments.as_slice() else { panic!("one shared argument") };
        let captured = if binding.is_empty() {
            assert!(site.retained_loans.is_empty());
            place
        } else {
            let [loan] = site.retained_loans.as_slice() else { panic!("one retained named loan") };
            assert_eq!(loan.descriptor, place.root);
            assert!(loan.needed_by_callee && !loan.needed_after_completion);
            assert_eq!(place.path, [PlaceProjection::Referent]);
            let PlaceOrigin::Operation(borrow) = body.places[place.root.index()].origin else { panic!("named borrow definition") };
            let OperationKind::BorrowPlace { source, access: ReferenceAccess::Shared } = &body.operations[borrow.index()].kind else {
                panic!("shared borrow operation")
            };
            source
        };
        assert_eq!(captured.root, *a);
        let [PlaceProjection::Field(link), PlaceProjection::Referent, PlaceProjection::Field(key)] = captured.path.as_slice() else {
            panic!("the selected scalar is beyond the stored reference")
        };
        assert_eq!(link.path, "Outer::link");
        assert_eq!(key.path, "Inner::key");
        assert!(site.retained_owners.is_empty());
        let [home] = site.retained_homes.as_slice() else { panic!("only the home containing the parent reference") };
        assert_eq!(home.place, *a);
        assert!(home.needed_by_callee && !home.needed_after_completion);
        let homes = parked_copyable_homes(&package, body, site).unwrap();
        let [observed] = homes.as_slice() else { panic!("one independently recovered home") };
        assert_eq!(observed.place, *a);
        assert_eq!(observed.formal, Some(0));
        assert!(observed.needed_by_callee && !observed.needed_after_completion);
        let [parent] = home.embedded_loans.as_slice() else { panic!("one stored parent") };
        let path = vec![PlaceProjection::Field(link.clone())];
        assert_eq!(parent.path, path);
        assert_eq!(parent.access, ReferenceAccess::Shared);
        assert_eq!(
            parent.origins,
            [EmbeddedLoanOrigin { arrival: vec![], source: EmbeddedLoanSource::Formal { formal: *a, path }, reborrows: vec![] }]
        );
    }
}
