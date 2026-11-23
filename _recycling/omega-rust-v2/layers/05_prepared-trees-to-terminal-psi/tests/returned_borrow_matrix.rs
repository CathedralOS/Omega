#[path = "support/source.rs"]
mod source;

use omega_psi::{
    BlockTerminator, CallArgument, CallSite, CallTarget, MachineBody, OperationKind, OperationResult, Parameter, PlaceId, PlaceProjection, ProjectedPlace,
    ReferenceAccess, ReturnEdge, SuspensionSite,
};

#[derive(Clone, Copy, PartialEq)]
enum Shape {
    Record,
    Reference,
}

#[derive(Clone, Copy, PartialEq)]
enum After {
    Read,
    Wait,
    RebindAndWait,
}

fn program(
    slot: usize,
    shape: Shape,
    pauses: bool,
    after: After,
) -> String {
    let homes = ["left", "right"];
    let (home, other) = (homes[slot], homes[1 - slot]);
    let (result_type, returned, read) = match shape {
        Shape::Record => ("Box<'r>", "k", "$item.r"),
        Shape::Reference => ("@'r u8", "k.r", "$item"),
    };
    let (effect, call) = if pauses { (" suspends", "suspend ") } else { ("", "") };
    let next = match after {
        After::Read => String::new(),
        After::Wait => "suspend wait();".into(),
        After::RebindAndWait => format!("seed.r = @{other}.v; suspend wait();"),
    };
    format!(
        "data Home [copy] {{ v:u8; }}
        data Box<'r> [copy] {{ r:@'r u8; }}
        machine pick<'r>(k:Box<'r>)->{result_type}{effect} {{ {returned} }}
        machine wait() suspends {{}}
        machine read(left:Home,right:Home)->u8 suspends {{
            let mutable seed=Box{{r:@{home}.v}};
            let item={call}pick(seed);
            {next}
            {read}
        }}"
    )
}

fn field(
    path: &[PlaceProjection],
    name: &str,
) -> bool {
    matches!(path, [PlaceProjection::Field(id)] if id.path == name)
}

fn box_referent(path: &[PlaceProjection]) -> bool {
    matches!(path, [PlaceProjection::Field(id), PlaceProjection::Referent] if id.path == "Box::r")
}

// Look up the borrow that established the descriptor. No compiler provenance
// or liveness routine participates in the expected answer.
fn borrow(
    body: &MachineBody,
    descriptor: PlaceId,
) -> &ProjectedPlace {
    body.operations
        .iter()
        .find_map(|op| match &op.kind {
            OperationKind::BorrowPlace {
                source,
                access: ReferenceAccess::Shared,
            } if op.result == OperationResult::Structural(descriptor) => Some(source),
            _ => None,
        })
        .expect("shared borrow that established the descriptor")
}

fn correct_wait_home(
    body: &MachineBody,
    site: &SuspensionSite,
    expected: PlaceId,
) -> bool {
    let mut inputs = site
        .retained_homes
        .iter()
        .filter(|row| body.parameters.contains(&Parameter::Structural(row.place)));
    matches!(inputs.next(), Some(row) if row.place == expected && row.ty == body.places[expected.index()].ty
        && !row.needed_by_callee && row.needed_after_completion)
        && inputs.next().is_none()
}

fn run(
    slot: usize,
    shape: Shape,
    pauses: bool,
    after: After,
) {
    let text = program(slot, shape, pauses, after);
    let pkg = source::lower(&[("family.omg", &text)]).unwrap_or_else(|error| panic!("lowering refused: {error}\n{text}"));
    let machine = |name| {
        pkg.machines
            .iter()
            .find(|m| m.declaration.path == name)
            .unwrap()
    };
    let pick = machine("pick");
    let [Parameter::Structural(formal)] = pick.body.parameters[..] else {
        panic!("pick takes the seed record")
    };
    let [exit] = pick.body.blocks.as_slice() else { panic!("unconditional pick") };
    let BlockTerminator::Return {
        result: ReturnEdge::Structural { source: returned, .. },
        ..
    } = exit.terminator
    else {
        panic!("pick returns a descriptor or a record")
    };
    match shape {
        Shape::Record => assert_eq!(returned, formal),
        Shape::Reference => {
            let source = borrow(&pick.body, returned);
            assert!(source.root == formal && box_referent(&source.path));
        }
    }

    let body = &machine("read").body;
    let home = |i: usize| match body.parameters[i] {
        Parameter::Structural(p) => p,
        _ => panic!("Home is a record"),
    };
    let call_to = |target| {
        body.operations
            .iter()
            .find(|op| matches!(op.kind, OperationKind::Call { callee: CallTarget::Machine(id), .. } if id == target))
    };
    let call = call_to(pick.id).expect("pick called");
    let OperationResult::Structural(item) = call.result else {
        panic!("completed call result")
    };
    let OperationKind::Call { arguments, .. } = &call.kind else { unreachable!() };
    let [CallArgument::CopyWhole(seed)] = arguments.as_slice() else {
        panic!("copyable seed argument")
    };
    let init = body.operations[..call.id.index()]
        .iter()
        .find(|op| op.result == OperationResult::Structural(*seed))
        .unwrap();
    let OperationKind::EstablishRecord { fields } = &init.kind else {
        panic!("seed initialization")
    };
    let [f] = fields.as_slice() else { panic!("seed.r") };
    assert_eq!(f.field.path, "Box::r");
    let CallArgument::CopyWhole(descriptor) = f.operand else {
        panic!("shared descriptor")
    };
    let initial = borrow(body, descriptor);
    assert!(initial.root == home(slot) && field(&initial.path, "Home::v"));

    let read = body
        .operations
        .iter()
        .find(|op| matches!(op.kind, OperationKind::ProjectedScalarRead { .. }))
        .expect("final dereference");
    let OperationKind::ProjectedScalarRead { source, .. } = &read.kind else {
        unreachable!()
    };
    assert!(call.id.index() < read.id.index() && source.root == item);
    assert!(match shape {
        Shape::Record => box_referent(&source.path),
        Shape::Reference => source.path == [PlaceProjection::Referent],
    });
    let OperationResult::Scalar(value) = read.result else {
        panic!("loaded scalar")
    };
    assert!(
        body.blocks
            .iter()
            .any(|b| matches!(b.terminator, BlockTerminator::Return { result: ReturnEdge::Scalar(id), .. } if id == value))
    );

    assert_eq!(body.suspension_sites.len(), usize::from(pauses) + usize::from(after != After::Read));
    let producer_site = body
        .suspension_sites
        .iter()
        .find(|s| s.call == CallSite::Operation(call.id));
    assert_eq!(producer_site.is_some(), pauses);
    if let Some(site) = producer_site {
        assert!(
            site.retained_homes
                .iter()
                .any(|row| row.place == home(slot) && row.needed_by_callee)
        );
        assert!(site.retained_homes.iter().all(|row| row.place != item));
        assert!(site.retained_loans.iter().all(|row| row.descriptor != item));
    }
    let wait = call_to(machine("wait").id);
    if after == After::Read {
        assert!(wait.is_none());
        return;
    }
    let wait = wait.unwrap();
    assert!(call.id.index() < wait.id.index() && wait.id.index() < read.id.index());
    let stores: Vec<_> = body.operations[call.id.index() + 1..wait.id.index()]
        .iter()
        .filter_map(|op| match &op.kind {
            OperationKind::StructuralStore { destination, value, .. } => Some((destination, value)),
            _ => None,
        })
        .collect();
    if after == After::RebindAndWait {
        let [(dst, value)] = stores[..] else { panic!("one descriptor reassignment") };
        let rebound = borrow(body, *value);
        assert!(dst.root == *seed && field(&dst.path, "Box::r"));
        assert!(rebound.root == home(1 - slot) && field(&rebound.path, "Home::v"));
    } else {
        assert!(stores.is_empty());
    }

    let site = body
        .suspension_sites
        .iter()
        .find(|s| s.call == CallSite::Operation(wait.id))
        .unwrap();
    assert!(correct_wait_home(body, site, home(slot)), "wrong backing at wait: {:?}", site.retained_homes);
    match shape {
        Shape::Record => assert!(
            site.retained_homes
                .iter()
                .any(|r| r.place == item && r.needed_after_completion)
        ),
        Shape::Reference => assert!(
            site.retained_loans
                .iter()
                .any(|r| r.descriptor == item && r.access == ReferenceAccess::Shared && r.needed_after_completion)
        ),
    }
    let mut corrupted = site.clone();
    let pos = corrupted
        .retained_homes
        .iter()
        .position(|r| r.place == home(slot))
        .unwrap();
    corrupted.retained_homes[pos].place = home(1 - slot);
    assert!(!correct_wait_home(body, &corrupted, home(slot)), "swapped home escaped detection");
    corrupted.retained_homes.remove(pos);
    assert!(!correct_wait_home(body, &corrupted, home(slot)), "missing home escaped detection");
}

// Separate test names keep the entire matrix visible when one or more cells fail.
macro_rules! family {
    ($name:ident, $slot:expr, $shape:expr) => {
        mod $name {
            use super::*;
            #[test]
            fn sync_read() {
                run($slot, $shape, false, After::Read);
            }
            #[test]
            fn sync_wait() {
                run($slot, $shape, false, After::Wait);
            }
            #[test]
            fn sync_rebind_wait() {
                run($slot, $shape, false, After::RebindAndWait);
            }
            #[test]
            fn suspending_read() {
                run($slot, $shape, true, After::Read);
            }
            #[test]
            fn suspending_wait() {
                run($slot, $shape, true, After::Wait);
            }
            #[test]
            fn suspending_rebind_wait() {
                run($slot, $shape, true, After::RebindAndWait);
            }
        }
    };
}
family!(left_record, 0, Shape::Record);
family!(right_record, 1, Shape::Record);
family!(left_reference, 0, Shape::Reference);
family!(right_reference, 1, Shape::Reference);
