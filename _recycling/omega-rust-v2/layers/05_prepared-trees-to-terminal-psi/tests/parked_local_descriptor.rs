#[path = "support/source.rs"]
mod source;

use omega_psi::{EmbeddedLoanSource, OperationKind};
use psi_handoff_consumer::parked_local_descriptor::{Backing, local_backing};

const DIR: &str = "package_cases/parked-reborrow-through-local-descriptor";
const CASE: &str = include_str!("fixtures/package_cases/parked-reborrow-through-local-descriptor/case/case.omg");
const CONTROL: &str = include_str!("fixtures/package_cases/parked-reborrow-through-local-descriptor/control/case.omg");

#[test]
fn a_parked_local_descriptor_keeps_its_borrow_operation_and_actual_backing() {
    let (_, local) = source::lower_built_fixture(&format!("{DIR}/case"), "case.omg", CASE);
    let (_, incoming) = source::lower_built_fixture(&format!("{DIR}/control"), "case.omg", CONTROL);
    spec_corpus::observe_handoff!(psi "psi-output/parked-local-descriptor-backing", assert_eq!(
        (local_backing(&local),local_backing(&incoming)),(Ok(Backing::Local(0)),Ok(Backing::Formal(0)))
    ));
}

#[test]
fn two_equal_typed_borrows_distinguish_initializer_formation_and_parked_home() {
    for (selected, slot) in [("first", 0), ("second", 1)] {
        let source = CASE
            .replace(
                "let inner=Inner{key:1u64};",
                "let left=Inner{key:1u64}; let right=Inner{key:1u64}; let first=@left; let second=@right;",
            )
            .replace("link:@inner", &format!("link:{selected}"));
        let mut pkg = source::lower(&[("twins.omg", &source)]).unwrap();
        assert_eq!(local_backing(&pkg), Ok(Backing::Local(slot)));
        let body = &mut pkg
            .machines
            .iter_mut()
            .find(|m| m.declaration.path == "forward")
            .unwrap()
            .body;
        let op = body
            .operations
            .iter()
            .filter(|op| matches!(op.kind, OperationKind::BorrowPlace { .. }))
            .nth(1 - slot)
            .unwrap()
            .id;
        let home = body.suspension_sites[0]
            .retained_homes
            .iter_mut()
            .find(|h| !h.embedded_loans.is_empty())
            .unwrap();
        home.embedded_loans[0].origins[0].source = EmbeddedLoanSource::Borrow { operation: op };
        assert_eq!(local_backing(&pkg), Err("wrong formation"));
    }
}

fn package(
    prefix: &str,
    body: &str,
) -> omega_psi::LoweredPackage {
    source::lower(&[("dependencies.omg", &format!("{prefix} {body}"))]).unwrap()
}

#[test]
fn intermediate_homes_propagate_callee_and_continuation_needs_to_the_actual_backing() {
    const TYPES: &str = "data Inner [copy] {key:u64;} data Middle<'r> [copy] {key:u64;link:@'r Inner;}
        data Outer<'a,'b> [copy] {link:@'a Middle<'b>;} machine wait(k:@u64) suspends {} machine pause()->u64 suspends {0u64}";
    for (use_site, callee, after) in [
        ("suspend wait(@a.link.key); 1u64", true, false),
        ("let n:u64=suspend pause(); a.link.key", false, true),
        ("suspend wait(@a.link.key); a.link.key", true, true),
    ] {
        let text = format!("machine forward()->u64 suspends {{let i=Inner{{key:1u64}};let m=Middle{{key:2u64,link:@i}};let a=Outer{{link:@m}}; {use_site} }}");
        let pkg = package(TYPES, &text);
        let body = &pkg
            .machines
            .iter()
            .find(|m| m.declaration.path == "forward")
            .unwrap()
            .body;
        let site = &body.suspension_sites[0];
        assert_eq!(site.retained_homes.len(), 3);
        let observed = psi_handoff_consumer::parked_copyable_homes(&pkg, body, site).unwrap();
        assert_eq!(observed.len(), 3);
        for home in &observed {
            assert_eq!((home.needed_by_callee, home.needed_after_completion), (callee, after));
        }
        let named = |row: &omega_psi::RetainedHome| {
            let Some(omega_psi::TypeShape::Nominal(id)) = pkg.types.get(row.ty) else {
                panic!("record storage")
            };
            pkg.data[id.get() - 1].declaration.path.as_str()
        };
        let [inner, middle, outer] = site.retained_homes.as_slice() else {
            panic!("three homes")
        };
        assert_eq!([named(inner), named(middle), named(outer)], ["Inner", "Middle", "Outer"]);
        for (owner, backing) in [(outer, middle), (middle, inner)] {
            let EmbeddedLoanSource::Borrow { operation } = owner.embedded_loans[0].origins[0].source else {
                panic!("direct local origin")
            };
            assert!(
                matches!(&body.operations[operation.index()].kind,OperationKind::BorrowPlace{source,..} if source.root==backing.place && source.path.is_empty())
            );
        }
    }
}

#[test]
fn distinct_arrival_edges_retain_only_already_established_backings() {
    use omega_psi::{BlockTerminator, OperationResult, ValueOperand};
    let program = "data Inner [copy] {key:u64;} data Outer<'r> [copy] {key:u64;link:@'r Inner;} machine wait(k:@u64) suspends {}
        machine forward<'r>(flag:bool) suspends { let left=Inner{key:1u64};let right=Inner{key:2u64};
        let a:Outer<'r> = match flag {true -> Outer{key:3u64,link:@left},false -> Outer{key:4u64,link:@right},}; suspend wait(@a.link.key);}";
    let pkg = source::lower(&[("arrivals.omg", program)]).unwrap();
    let body = &pkg
        .machines
        .iter()
        .find(|m| m.declaration.path == "forward")
        .unwrap()
        .body;
    let [site] = body.suspension_sites.as_slice() else {
        panic!("one parked crossing")
    };
    assert_eq!(site.retained_homes.len(), 3);
    let home = site
        .retained_homes
        .iter()
        .find(|h| !h.embedded_loans.is_empty())
        .unwrap();
    let [loan] = home.embedded_loans.as_slice() else {
        panic!("one stored reference")
    };
    assert_eq!(loan.origins.len(), 2);
    let mut backings = Vec::new();
    for origin in &loan.origins {
        let [arrival] = origin.arrival.as_slice() else { panic!("one arriving edge") };
        let block = body
            .blocks
            .iter()
            .find(|b| matches!(&b.terminator,BlockTerminator::Jump(j) if j.id==*arrival))
            .unwrap();
        let EmbeddedLoanSource::Borrow { operation } = origin.source else {
            panic!("local origin")
        };
        assert!(block.operations.contains(&operation.index()));
        let OperationKind::BorrowPlace { source, .. } = &body.operations[operation.index()].kind else {
            panic!("formation")
        };
        assert!(
            site.retained_homes
                .iter()
                .any(|h| h.place == source.root && h.needed_by_callee && !h.needed_after_completion)
        );
        assert!(
            body.operations[body.blocks[0].operations.clone()]
                .iter()
                .any(|op| op.result == OperationResult::Structural(source.root))
        );
        let linked = body.operations[block.operations.clone()]
            .iter()
            .find_map(|op| match &op.kind {
                OperationKind::EstablishRecord { fields } => fields
                    .iter()
                    .find(|f| f.field.path == "Outer::link")
                    .map(|f| &f.operand),
                _ => None,
            })
            .unwrap();
        assert!(matches!(linked,ValueOperand::CopyWhole(input) if body.operations[operation.index()].result==OperationResult::Structural(*input)));
        backings.push(source.root);
    }
    assert_ne!(backings[0], backings[1]);
}

#[test]
fn an_owned_backing_keeps_its_own_custody_and_gets_no_copyable_home() {
    let program = CASE.replace("data Inner [copy]", "data Inner");
    let pkg = source::lower(&[("owned.omg", &program)]).unwrap();
    let body = &pkg
        .machines
        .iter()
        .find(|m| m.declaration.path == "forward")
        .unwrap()
        .body;
    let site = &body.suspension_sites[0];
    let [outer] = site.retained_homes.as_slice() else {
        panic!("only Outer needs a copyable home")
    };
    let [inner] = site.retained_owners.as_slice() else {
        panic!("one owned backing")
    };
    let EmbeddedLoanSource::Borrow { operation } = outer.embedded_loans[0].origins[0].source else {
        panic!("local borrow")
    };
    assert!(matches!(&body.operations[operation.index()].kind,OperationKind::BorrowPlace{source,..} if source.root==inner.place));
    assert_eq!(inner.custody, omega_psi::OwnedCustody::Caller);
}
