//! Borrowed storage after the stores-and-windows and stored-exclusive repairs
//! met: stores below a field-held `@mut`, write-only loans at joins and in
//! fields, second names for exclusive loans, and descriptor stores resolved by
//! their newest prefix. Each test lowers exact source bytes and checks the
//! published Psi; L6 still judges every lifetime, restoration and use.
#[path = "support/source.rs"]
mod source;

use omega_lowering::{
    BlockTerminator, CallArgument, CallSite, LoweredPackage, MachineBody, OperationKind, OperationResult, Parameter, PlaceId, PlaceOrigin, PlaceProjection,
    ProjectedPlace, ReferenceAccess, StructuralStoreDisposition, TypeShape,
};
use psi_handoff_consumer::{ExclusiveOrigin, ReturnedExclusive, StoreKind, parked_windows, returned_exclusive, window_stores, write_only_stores};

fn lower(text: &str) -> LoweredPackage {
    source::lower(&[("case.omg", text)]).unwrap()
}

/// L5 refuses `text` at the one occurrence of `at` inside `context`, with
/// `message`; the package command replays the same bytes independently.
fn refusal(
    text: &str,
    context: &str,
    at: &str,
    message: &str,
) {
    source::refuse(&[("case.omg", text)], "case.omg", source::span_within(text, context, at), message);
}

fn typer_refusal(text: &str) -> String {
    let package = source::resolve(&[("case.omg", text)]);
    match omega_preparation::prepare_source_package_diagnosed(&package) {
        Err(omega_preparation::PreparationFailure::Typing(error)) => error.message,
        other => panic!("expected a typing refusal, got {:?}", other.map(|_| ())),
    }
}

fn body<'a>(
    package: &'a LoweredPackage,
    name: &str,
) -> &'a MachineBody {
    &package
        .machines
        .iter()
        .find(|machine| machine.declaration.path == name)
        .expect("machine")
        .body
}

fn body_mut<'a>(
    package: &'a mut LoweredPackage,
    name: &str,
) -> &'a mut MachineBody {
    &mut package
        .machines
        .iter_mut()
        .find(|machine| machine.declaration.path == name)
        .expect("machine")
        .body
}

fn formal(
    body: &MachineBody,
    index: usize,
) -> PlaceId {
    match body.parameters[index] {
        Parameter::Structural(place) => place,
        Parameter::Scalar(_) => panic!("formal {index} is a scalar"),
    }
}

/// Field declarations by name, with `*` for each loan crossing.
fn spell(path: &[PlaceProjection]) -> Vec<String> {
    path.iter()
        .map(|step| match step {
            PlaceProjection::Field(field) => field.path.clone(),
            PlaceProjection::Referent => "*".to_string(),
            other => format!("{other:?}"),
        })
        .collect()
}

fn stores(body: &MachineBody) -> Vec<(&ProjectedPlace, &StructuralStoreDisposition)> {
    body.operations
        .iter()
        .filter_map(|op| match &op.kind {
            OperationKind::StructuralStore { destination, disposition, .. } => Some((destination, disposition)),
            _ => None,
        })
        .collect()
}

fn borrowed_moves(body: &MachineBody) -> Vec<&ProjectedPlace> {
    body.operations
        .iter()
        .filter_map(|op| match &op.kind {
            OperationKind::BorrowedMove { source } => Some(source),
            _ => None,
        })
        .collect()
}

const FIELD_HELD: &str = "data T{v:u8;} data Leaf{t:T;} data Node{next:@mut Leaf;}";

/// `x.next.t` is storage the caller lent through the `@mut` that the owned
/// record `x` holds. Restoring it after a move fills the vacancy; it never
/// disposes the moved value. Before the integration repair, a path that began
/// at an owned field took the owned-store route and published `Dispose` of the
/// vacated place, while the same program through `x:@mut Node` filled it.
#[test]
fn a_store_below_a_field_held_exclusive_loan_is_a_store_into_borrowed_storage() {
    for (parent, prefix) in [("Node", vec![]), ("@mut Node", vec!["*"])] {
        let text = format!("{FIELD_HELD} machine f(x:{parent}){{ let o = move x.next.t; x.next.t = move o; }}");
        let mut package = lower(&text);
        let f = body(&package, "f");
        let x = formal(f, 0);
        let path: Vec<String> = prefix
            .iter()
            .copied()
            .chain(["Node::next", "*", "Leaf::t"])
            .map(str::to_string)
            .collect();
        let [moved] = borrowed_moves(f)[..] else {
            panic!("one borrowed move for {parent}")
        };
        assert_eq!((moved.root, spell(&moved.path)), (x, path.clone()), "{parent}");
        let [(destination, disposition)] = stores(f)[..] else {
            panic!("one store for {parent}")
        };
        assert_eq!(destination, moved, "{parent}: the store restores the exact vacated place");
        assert_eq!(disposition, &StructuralStoreDisposition::FillVacancy, "{parent}");
        // The Psi-only reader sees one store that fills, at the vacated place.
        let stored = window_stores(&package, "f").unwrap();
        assert_eq!(
            stored
                .iter()
                .map(|(_, place, kind)| (spell(&place.path), *kind))
                .collect::<Vec<_>>(),
            [(path.clone(), StoreKind::Fill)]
        );

        // Corrupted product: the restoring store displaces a value instead.
        for op in &mut body_mut(&mut package, "f").operations {
            if let OperationKind::StructuralStore { disposition, .. } = &mut op.kind {
                *disposition = StructuralStoreDisposition::ReplaceUnrestricted;
            }
        }
        let stored = window_stores(&package, "f").unwrap();
        assert_eq!(stored.iter().map(|(_, _, kind)| *kind).collect::<Vec<_>>(), [StoreKind::Displace], "{parent}");
    }

    // Replacing intact borrowed content extracts the old value through the same
    // loan and disposes it on the continuation; the store itself then fills.
    for parent in ["Node", "@mut Node"] {
        let text = format!("{FIELD_HELD} machine f(x:{parent}){{ x.next.t = T{{v:1u8}}; }}");
        let package = lower(&text);
        let f = body(&package, "f");
        let [old] = borrowed_moves(f)[..] else {
            panic!("the old value is extracted once for {parent}")
        };
        let [(destination, disposition)] = stores(f)[..] else {
            panic!("one store for {parent}")
        };
        assert_eq!((destination, disposition), (old, &StructuralStoreDisposition::FillVacancy), "{parent}");
        assert!(
            !f.operations.iter().any(|op| matches!(
                &op.kind,
                OperationKind::StructuralStore {
                    disposition: StructuralStoreDisposition::Dispose(_),
                    ..
                }
            )),
            "{parent}: no owned disposal of lent content"
        );
    }
}

const CELL: &str = "data Cell{value:u8;}";

/// The operand each jump into the one structural join supplies, in edge order.
fn arrivals(body: &MachineBody) -> Vec<CallArgument> {
    let join = body
        .blocks
        .iter()
        .find(|block| !block.structural_parameters.is_empty())
        .expect("a structural join")
        .id;
    body.blocks
        .iter()
        .filter_map(|block| match &block.terminator {
            BlockTerminator::Jump(edge) if edge.target == join => Some(edge.structural_arguments.clone()),
            _ => None,
        })
        .flatten()
        .collect()
}

/// An independent consumer: every exclusive descriptor that arrives at a join
/// must arrive by transfer.
fn joins_transfer_exclusive_loans(
    package: &LoweredPackage,
    name: &str,
) -> Result<(), &'static str> {
    let f = body(package, name);
    for operand in arrivals(f) {
        let place = match operand {
            CallArgument::MoveWhole(_) => continue,
            CallArgument::CopyWhole(place) => place,
            _ => return Err("a join operand is not a whole descriptor"),
        };
        if matches!(
            package.types.get(f.places[place.index()].ty),
            Some(TypeShape::NominalReference {
                access: ReferenceAccess::Mutable | ReferenceAccess::WriteOnly,
                ..
            })
        ) {
            return Err("an exclusive loan is copied onto a join edge");
        }
    }
    Ok(())
}

/// `@write T` is an exclusive loan. Choosing one at a `match` transfers each
/// arriving formal, as for `@mut`, and never copies it.
#[test]
fn a_write_only_loan_chosen_by_match_transfers_each_arriving_formal() {
    for access in ["write", "mut"] {
        let text = format!("{CELL} machine f(c:bool, a:@{access} Cell, b:@{access} Cell) {{ let w = match c {{true -> a, false -> b,}}; w.value = 7; }}");
        let mut package = lower(&text);
        let f = body(&package, "f");
        let (a, b) = (formal(f, 1), formal(f, 2));
        assert_eq!(arrivals(f), [CallArgument::MoveWhole(a), CallArgument::MoveWhole(b)], "@{access}");
        let expected = if access == "write" {
            ReferenceAccess::WriteOnly
        } else {
            ReferenceAccess::Mutable
        };
        let join = f
            .blocks
            .iter()
            .find(|block| !block.structural_parameters.is_empty())
            .unwrap();
        let [joined] = join.structural_parameters[..] else {
            panic!("one joined loan")
        };
        assert!(matches!(package.types.get(f.places[joined.index()].ty), Some(TypeShape::NominalReference { access, .. }) if *access == expected));
        assert!(f.operations.iter().any(|op| matches!(&op.kind,
            OperationKind::StructuralScalarFieldStore { destination, .. } if destination.root == joined && destination.path == [PlaceProjection::Referent])));
        assert_eq!(joins_transfer_exclusive_loans(&package, "f"), Ok(()));

        // Corrupted product: copying the loan onto an edge duplicates its authority.
        let f = body_mut(&mut package, "f");
        let join = f
            .blocks
            .iter()
            .find(|block| !block.structural_parameters.is_empty())
            .unwrap()
            .id;
        for block in &mut f.blocks {
            if let BlockTerminator::Jump(edge) = &mut block.terminator
                && edge.target == join
                && let Some(CallArgument::MoveWhole(place)) = edge.structural_arguments.first().cloned()
            {
                edge.structural_arguments[0] = CallArgument::CopyWhole(place);
            }
        }
        assert_eq!(
            joins_transfer_exclusive_loans(&package, "f"),
            Err("an exclusive loan is copied onto a join edge")
        );
    }
}

const WRITE_FIELD: &str = "data Leaf{v:u8;} data Node{next:@write Leaf;}";

/// A stored `@write` leaves owned storage by transfer, opens a window when
/// moved through a `@mut` parent, and reborrows with its own access.
#[test]
fn a_stored_write_only_reference_transfers_or_reborrows_with_its_own_access() {
    let read = |body: &str| returned_exclusive(&lower(&format!("{WRITE_FIELD} {body}")), "read");
    let cases = [
        ("machine read(x:Node)->@write Leaf{x.next}", ExclusiveOrigin::Transfer, vec!["Node::next"]),
        (
            "machine read(x:@mut Node)->@write Leaf{x.next}",
            ExclusiveOrigin::BorrowedMove,
            vec!["*", "Node::next"],
        ),
        (
            "machine read(x:@mut Node)->@write Leaf{@write $(x.next)}",
            ExclusiveOrigin::Reborrow,
            vec!["*", "Node::next", "*"],
        ),
    ];
    for (text, origin, path) in &cases {
        let (origin, found) = (*origin, read(text).unwrap_or_else(|error| panic!("{text}: {error}")));
        assert_eq!(
            (found.origin, found.formal, found.path.clone(), found.moves_of_source),
            (
                origin,
                0,
                path.iter().map(|s| s.to_string()).collect(),
                usize::from(origin != ExclusiveOrigin::Reborrow)
            ),
            "{text}"
        );
    }
    let owned: ReturnedExclusive = read(cases[0].0).unwrap();
    assert!(owned.cleanup.is_empty(), "the transferred field is not disposed with x: {:?}", owned.cleanup);

    // Corrupted products.
    let mut copied = lower(&format!("{WRITE_FIELD} {}", cases[0].0));
    for op in &mut body_mut(&mut copied, "read").operations {
        if let OperationKind::MoveStructural { source } = &op.kind {
            op.kind = OperationKind::CopyStructural { source: source.clone() };
        }
    }
    assert_eq!(returned_exclusive(&copied, "read"), Err("an exclusive reference is copied"));
    for (access, error) in [
        (ReferenceAccess::Mutable, "a reborrow changed its exclusive access"),
        (ReferenceAccess::Shared, "a mutable reborrow became shared"),
    ] {
        let mut widened = lower(&format!("{WRITE_FIELD} {}", cases[2].0));
        for op in &mut body_mut(&mut widened, "read").operations {
            if let OperationKind::BorrowPlace { access: current, .. } = &mut op.kind {
                *current = access;
            }
        }
        assert_eq!(returned_exclusive(&widened, "read"), Err(error));
    }
    // Taking through a write-only loan reads it: the typer refuses.
    assert_eq!(
        typer_refusal(&format!("{WRITE_FIELD} machine read(x:@write Node)->@write Leaf{{x.next}}")),
        "cannot read through a write-only reference"
    );
}

/// Writing through a stored `@write` needs that loan's access, not a `mut`
/// owner, and may follow a `@mut` crossing
/// (terminal-psi/loans.md#reborrow-lineage-and-access). The store stays the
/// same scalar field store, and nothing reads the leaf.
#[test]
fn a_write_through_a_stored_write_only_reference_keeps_its_crossings() {
    for (parent, path) in [
        ("Node", vec!["Node::next", "*"]),
        ("@mut Node", vec!["*", "Node::next", "*"]),
    ] {
        let package = lower(&format!("{WRITE_FIELD} machine f(x:{parent}){{x.next.v = 1u8;}}"));
        let f = body(&package, "f");
        let stores: Vec<_> = f
            .operations
            .iter()
            .filter_map(|op| match &op.kind {
                OperationKind::StructuralScalarFieldStore { destination, field, .. } => Some((destination.root, spell(&destination.path), field.path.clone())),
                OperationKind::Constant(_) => None,
                other => panic!("{parent}: unexpected operation {other:?}"),
            })
            .collect();
        assert_eq!(
            stores,
            [(formal(f, 0), path.iter().map(|s| s.to_string()).collect(), "Leaf::v".to_string())],
            "{parent}"
        );
    }
    // A write-only crossing may not be followed by another one: that reads the stored reference.
    let mut_field = "data Leaf{v:u8;} data Node{next:@mut Leaf;}";
    assert_eq!(
        typer_refusal(&format!("{mut_field} machine f(x:@write Node){{x.next.v = 1u8;}}")),
        "cannot read through a write-only reference"
    );
    assert_eq!(
        typer_refusal("data Leaf{v:u8;} data Node{next:@Leaf;} machine f(x:@mut Node){x.next.v = 1u8;}"),
        "place requires writable authority"
    );
    assert_eq!(
        typer_refusal(&format!("{WRITE_FIELD} machine look(v:@u8) {{}} machine f(x:@mut Node){{look(@x.next.v)}}")),
        "cannot read through a write-only reference"
    );
}

/// `let b = a;` with an exclusive `a` transfers the loan into a new place,
/// for `@write` exactly as for `@mut`. Naming `a` again transfers it again;
/// the checker judges that reuse. Each store goes through its own binding.
#[test]
fn a_second_name_for_an_exclusive_reference_transfers_into_its_own_place() {
    for access in ["write", "mut"] {
        let mut package = lower(&format!(
            "{CELL} machine f(a:@{access} Cell) {{ let b = a; let c = a; b.value = 1; c.value = 2; }}"
        ));
        let f = body(&package, "f");
        let a = formal(f, 0);
        let moves: Vec<_> = f
            .operations
            .iter()
            .filter_map(|op| match (&op.kind, op.result) {
                (OperationKind::MoveStructural { source }, OperationResult::Structural(to)) => Some((source.root, source.path.is_empty(), to)),
                _ => None,
            })
            .collect();
        let [(first, true, b), (second, true, c)] = moves[..] else {
            panic!("@{access}: two whole transfers: {moves:?}")
        };
        assert_eq!((first, second), (a, a), "@{access}");
        assert!(b != c && b != a && c != a, "@{access}");
        let stored: Vec<_> = f
            .operations
            .iter()
            .filter_map(|op| match &op.kind {
                OperationKind::StructuralScalarFieldStore { destination, .. } => Some(destination.root),
                _ => None,
            })
            .collect();
        assert_eq!(stored, [b, c], "@{access}");

        // One binding: the Psi-only reader names the transfer; a copied one is not a transfer.
        package = lower(&format!("{CELL} machine f(a:@{access} Cell) {{ let b = a; b.value = 1; }}"));
        let (source, destination, _) = psi_handoff_consumer::local_descriptor_move(body(&package, "f")).unwrap();
        assert_eq!(source, formal(body(&package, "f"), 0));
        for op in &mut body_mut(&mut package, "f").operations {
            if let OperationKind::MoveStructural { source } = &op.kind {
                op.kind = OperationKind::CopyStructural { source: source.clone() };
            }
        }
        assert_eq!(psi_handoff_consumer::local_descriptor_move(body(&package, "f")), Err("missing descriptor move"));
        assert_ne!(source, destination);
    }
}

const HOLDER: &str = "data Holder{r:@u8;} machine wait() suspends {}";
const NESTED: &str = "data Inner{r:@u8;} data Outer{i:Inner;} machine wait() suspends {}";
const TWO_RECORDS: &str = "data A{r:@u8;} data B{r:@u8;} data H{a:A; b:B;} machine wait() suspends {}";

/// For the one parked owner of `forward`, each embedded descriptor path and the
/// position of the formal whose loan it holds.
fn embedded_origins(package: &LoweredPackage) -> Vec<(Vec<String>, usize)> {
    let f = body(package, "forward");
    let [site] = &f.suspension_sites[..] else { panic!("one parked call") };
    let [owner] = &site.retained_owners[..] else { panic!("one parked owner") };
    owner
        .embedded_loans
        .iter()
        .map(|loan| {
            let position = f
                .parameters
                .iter()
                .position(|p| *p == Parameter::Structural(loan.origin.formal))
                .expect("an incoming formal");
            assert!(loan.origin.path.is_empty() && loan.origin.reborrows.is_empty());
            (spell(&loan.path), position)
        })
        .collect()
}

/// Inside one block, a home's descriptor comes from the newest store at a
/// prefix of its path: a whole or parent store replaces earlier field stores,
/// and `A::r` and `B::r` are different fields. A join, a store in an earlier
/// block or a writable alias leaves no single origin, so the crossing refuses.
#[test]
fn a_parked_home_names_the_newest_store_at_each_descriptor_prefix() {
    let origins = |text: String| embedded_origins(&lower(&text));
    let row = |path: &[&str], formal: usize| (path.iter().map(|s| s.to_string()).collect::<Vec<_>>(), formal);
    let forward = "machine forward(first:@u8,second:@u8) suspends";
    assert_eq!(
        origins(format!(
            "{HOLDER} {forward} {{let mut h=Holder{{r:first}};h.r=second;h=Holder{{r:first}};suspend wait()}}"
        )),
        [row(&["Holder::r"], 0)]
    );
    assert_eq!(
        origins(format!("{HOLDER} {forward} {{let mut h=Holder{{r:first}};h.r=second;suspend wait()}}")),
        [row(&["Holder::r"], 1)]
    );
    assert_eq!(
        origins(format!(
            "{NESTED} {forward} {{let mut o=Outer{{i:Inner{{r:first}}}};o.i.r=second;o.i=Inner{{r:first}};suspend wait()}}"
        )),
        [row(&["Outer::i", "Inner::r"], 0)]
    );
    assert_eq!(
        origins(format!(
            "{NESTED} {forward} {{let mut o=Outer{{i:Inner{{r:first}}}};o.i=Inner{{r:first}};o.i.r=second;suspend wait()}}"
        )),
        [row(&["Outer::i", "Inner::r"], 1)]
    );
    assert_eq!(
        origins(format!(
            "{TWO_RECORDS} {forward} {{let mut h=H{{a:A{{r:first}},b:B{{r:first}}}};h.a.r=second;suspend wait()}}"
        )),
        [row(&["H::a", "A::r"], 1), row(&["H::b", "B::r"], 0)]
    );
    assert_eq!(
        origins(format!(
            "{TWO_RECORDS} {forward} {{let mut h=H{{a:A{{r:first}},b:B{{r:first}}}};h.b.r=second;suspend wait()}}"
        )),
        [row(&["H::a", "A::r"], 0), row(&["H::b", "B::r"], 1)]
    );
    const UNTRACED: &str = "parking this owner requires the backing or carry evidence of its embedded descriptors";
    let branch = "machine forward(first:@u8,second:@u8,c:bool) suspends";
    for text in [
        format!("{HOLDER} {branch} {{let mut h=Holder{{r:first}};h.r=match c {{true -> second, false -> first,}};suspend wait()}}"),
        format!("{HOLDER} {branch} {{let mut h=Holder{{r:first}};h.r=second;let n=match c {{true -> 1u8, false -> 2u8,}};suspend wait()}}"),
        format!(
            "data Holder{{r:@u8;}} machine poke(h:@mut Holder) {{}} machine wait() suspends {{}} {forward} {{let mut h=Holder{{r:first}};poke(@mut h);h.r=second;suspend wait()}}"
        ),
    ] {
        refusal(&text, "suspend wait()", "wait()", UNTRACED);
    }
}

/// A window names only its own lender. A store through another `@mut` lender
/// is neither a restoration nor an overlap.
#[test]
fn a_parked_window_names_only_its_own_lender() {
    let text = "data T{v:u8;} data Cell{t:T; u:T;} machine wait() suspends {} machine f(p:@mut Cell, q:@mut Cell) suspends { let x = move p.t; q.t = T{v:3u8}; suspend wait(); p.t = move x; }";
    let package = lower(text);
    let f = body(&package, "f");
    let sites = parked_windows(&package, "f").unwrap();
    let [(CallSite::Operation(_), windows)] = &sites[..] else {
        panic!("one ordinary parked call")
    };
    let [window] = &windows[..] else { panic!("one open window: {windows:?}") };
    assert_eq!(
        (window.place.root, spell(&window.place.path), window.lender),
        (formal(f, 0), vec!["*".to_string(), "Cell::t".to_string()], Some(ReferenceAccess::Mutable))
    );
    let [published] = &f.suspension_sites[0].open_windows[..] else {
        panic!("one published window")
    };
    assert_eq!(published.place, window.place);
}

/// A `match` result returns either formal's stored loan. Each return edge
/// transfers its own formal's field and disposes only the other formal; the
/// reader that names one transferred field refuses two returns instead of
/// choosing one of them.
#[test]
fn a_match_of_two_field_transfers_names_each_origin_and_no_single_one() {
    let package =
        lower("data Leaf{v:u8;} data Node{next:@mut Leaf;} machine read(c:bool, x:Node, y:Node)->@mut Leaf{match c {true -> x.next, false -> y.next,}}");
    let f = body(&package, "read");
    let (x, y) = (formal(f, 1), formal(f, 2));
    let returns: Vec<_> = f
        .blocks
        .iter()
        .filter_map(|block| match &block.terminator {
            BlockTerminator::Return {
                result: omega_lowering::ReturnEdge::Structural { source, .. },
                cleanup,
                ..
            } => {
                let PlaceOrigin::Operation(id) = f.places[source.index()].origin else {
                    panic!("formed in the arm")
                };
                let OperationKind::MoveStructural { source } = &f.operations[id.index()].kind else {
                    panic!("a field transfer")
                };
                Some((source.root, spell(&source.path), cleanup.clone()))
            }
            _ => None,
        })
        .collect();
    let next = vec!["Node::next".to_string()];
    assert_eq!(
        returns,
        [
            (x, next.clone(), vec![omega_lowering::CleanupAction::DiscardRoot(y)]),
            (y, next, vec![omega_lowering::CleanupAction::DiscardRoot(x)]),
        ]
    );
    assert_eq!(returned_exclusive(&package, "read"), Err("expected one structural return"));
}

/// A write-only loan survives the suspension as a retained loan with its own
/// access, and the body still reads nothing through it.
#[test]
fn a_parked_write_only_loan_keeps_its_access() {
    let package = lower("data Cell{value:u8;} machine wait(v:@write Cell) suspends {} machine f(v:@write Cell) suspends { v.value = 1; suspend wait(v) }");
    let f = body(&package, "f");
    let v = formal(f, 0);
    let [site] = &f.suspension_sites[..] else { panic!("one parked call") };
    assert_eq!(
        site.retained_loans
            .iter()
            .map(|loan| (loan.descriptor, loan.access))
            .collect::<Vec<_>>(),
        [(v, ReferenceAccess::WriteOnly)]
    );
    assert!(site.open_windows.is_empty());
    let written = write_only_stores(&package, "f").unwrap();
    assert_eq!(
        written
            .iter()
            .map(|store| (store.formal, store.field.as_ref().map(|field| field.path.clone())))
            .collect::<Vec<_>>(),
        [(0, Some("Cell::value".to_string()))]
    );
}

const DEEPEST: &str = "data T{v:u8;} data Leaf{v:u8; t:T;}";

/// Lowering picks the operation for an admitted structural store from the
/// access of the deepest loan the store goes through
/// (structural_access.md#write-only-authority); L3 already refused every store
/// the source may not make. `x.next.t = T{v:1u8}` below a stored `@mut` extracts
/// and disposes the old `T` through that loan (the field-held test above).
/// Changing only the stored loan to `@write` refuses whatever the owner's own
/// access, because a write-only loan cannot take the displaced `T` and nothing
/// supplies cleanup authority for it. Its scalar twin `x.next.v = 1u8` lowers in
/// `a_write_through_a_stored_write_only_reference_keeps_its_crossings`.
#[test]
fn a_structural_store_takes_its_operation_from_the_deepest_loan() {
    for parent in ["Node", "@mut Node"] {
        let text = format!("{DEEPEST} data Node{{next:@write Leaf;}} machine f(x:{parent}){{ x.next.t = T{{v:1u8}}; }}");
        refusal(
            &text,
            "x.next.t = ",
            "x.next.t",
            "write-only structural replacement needs its cleanup authority",
        );
    }
}

/// A copy record is freely discardable, so the spec admits `x.c = C{v:1u8}`
/// through `@write` without cleanup authority. Lowering and the Psi-only
/// write-only reader both still refuse it; this pins that open gap.
#[test]
fn a_copy_record_through_a_write_only_loan_still_refuses() {
    let text = "data C [copy] {v:u8;} data Leaf{c:C;} machine f(x:@write Leaf){ x.c = C{v:1u8}; }";
    refusal(text, "x.c = ", "x.c", "write-only structural replacement needs its cleanup authority");
}

/// `@mut x.left` and `@write x.left` in a `Ch::A` arm lend a payload field with
/// the access L3 admitted, along the same `Case` path a shared borrow uses. An
/// owned `let mut` sum lends from its home without a crossing. Lowering used to
/// refuse the exclusive borrows only because its own permission walk could not
/// step through a `Case`. Through `x:@Ch` L3 refuses `@mut x.left`.
#[test]
fn a_payload_field_lends_with_the_access_l3_admitted() {
    const SUM: &str = "data Tk{v:u8;} data Ch{case A(left:Tk,right:Tk); case B(left:Tk,right:Tk);}";
    let borrowed = |text: &str| -> Vec<(Vec<String>, ReferenceAccess)> {
        let package = lower(text);
        let f = body(&package, "f");
        f.operations
            .iter()
            .filter_map(|op| match &op.kind {
                OperationKind::Call { arguments, .. } => Some(arguments),
                _ => None,
            })
            .flatten()
            .filter_map(|argument| match argument {
                CallArgument::Borrow { place, access } => Some((
                    place
                        .path
                        .iter()
                        .map(|step| match step {
                            PlaceProjection::Case(case) => format!("case {}", case.path),
                            _ => spell(std::slice::from_ref(step)).remove(0),
                        })
                        .collect(),
                    *access,
                )),
                _ => None,
            })
            .collect()
    };
    let payload = |case: &str, field: &str, crossing: bool| -> Vec<String> {
        let mut path = if crossing { vec!["*".to_string()] } else { vec![] };
        path.push(format!("case {case}"));
        path.push(format!("{case}::{field}"));
        path
    };
    for (borrow, access, parent) in [
        ("@", ReferenceAccess::Shared, "@Ch"),
        ("@mut ", ReferenceAccess::Mutable, "@mut Ch"),
        ("@write ", ReferenceAccess::WriteOnly, "@mut Ch"),
    ] {
        let text = format!(
            "{SUM} machine g(t:{borrow}Tk){{}} machine f(x:{parent}){{ match x {{Ch::A{{..}} -> g({borrow}x.left), Ch::B{{..}} -> g({borrow}x.right),}} }}"
        );
        assert_eq!(
            borrowed(&text),
            [
                (payload("Ch::A", "left", true), access),
                (payload("Ch::B", "right", true), access)
            ],
            "{borrow}"
        );
    }
    let owned = format!(
        "{SUM} machine g(t:@mut Tk){{}} machine f(){{ let mut x = Ch::A{{left:Tk{{v:1u8}}, right:Tk{{v:2u8}}}}; match x {{Ch::A{{..}} -> g(@mut x.left), Ch::B{{..}} -> g(@mut x.right),}} }}"
    );
    assert_eq!(
        borrowed(&owned),
        [
            (payload("Ch::A", "left", false), ReferenceAccess::Mutable),
            (payload("Ch::B", "right", false), ReferenceAccess::Mutable)
        ]
    );
    assert_eq!(
        typer_refusal(&format!(
            "{SUM} machine g(t:@mut Tk){{}} machine f(x:@Ch){{ match x {{Ch::A{{..}} -> g(@mut x.left), Ch::B{{..}} -> g(@mut x.right),}} }}"
        )),
        "place requires writable authority"
    );
}

#[test]
fn an_unrestored_payload_move_reaches_its_own_return() {
    let package = lower(
        "data Tk{v:u8;} data Ch{case A(left:Tk,right:Tk); case B(left:Tk,right:Tk);}
         machine f(x:@mut Ch)->Tk{match x {Ch::A{..} -> move x.left, Ch::B{..} -> move x.right,}}",
    );
    let f = body(&package, "f");
    let root = formal(f, 0);
    let mut returns = Vec::new();
    for block in &f.blocks {
        let BlockTerminator::Return {
            result: omega_lowering::ReturnEdge::Structural { source, .. },
            cleanup,
            ..
        } = &block.terminator
        else {
            continue;
        };
        let PlaceOrigin::Operation(id) = f.places[source.index()].origin else {
            panic!("the return must take its value from the selected field");
        };
        assert!(block.operations.contains(&id.index()));
        let OperationKind::BorrowedMove { source: moved } = &f.operations[id.index()].kind else {
            panic!("each return must preserve the open borrowed move for the checker");
        };
        let [
            PlaceProjection::Referent,
            PlaceProjection::Case(case),
            PlaceProjection::Field(field),
        ] = moved.path.as_slice()
        else {
            panic!("expected a field of the selected case through the caller's loan");
        };
        returns.push((moved.root, case.path.as_str(), field.path.as_str(), cleanup.is_empty()));
    }
    spec_corpus::observe_handoff!(
        psi "psi-output/borrowed-payload-returns",
        assert_eq!(
            (returns.as_slice(), stores(f).len()),
            ([(root, "Ch::A", "Ch::A::left", true), (root, "Ch::B", "Ch::B::right", true)].as_slice(), 0)
        ),
    );
}
