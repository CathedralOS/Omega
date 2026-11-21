//! Borrowed storage after the stores-and-windows and stored-exclusive repairs
//! met: stores below a field-held `@mutable`, write-only loans at joins and in
//! fields, second names for exclusive loans, and descriptor stores resolved by
//! their newest prefix. Each test lowers exact source bytes and checks the
//! published Psi; L6 still judges every lifetime, restoration and use.
#[path = "support/source.rs"]
mod source;

use omega_lowering::{
    BlockTerminator, CallArgument, CallSite, CallTarget, EdgeId, EmbeddedLoanOrigin, EmbeddedLoanSource, LoweredPackage, MachineBody, OperationKind,
    OperationResult, Parameter, PlaceId, PlaceOrigin, PlaceProjection, ProjectedPlace, ReferenceAccess, StructuralStoreDisposition, TypeShape,
    WritableLoanSite,
};
use psi_handoff_consumer::{
    Arrival, ArrivalSource, ExclusiveOrigin, ReturnedExclusive, StoreKind, parked_descriptor_arrivals, parked_windows, returned_exclusive, window_stores,
    write_only_stores,
};

fn lower(text: &str) -> LoweredPackage {
    source::lower(&[("case.omg", text)]).unwrap()
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

const FIELD_HELD: &str = "data T{v:u8;} data Leaf{t:T;} data Node{next:@mutable Leaf;}";

/// `x.next.t` is storage the caller lent through the `@mutable` that the owned
/// record `x` holds. Restoring it after a move fills the vacancy; it never
/// disposes the moved value. Before the integration repair, a path that began
/// at an owned field took the owned-store route and published `Dispose` of the
/// vacated place, while the same program through `x:@mutable Node` filled it.
#[test]
fn a_store_below_a_field_held_exclusive_loan_is_a_store_into_borrowed_storage() {
    for (parent, prefix) in [("Node", vec![]), ("@mutable Node", vec!["*"])] {
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
    for parent in ["Node", "@mutable Node"] {
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
/// arriving formal, as for `@mutable`, and never copies it.
#[test]
fn a_write_only_loan_chosen_by_match_transfers_each_arriving_formal() {
    for access in ["write", "mutable"] {
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
/// moved through a `@mutable` parent, and reborrows with its own access.
#[test]
fn a_stored_write_only_reference_transfers_or_reborrows_with_its_own_access() {
    let read = |body: &str| returned_exclusive(&lower(&format!("{WRITE_FIELD} {body}")), "read");
    let cases = [
        ("machine read(x:Node)->@write Leaf{x.next}", ExclusiveOrigin::Transfer, vec!["Node::next"]),
        (
            "machine read(x:@mutable Node)->@write Leaf{x.next}",
            ExclusiveOrigin::BorrowedMove,
            vec!["*", "Node::next"],
        ),
        (
            "machine read(x:@mutable Node)->@write Leaf{@write $(x.next)}",
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

/// Writing through a stored `@write` needs that loan's access, not a `mutable`
/// owner, and may follow a `@mutable` crossing
/// (terminal-psi/loans.md#reborrow-lineage-and-access). The store stays the
/// same scalar field store, and nothing reads the leaf.
#[test]
fn a_write_through_a_stored_write_only_reference_keeps_its_crossings() {
    for (parent, path) in [
        ("Node", vec!["Node::next", "*"]),
        ("@mutable Node", vec!["*", "Node::next", "*"]),
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
    let mut_field = "data Leaf{v:u8;} data Node{next:@mutable Leaf;}";
    assert_eq!(
        typer_refusal(&format!("{mut_field} machine f(x:@write Node){{x.next.v = 1u8;}}")),
        "cannot read through a write-only reference"
    );
    assert_eq!(
        typer_refusal("data Leaf{v:u8;} data Node{next:@Leaf;} machine f(x:@mutable Node){x.next.v = 1u8;}"),
        "place requires writable authority"
    );
    assert_eq!(
        typer_refusal(&format!("{WRITE_FIELD} machine look(v:@u8) {{}} machine f(x:@mutable Node){{look(@x.next.v)}}")),
        "cannot read through a write-only reference"
    );
}

/// `let b = a;` with an exclusive `a` transfers the loan into a new place,
/// for `@write` exactly as for `@mutable`. Naming `a` again transfers it again;
/// the checker judges that reuse. Each store goes through its own binding.
#[test]
fn a_second_name_for_an_exclusive_reference_transfers_into_its_own_place() {
    for access in ["write", "mutable"] {
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
            let [origin] = loan.origins.as_slice() else { panic!("one origin") };
            let EmbeddedLoanSource::Formal { formal, path } = &origin.source else {
                panic!("a formal origin")
            };
            let position = f
                .parameters
                .iter()
                .position(|p| *p == Parameter::Structural(*formal))
                .expect("an incoming formal");
            assert!(origin.arrival.is_empty() && path.is_empty() && origin.reborrows.is_empty());
            (spell(&loan.path), position)
        })
        .collect()
}

/// Inside one block, a home's descriptor comes from the newest store at a
/// prefix of its path: a whole or parent store replaces earlier field stores,
/// and `A::r` and `B::r` are different fields. A join, a store in an earlier
/// block and a store after a writable loan are in
/// `a_parked_home_names_every_origin_that_reaches_its_descriptor`.
#[test]
fn a_parked_home_names_the_newest_store_at_each_descriptor_prefix() {
    let origins = |text: String| embedded_origins(&lower(&text));
    let row = |path: &[&str], formal: usize| (path.iter().map(|s| s.to_string()).collect::<Vec<_>>(), formal);
    let forward = "machine forward(first:@u8,second:@u8) suspends";
    assert_eq!(
        origins(format!(
            "{HOLDER} {forward} {{let mutable h=Holder{{r:first}};h.r=second;h=Holder{{r:first}};suspend wait()}}"
        )),
        [row(&["Holder::r"], 0)]
    );
    assert_eq!(
        origins(format!("{HOLDER} {forward} {{let mutable h=Holder{{r:first}};h.r=second;suspend wait()}}")),
        [row(&["Holder::r"], 1)]
    );
    assert_eq!(
        origins(format!(
            "{NESTED} {forward} {{let mutable o=Outer{{i:Inner{{r:first}}}};o.i.r=second;o.i=Inner{{r:first}};suspend wait()}}"
        )),
        [row(&["Outer::i", "Inner::r"], 0)]
    );
    assert_eq!(
        origins(format!(
            "{NESTED} {forward} {{let mutable o=Outer{{i:Inner{{r:first}}}};o.i=Inner{{r:first}};o.i.r=second;suspend wait()}}"
        )),
        [row(&["Outer::i", "Inner::r"], 1)]
    );
    assert_eq!(
        origins(format!(
            "{TWO_RECORDS} {forward} {{let mutable h=H{{a:A{{r:first}},b:B{{r:first}}}};h.a.r=second;suspend wait()}}"
        )),
        [row(&["H::a", "A::r"], 1), row(&["H::b", "B::r"], 0)]
    );
    assert_eq!(
        origins(format!(
            "{TWO_RECORDS} {forward} {{let mutable h=H{{a:A{{r:first}},b:B{{r:first}}}};h.b.r=second;suspend wait()}}"
        )),
        [row(&["H::a", "A::r"], 0), row(&["H::b", "B::r"], 1)]
    );
}

/// The three programs of the former gap `parked home descriptor without one
/// reaching store`, byte for byte: `h.r` assigned a join of two formals, a
/// store before an unrelated branch, and a full field store after a call that
/// lent `h` with write access.
fn original_programs() -> [String; 3] {
    let forward = "machine forward(first:@u8,second:@u8) suspends";
    let branch = "machine forward(first:@u8,second:@u8,c:bool) suspends";
    [
        format!("{HOLDER} {branch} {{let mutable h=Holder{{r:first}};h.r=match c {{true -> second, false -> first,}};suspend wait()}}"),
        format!("{HOLDER} {branch} {{let mutable h=Holder{{r:first}};h.r=second;let n=match c {{true -> 1u8, false -> 2u8,}};suspend wait()}}"),
        format!(
            "data Holder{{r:@u8;}} machine poke(h:@mutable Holder) {{}} machine wait() suspends {{}} {forward} {{let mutable h=Holder{{r:first}};poke(@mutable h);h.r=second;suspend wait()}}"
        ),
    ]
}

/// The one embedded loan of the one parked owner of `forward`, which is its writable home.
fn parked_home_loan(package: &LoweredPackage) -> (PlaceId, &omega_lowering::EmbeddedLoan) {
    let f = body(package, "forward");
    let [site] = &f.suspension_sites[..] else { panic!("one parked call") };
    let [owner] = &site.retained_owners[..] else { panic!("one parked owner") };
    let [home] = f.mutable_homes[..] else { panic!("one writable home") };
    assert_eq!(owner.place, home);
    let [loan] = &owner.embedded_loans[..] else {
        panic!("one embedded descriptor")
    };
    assert_eq!(spell(&loan.path), ["Holder::r"]);
    (home, loan)
}

fn origins_mut(package: &mut LoweredPackage) -> &mut Vec<EmbeddedLoanOrigin> {
    &mut body_mut(package, "forward").suspension_sites[0].retained_owners[0].embedded_loans[0].origins
}

fn incoming(
    body: &MachineBody,
    target: omega_lowering::BlockId,
) -> Vec<(EdgeId, Vec<CallArgument>)> {
    let mut edges: Vec<_> = body
        .blocks
        .iter()
        .flat_map(|block| match &block.terminator {
            BlockTerminator::Jump(edge) => vec![edge],
            BlockTerminator::Conditional { when_true, when_false, .. } => vec![when_true, when_false],
            _ => vec![],
        })
        .filter(|edge| edge.target == target)
        .map(|edge| (edge.id, edge.structural_arguments.clone()))
        .collect();
    edges.sort_by_key(|(id, _)| id.get());
    edges
}

fn formal_source(formal: PlaceId) -> EmbeddedLoanSource {
    EmbeddedLoanSource::Formal { formal, path: vec![] }
}

fn single(formal: PlaceId) -> Vec<EmbeddedLoanOrigin> {
    vec![EmbeddedLoanOrigin {
        arrival: vec![],
        source: formal_source(formal),
        reborrows: vec![],
    }]
}

/// The position of the formal a replayed origin names.
fn replayed_formal(arrival: &Arrival) -> usize {
    match arrival.source {
        ArrivalSource::Formal { position, .. } => position,
        ArrivalSource::Written { .. } => panic!("a formal origin"),
    }
}

/// A descriptor whose value depends on the path taken keeps every origin
/// (lifetimes.md#carried-loan-transport: an unresolved selection retains all
/// possible sources), each with the join edges it arrives through. A store in
/// an earlier block that every path passes reaches the crossing unchanged.
/// After a call lent `h` with write access, the store `h.r = second` replaces
/// whatever the call's loan left. L6 judges lifetime and aliasing.
#[test]
fn a_parked_home_names_every_origin_that_reaches_its_descriptor() {
    let [joined, dominating, after_call] = original_programs();

    // `h.r = match c {...}`: the store in the join block installs the block
    // parameter, and each arriving edge copies its own formal into it.
    let package = lower(&joined);
    let f = body(&package, "forward");
    let (first, second) = (formal(f, 0), formal(f, 1));
    let join = f
        .blocks
        .iter()
        .find(|block| !block.structural_parameters.is_empty())
        .expect("the match result joins");
    let [selected] = join.structural_parameters[..] else {
        panic!("one joined descriptor")
    };
    let (home, loan) = parked_home_loan(&package);
    let stored: Vec<_> = join
        .operations
        .clone()
        .filter_map(|ordinal| match &f.operations[ordinal].kind {
            OperationKind::StructuralStore { destination, value, .. } => Some((destination.root, spell(&destination.path), *value)),
            _ => None,
        })
        .collect();
    assert_eq!(stored, [(home, vec!["Holder::r".to_string()], selected)]);
    let [(left, left_value), (right, right_value)] = &incoming(f, join.id)[..] else {
        panic!("two arrivals")
    };
    assert_eq!(
        (left_value.as_slice(), right_value.as_slice()),
        ([CallArgument::CopyWhole(second)].as_slice(), [CallArgument::CopyWhole(first)].as_slice())
    );
    let expected = vec![
        EmbeddedLoanOrigin {
            arrival: vec![*left],
            ..single(second).remove(0)
        },
        EmbeddedLoanOrigin {
            arrival: vec![*right],
            ..single(first).remove(0)
        },
    ];
    spec_corpus::observe_handoff!(psi "psi-output/parked-descriptor-join-arrivals", assert_eq!(loan.origins, expected));
    let replayed = parked_descriptor_arrivals(&package, "forward").unwrap();
    assert_eq!(
        replayed
            .iter()
            .map(|row| row
                .arrivals
                .iter()
                .map(|a| (a.edges.clone(), replayed_formal(a)))
                .collect::<Vec<_>>())
            .collect::<Vec<_>>(),
        [vec![(vec![*left], 1), (vec![*right], 0)]]
    );

    // Corrupted rows: the reader replays the edges and rejects each change.
    let check = |change: &dyn Fn(&mut Vec<EmbeddedLoanOrigin>)| {
        let mut package = lower(&joined);
        change(origins_mut(&mut package));
        parked_descriptor_arrivals(&package, "forward").map(|_| ())
    };
    assert_eq!(check(&|o| drop(o.pop())), Err("a possible arrival is missing"));
    assert_eq!(check(&|o| o.clear()), Err("a possible arrival is missing"));
    assert_eq!(
        check(&|o| {
            let source = o[0].source.clone();
            o[0].source = o[1].source.clone();
            o[1].source = source;
        }),
        Err("an origin is associated with another predecessor")
    );
    // The joined parameter is not an incoming formal.
    assert_eq!(check(&|o| o[0].source = formal_source(selected)), Err("an arrival names a different origin"));
    assert_eq!(check(&|o| o.reverse()), Err("arrivals are out of edge order"));
    // A corrupted product: swapping the edge arguments leaves the rows naming the other predecessor.
    let mut swapped = lower(&joined);
    let f = body_mut(&mut swapped, "forward");
    for block in &mut f.blocks {
        if let BlockTerminator::Jump(edge) = &mut block.terminator
            && let [CallArgument::CopyWhole(place)] = edge.structural_arguments.as_mut_slice()
        {
            *place = if *place == first { second } else { first };
        }
    }
    assert_eq!(
        parked_descriptor_arrivals(&swapped, "forward").map(|_| ()),
        Err("an origin is associated with another predecessor")
    );

    // `h.r = second;` before an unrelated `match`: both arrivals at the parked
    // block carry the same store, so the origin has no edge.
    let package = lower(&dominating);
    let f = body(&package, "forward");
    let second = formal(f, 1);
    let [site] = &f.suspension_sites[..] else { panic!("one parked call") };
    let CallSite::Operation(call) = site.call else {
        panic!("an ordinary parked call")
    };
    let parked = f
        .blocks
        .iter()
        .find(|block| block.operations.contains(&call.index()))
        .unwrap();
    assert_eq!(incoming(f, parked.id).len(), 2, "the parked call follows a join");
    let store = f
        .operations
        .iter()
        .position(|op| matches!(&op.kind, OperationKind::StructuralStore { value, .. } if *value == second))
        .expect("the store of second");
    assert!(f.blocks[0].operations.contains(&store), "the store is in the entry block");
    let (_, loan) = parked_home_loan(&package);
    spec_corpus::observe_handoff!(psi "psi-output/parked-descriptor-dominating-store", assert_eq!(loan.origins, single(second)));
    assert!(parked_descriptor_arrivals(&package, "forward").is_ok());
    let mut changed = lower(&dominating);
    let first = formal(body(&changed, "forward"), 0);
    origins_mut(&mut changed)[0].source = formal_source(first);
    assert_eq!(
        parked_descriptor_arrivals(&changed, "forward").map(|_| ()),
        Err("an arrival names a different origin")
    );

    // `poke(@mutable h); h.r = second;`: the store replaces whatever the call left.
    let package = lower(&after_call);
    let f = body(&package, "forward");
    let (home, loan) = parked_home_loan(&package);
    let order: Vec<_> = f
        .operations
        .iter()
        .map(|op| match &op.kind {
            OperationKind::EstablishRecord { .. } => "establish",
            OperationKind::Call { arguments, .. }
                if arguments
                    == &[CallArgument::Borrow {
                        place: ProjectedPlace { root: home, path: vec![] },
                        access: ReferenceAccess::Mutable,
                    }] =>
            {
                "lend h"
            }
            OperationKind::StructuralStore { destination, value, .. } if destination.root == home && *value == formal(f, 1) => "store second",
            OperationKind::Call { arguments, .. } if arguments.is_empty() => "park",
            _ => "other",
        })
        .collect();
    assert_eq!(order, ["establish", "lend h", "store second", "park"]);
    spec_corpus::observe_handoff!(psi "psi-output/parked-descriptor-store-after-writable-call", assert_eq!(loan.origins, single(formal(f, 1))));
    assert!(parked_descriptor_arrivals(&package, "forward").is_ok());
    // A corrupted product: without the store into h, the call's loan is a writer the rows omit.
    let mut redirected = lower(&after_call);
    let f = body_mut(&mut redirected, "forward");
    let first = formal(f, 0);
    for op in &mut f.operations {
        if let OperationKind::StructuralStore { destination, .. } = &mut op.kind {
            destination.root = first;
        }
    }
    assert_eq!(
        parked_descriptor_arrivals(&redirected, "forward").map(|_| ()),
        Err("a possible writer is missing")
    );
}

/// Each parked descriptor as `(owner path, [(join edges, formal, path in formal, reborrows)])`.
type Summary = Vec<(Vec<String>, Vec<(usize, usize, Vec<String>, usize)>)>;

fn published_summary(package: &LoweredPackage) -> Summary {
    let f = body(package, "forward");
    let position = |place: PlaceId| {
        f.parameters
            .iter()
            .position(|p| *p == Parameter::Structural(place))
            .unwrap()
    };
    f.suspension_sites
        .iter()
        .flat_map(|site| &site.retained_owners)
        .flat_map(|owner| &owner.embedded_loans)
        .map(|loan| {
            let origins = loan
                .origins
                .iter()
                .map(|o| {
                    let EmbeddedLoanSource::Formal { formal, path } = &o.source else {
                        panic!("a formal origin")
                    };
                    (o.arrival.len(), position(*formal), spell(path), o.reborrows.len())
                })
                .collect();
            (spell(&loan.path), origins)
        })
        .collect()
}

/// Positive controls around the original programs. Nested fields, sibling
/// fields, nested and agreeing joins, later replacements, a home read or moved
/// elsewhere, an owner chosen by `match`, a terminal crossing and two
/// crossings all name their origins. A field lent by itself leaves its
/// sibling known. A loan formed after the crossing, or before a store that
/// replaces the field, is no writer there. The Psi-only reader replays each one.
#[test]
fn parked_descriptor_arrivals_follow_fields_joins_and_reads() {
    let row = |path: &[&str], origins: &[(usize, usize, &[&str], usize)]| {
        (
            path.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
            origins
                .iter()
                .map(|&(edges, formal, inner, reborrows)| (edges, formal, inner.iter().map(|s| s.to_string()).collect(), reborrows))
                .collect::<Vec<_>>(),
        )
    };
    let r = &["Holder::r"][..];
    let f2 = "machine forward(first:@u8,second:@u8) suspends";
    let f3 = "machine forward(first:@u8,second:@u8,c:bool) suspends";
    let join = "match c {true -> second, false -> first,}";
    let wait = "machine wait() suspends {}";
    let both: &[(usize, usize, &[&str], usize)] = &[(1, 1, &[], 0), (1, 0, &[], 0)];
    let cases: Vec<(String, Summary)> = vec![
        // The writable borrow happens after the crossing, within its block or after a join.
        (
            format!("{HOLDER} machine keep(h:@mutable Holder) {{}} {f3} {{let mutable h=Holder{{r:first}};h.r=second;suspend wait();let k=@mutable h;keep(k)}}"),
            vec![row(r, &[(0, 1, &[], 0)])],
        ),
        (
            format!(
                "{HOLDER} machine keep(h:@mutable Holder) {{}} {f3} {{let mutable h=Holder{{r:first}};h.r=second;suspend wait();let n=match c {{true -> 1u8, false -> 2u8,}};let k=@mutable h;keep(k)}}"
            ),
            vec![row(r, &[(0, 1, &[], 0)])],
        ),
        (
            format!("data Inner{{r:@u8;}} data Outer{{i:Inner;}} {wait} {f3} {{let mutable o=Outer{{i:Inner{{r:first}}}};o.i.r={join};suspend wait()}}"),
            vec![row(&["Outer::i", "Inner::r"], both)],
        ),
        (
            format!(
                "{HOLDER} machine keep(h:@mutable Holder) {{}} machine relay(h:@mutable Holder)->@mutable Holder {{h}} {f3} {{let mutable h=Holder{{r:first}};h.r=second;transition c {{true -> suspend wait(), false -> keep(relay(@mutable h)),}}}}"
            ),
            vec![row(r, &[(0, 1, &[], 0)])],
        ),
        // The direct twin of a refused program below: both fields are replaced after the loan.
        (
            format!(
                "data H{{a:@u8;b:@u8;}} machine poke(h:@mutable H) {{}} {wait} {f2} {{let mutable h=H{{a:first,b:first}};poke(@mutable h);h.a=second;h.b=first;suspend wait()}}"
            ),
            vec![
                row(&["H::a"], &[(0, 1, &[], 0)]),
                row(&["H::b"], &[(0, 0, &[], 0)]),
            ],
        ),
        (
            format!(
                "data A{{r:@u8;}} data H{{a:A;b:A;}} machine pokeb(b:@mutable A) {{}} {wait} {f2} {{let mutable h=H{{a:A{{r:first}},b:A{{r:first}}}};pokeb(@mutable h.b);h.b.r=second;suspend wait()}}"
            ),
            vec![
                row(&["H::a", "A::r"], &[(0, 0, &[], 0)]),
                row(&["H::b", "A::r"], &[(0, 1, &[], 0)]),
            ],
        ),
        (
            format!(
                "{HOLDER} machine forward(first:@u8,second:@u8,c:bool,d:bool) suspends {{let mutable h=Holder{{r:first}};h.r=match c {{true -> match d {{true -> second, false -> first,}}, false -> first,}};suspend wait()}}"
            ),
            vec![row(r, &[(2, 1, &[], 0), (2, 0, &[], 0), (1, 0, &[], 0)])],
        ),
        (
            format!("{HOLDER} {f3} {{let mutable h=Holder{{r:first}};h.r=match c {{true -> second, false -> second,}};suspend wait()}}"),
            vec![row(r, &[(0, 1, &[], 0)])],
        ),
        (
            format!("{HOLDER} {f3} {{let mutable h=Holder{{r:first}};h.r={join};h.r=first;suspend wait()}}"),
            vec![row(r, &[(0, 0, &[], 0)])],
        ),
        (
            format!("{HOLDER} {f3} {{let mutable h=Holder{{r:first}};h.r={join};h=Holder{{r:second}};suspend wait()}}"),
            vec![row(r, &[(0, 1, &[], 0)])],
        ),
        (
            format!("{HOLDER} {f3} {{let mutable h=Holder{{r:first}};let n=match c {{true -> 1u8, false -> 2u8,}};suspend wait()}}"),
            vec![row(r, &[(0, 0, &[], 0)])],
        ),
        (
            format!(
                "data H{{a:@u8;b:@u8;}} {wait} machine forward(first:@u8,second:@u8,c:bool,d:bool) suspends {{let mutable h=H{{a:first,b:first}};h.a={join};h.b=match d {{true -> first, false -> second,}};suspend wait()}}"
            ),
            vec![
                row(&["H::a"], both),
                row(&["H::b"], &[(1, 0, &[], 0), (1, 1, &[], 0)]),
            ],
        ),
        // A shared borrow of the home kept across the park does not hide stores; two parks see the same arrivals.
        (
            format!(
                "data Holder{{r:@u8;}} machine look(h:@Holder) {{}} {wait} {f3} {{let mutable h=Holder{{r:first}};h.r={join};let b=@h;suspend wait();look(b)}}"
            ),
            vec![row(r, both)],
        ),
        (
            format!("{HOLDER} {f3} {{let mutable h=Holder{{r:first}};h.r={join};suspend wait();suspend wait()}}"),
            vec![row(r, both), row(r, both)],
        ),
        (
            format!("{HOLDER} {f3} {{let mutable h=Holder{{r:first}};h.r={join};transition{{_ -> suspend wait(),}}}}"),
            vec![row(r, both)],
        ),
        (
            format!("{HOLDER} {f3} {{let h=match c {{true -> Holder{{r:second}}, false -> Holder{{r:first}},}};suspend wait()}}"),
            vec![row(r, both)],
        ),
        (
            format!("{HOLDER} {f3} {{let mutable h=Holder{{r:first}};h=match c {{true -> Holder{{r:second}}, false -> Holder{{r:first}},}};suspend wait()}}"),
            vec![row(r, both)],
        ),
        // Reading `h.r` reborrows the descriptor it holds at the read, not later.
        (
            format!("{HOLDER} {f2} {{let mutable h=Holder{{r:first}};let x=h.r;h.r=second;let g=Holder{{r:x}};suspend wait()}}"),
            vec![row(r, &[(0, 1, &[], 0)]), row(r, &[(0, 0, &[], 1)])],
        ),
        (
            format!("{HOLDER} {f3} {{let mutable h=Holder{{r:first}};h.r={join};let mutable g=Holder{{r:first}};g.r=h.r;suspend wait()}}"),
            vec![row(r, both), row(r, &[(1, 1, &[], 1), (1, 0, &[], 1)])],
        ),
        (
            format!("data Holder{{r:@u8;}} data Box{{h:Holder;}} {wait} {f3} {{let mutable h=Holder{{r:first}};h.r={join};let b=Box{{h:move h}};suspend wait()}}"),
            vec![row(&["Box::h", "Holder::r"], both)],
        ),
        (
            format!(
                "data Cell{{r:@u8;}} {HOLDER} machine forward(x:Cell,y:Cell,c:bool) suspends {{let mutable h=Holder{{r:x.r}};h.r=match c {{true -> y.r, false -> x.r,}};suspend wait()}}"
            ),
            vec![
                row(&["Cell::r"], &[(0, 0, &["Cell::r"], 0)]),
                row(&["Cell::r"], &[(0, 1, &["Cell::r"], 0)]),
                row(r, &[(1, 1, &["Cell::r"], 1), (1, 0, &["Cell::r"], 1)]),
            ],
        ),
        // A lending call before the store is no writer at the crossing, whatever it returns or keeps.
        (
            format!(
                "data Holder{{r:@u8;}} machine poke(h:@mutable Holder, s:@u8) {{}} {wait} {f2} {{let mutable h=Holder{{r:first}};poke(@mutable h, second);h.r=second;suspend wait()}}"
            ),
            vec![row(r, &[(0, 1, &[], 0)])],
        ),
        (
            format!(
                "data Holder{{r:@u8;}} machine poke(h:@mutable Holder)->u8 {{1u8}} {wait} {f2} {{let mutable h=Holder{{r:first}};let n=poke(@mutable h);h.r=second;suspend wait()}}"
            ),
            vec![row(r, &[(0, 1, &[], 0)])],
        ),
    ];
    for (text, expected) in &cases {
        let package = lower(text);
        assert_eq!(&published_summary(&package), expected, "{text}");
        let rows: Summary = parked_descriptor_arrivals(&package, "forward")
            .unwrap_or_else(|error| panic!("{text}: {error}"))
            .iter()
            .map(|row| {
                (
                    spell(&row.path),
                    row.arrivals
                        .iter()
                        .map(|a| {
                            let ArrivalSource::Formal { position, path } = &a.source else {
                                panic!("{text}: a formal origin")
                            };
                            (a.edges.len(), *position, spell(path), a.reborrows.len())
                        })
                        .collect(),
                )
            })
            .collect();
        assert_eq!(&rows, expected, "{text}");
    }
}

/// The archived programs of the former gap `parked descriptor after an unreplaced
/// writable loan`, byte for byte (`a_writable_loan_leaves_a_parked_descriptor_unknown_until_a_store_replaces_it`).
const UNREPLACED_ORIGINALS: [&str; 4] = [
    "data H{a:@u8;b:@u8;} machine poke(h:@mutable H) {} machine wait() suspends {} machine forward(first:@u8,second:@u8) suspends {let mutable h=H{a:first,b:first};poke(@mutable h);h.a=second;suspend wait()}",
    "data A{r:@u8;} data H{a:A;b:A;} machine pokeb(b:@mutable A) {} machine wait() suspends {} machine forward(first:@u8,second:@u8) suspends {let mutable h=H{a:A{r:first},b:A{r:first}};pokeb(@mutable h.b);suspend wait()}",
    "data Holder{r:@u8;} machine poke(h:@mutable Holder) {} machine wait() suspends {} machine forward(first:@u8,second:@u8) suspends {let mutable h=Holder{r:first};h.r=second;poke(@mutable h);suspend wait()}",
    "data Holder{r:@u8;} machine wait(h:@mutable Holder) suspends {} machine forward(first:@u8,second:@u8) suspends {let mutable h=Holder{r:first};h.r=second;suspend wait(@mutable h)}",
];

/// The archived programs of the former gap `parked descriptor behind a writable
/// loan that can outlive its operation`, byte for byte
/// (`a_writable_loan_that_can_outlive_its_operation_hides_later_stores`).
const OUTLIVING_ORIGINALS: [&str; 5] = [
    "data Holder{r:@u8;} machine wait() suspends {} machine keep(h:@mutable Holder) {} machine forward(first:@u8,second:@u8) suspends {let mutable h=Holder{r:first};h.r=second;suspend wait();let k=@mutable h;keep(k);suspend wait()}",
    "data Holder{r:@u8;} machine wait() suspends {} machine keep(h:@mutable Holder) {} machine forward(first:@u8,second:@u8) suspends {let mutable h=Holder{r:first};let m=@mutable h;keep(m);h.r=second;suspend wait()}",
    "data Holder{r:@u8;} machine wait() suspends {} machine keep(h:@mutable Holder)->@mutable Holder {h} machine forward(first:@u8,second:@u8) suspends {let mutable h=Holder{r:first};let m=keep(@mutable h);h.r=second;suspend wait()}",
    "data Holder{r:@u8;} machine wait() suspends {} data Sink{v:u8;} machine poke(h:@mutable Holder, s:@mutable Sink) {} machine forward(first:@u8,second:@u8) suspends {let mutable h=Holder{r:first};let mutable s=Sink{v:1u8};poke(@mutable h, @mutable s);h.r=second;suspend wait()}",
    "data Holder{r:@u8;} machine wait() suspends {} data Cell{v:u8;} machine poke(h:@mutable Holder, c:@mutable Cell) {} machine forward(first:@u8,second:@u8,c:@mutable Cell) suspends {let mutable h=Holder{r:first};poke(@mutable h, c);h.r=second;suspend wait()}",
];

/// One origin as `(join edges, source, path, reborrows)`. A source is `f<n>`
/// for the incoming formal at position `n`, `borrow` for a named writable
/// loan and `<callee>#<argument>` for a call's writable operand, where a
/// boundary or selected member call is `member`.
type Origin = (usize, String, Vec<String>, usize);

fn o(
    edges: usize,
    source: &str,
    path: &[&str],
    reborrows: usize,
) -> Origin {
    (edges, source.to_string(), path.iter().map(|s| s.to_string()).collect(), reborrows)
}

fn d(
    path: &[&str],
    origins: Vec<Origin>,
) -> (Vec<String>, Vec<Origin>) {
    (path.iter().map(|s| s.to_string()).collect(), origins)
}

fn callee_name(
    package: &LoweredPackage,
    callee: &CallTarget,
) -> String {
    match callee {
        CallTarget::Machine(id) => package
            .machines
            .iter()
            .find(|machine| machine.id == *id)
            .expect("callee")
            .declaration
            .path
            .clone(),
        CallTarget::Member(_) => "member".into(),
        CallTarget::Runtime { .. } => "runtime".into(),
    }
}

/// Names a writable loan site after checking that it forms a writable loan:
/// a `BorrowPlace`, or a writable borrow operand at that call position.
fn site_name(
    package: &LoweredPackage,
    f: &MachineBody,
    loan: WritableLoanSite,
) -> String {
    let writable = |operand: Option<&CallArgument>| matches!(operand, Some(CallArgument::Borrow { access, .. }) if *access != ReferenceAccess::Shared);
    match loan {
        WritableLoanSite::Borrow(id) => {
            assert!(matches!(f.operations[id.index()].kind, OperationKind::BorrowPlace { access, .. } if access != ReferenceAccess::Shared));
            "borrow".into()
        }
        WritableLoanSite::Call {
            call: CallSite::Operation(id),
            argument,
        } => {
            let OperationKind::Call { callee, arguments, .. } = &f.operations[id.index()].kind else {
                panic!("a call")
            };
            assert!(writable(arguments.get(argument)));
            format!("{}#{argument}", callee_name(package, callee))
        }
        WritableLoanSite::Call {
            call: CallSite::Edge(edge),
            argument,
        } => {
            let (callee, arguments) = f
                .blocks
                .iter()
                .find_map(|block| match &block.terminator {
                    BlockTerminator::TerminalCall { id, callee, arguments, .. } if *id == edge => Some((callee, arguments)),
                    _ => None,
                })
                .expect("a terminal call");
            assert!(writable(arguments.get(argument)));
            format!("{}#{argument}", callee_name(package, callee))
        }
        WritableLoanSite::Call { call, .. } => panic!("no call at {call:?}"),
    }
}

/// Every embedded descriptor of every parked owner of `machine`, in site,
/// owner and declaration order, as the producer published it.
fn writer_summary(
    package: &LoweredPackage,
    machine: &str,
) -> Vec<(Vec<String>, Vec<Origin>)> {
    let f = body(package, machine);
    let position = |place: PlaceId| {
        f.parameters
            .iter()
            .position(|p| *p == Parameter::Structural(place))
            .expect("an incoming formal")
    };
    f.suspension_sites
        .iter()
        .flat_map(|site| &site.retained_owners)
        .flat_map(|owner| &owner.embedded_loans)
        .map(|loan| {
            let origins = loan
                .origins
                .iter()
                .map(|origin| {
                    let (source, path) = match &origin.source {
                        EmbeddedLoanSource::Formal { formal, path } => (format!("f{}", position(*formal)), path),
                        EmbeddedLoanSource::Written { loan, path } => (site_name(package, f, *loan), path),
                    };
                    (origin.arrival.len(), source, spell(path), origin.reborrows.len())
                })
                .collect();
            (spell(&loan.path), origins)
        })
        .collect()
}

/// The same descriptors as the Psi-only reader replays them.
fn replayed_summary(
    package: &LoweredPackage,
    machine: &str,
) -> Result<Vec<(Vec<String>, Vec<Origin>)>, &'static str> {
    let f = body(package, machine);
    Ok(parked_descriptor_arrivals(package, machine)?
        .iter()
        .map(|row| {
            let origins = row
                .arrivals
                .iter()
                .map(|arrival| {
                    let (source, path) = match &arrival.source {
                        ArrivalSource::Formal { position, path } => (format!("f{position}"), path),
                        ArrivalSource::Written { loan, path } => (site_name(package, f, *loan), path),
                    };
                    (arrival.edges.len(), source, spell(path), arrival.reborrows.len())
                })
                .collect();
            (spell(&row.path), origins)
        })
        .collect())
}

/// Each program lowers, publishes `expected` and the Psi-only reader replays the same rows.
fn writers_agree(
    machine: &str,
    cases: &[(String, Vec<(Vec<String>, Vec<Origin>)>)],
) {
    for (text, expected) in cases {
        let package = lower(text);
        assert_eq!(&writer_summary(&package, machine), expected, "{text}");
        assert_eq!(
            &replayed_summary(&package, machine).unwrap_or_else(|error| panic!("{text}: {error}")),
            expected,
            "{text}"
        );
    }
}

const POKE: &str = "machine poke(h:@mutable Holder) {}";

/// A call operand that lends the home, or a field enclosing the descriptor,
/// with write access is a possible writer. The descriptor at the crossing may
/// hold whatever a holder of that loan wrote, or still the earlier contents,
/// since the callee may write nothing. Lowering names the call and operand
/// and keeps walking; L6 judges the callee's effect and the loan's lifetime
/// (`wiki/spec/terminal-psi/loans.md#restoration-and-publication`). This
/// holds for a body that writes and one that does not, a store before the
/// call, a sibling the store left alone, a field lent by itself, the parked
/// call's own operand, a loan on one arm, two loans in a row and a later read.
/// A store after the call replaces every earlier writer
/// (`parked_descriptor_arrivals_follow_fields_joins_and_reads`).
#[test]
fn a_writable_call_operand_is_a_possible_writer_of_a_parked_descriptor() {
    let f2 = "machine forward(first:@u8,second:@u8) suspends";
    let f3 = "machine forward(first:@u8,second:@u8,c:bool) suspends";
    let r = &["Holder::r"][..];
    let cases = vec![
        (
            format!("{HOLDER} {POKE} {f2} {{let mutable h=Holder{{r:first}};poke(@mutable h);suspend wait()}}"),
            vec![d(r, vec![o(0, "poke#0", r, 0), o(0, "f0", &[], 0)])],
        ),
        // A local mutator that replaces the record through its loan.
        (
            format!(
                "{HOLDER} machine poke(h:@mutable Holder, s:@u8) {{$h=Holder{{r:s}};}} {f2} {{let mutable h=Holder{{r:first}};poke(@mutable h, second);suspend wait()}}"
            ),
            vec![d(r, vec![o(0, "poke#0", r, 0), o(0, "f0", &[], 0)])],
        ),
        // A local mutator that writes only a scalar sibling of the descriptor.
        (
            format!(
                "data Cell{{v:u8;}} data Holder{{r:@u8;c:Cell;}} machine wait() suspends {{}} machine poke(h:@mutable Holder) {{h.c.v=2u8;}} {f2} {{let mutable h=Holder{{r:first,c:Cell{{v:1u8}}}};poke(@mutable h);suspend wait()}}"
            ),
            vec![d(r, vec![o(0, "poke#0", r, 0), o(0, "f0", &[], 0)])],
        ),
        // The parked callee may write through its own operand while parked.
        (
            format!(
                "data Holder{{r:@u8;}} machine wait(h:@mutable Holder) suspends {{}} {f2} {{let mutable h=Holder{{r:first}};transition{{_ -> suspend wait(@mutable h),}}}}"
            ),
            vec![d(r, vec![o(0, "wait#0", r, 0), o(0, "f0", &[], 0)])],
        ),
        (
            format!(
                "{HOLDER} {POKE} machine skip() {{}} {f3} {{let mutable h=Holder{{r:first}};match c {{true -> poke(@mutable h), false -> skip(),}};suspend wait()}}"
            ),
            vec![d(
                r,
                vec![o(1, "poke#0", r, 0), o(1, "f0", &[], 0), o(1, "f0", &[], 0)],
            )],
        ),
        (
            format!("{HOLDER} {POKE} {f2} {{let mutable h=Holder{{r:first}};poke(@mutable h);poke(@mutable h);suspend wait()}}"),
            vec![d(
                r,
                vec![o(0, "poke#0", r, 0), o(0, "poke#0", r, 0), o(0, "f0", &[], 0)],
            )],
        ),
        // Reading `h.r` after the call reborrows whatever the loan left there.
        (
            format!("{HOLDER} {POKE} {f2} {{let mutable h=Holder{{r:first}};poke(@mutable h);let x=h.r;let g=Holder{{r:x}};suspend wait()}}"),
            vec![
                d(r, vec![o(0, "poke#0", r, 0), o(0, "f0", &[], 0)]),
                d(r, vec![o(0, "poke#0", r, 1), o(0, "f0", &[], 1)]),
            ],
        ),
    ];
    writers_agree("forward", &cases);

    // The archived programs of the former gap, which each refused at the crossing.
    let expected = vec![
        vec![
            d(&["H::a"], vec![o(0, "f1", &[], 0)]),
            d(&["H::b"], vec![o(0, "poke#0", &["H::b"], 0), o(0, "f0", &[], 0)]),
        ],
        vec![
            d(&["H::a", "A::r"], vec![o(0, "f0", &[], 0)]),
            d(&["H::b", "A::r"], vec![o(0, "pokeb#0", &["A::r"], 0), o(0, "f0", &[], 0)]),
        ],
        vec![d(r, vec![o(0, "poke#0", r, 0), o(0, "f1", &[], 0)])],
        vec![d(r, vec![o(0, "wait#0", r, 0), o(0, "f1", &[], 0)])],
    ];
    let published: Vec<_> = UNREPLACED_ORIGINALS
        .iter()
        .map(|text| {
            let package = lower(text);
            (writer_summary(&package, "forward"), replayed_summary(&package, "forward"))
        })
        .collect();
    let expected: Vec<_> = expected
        .into_iter()
        .map(|rows| (rows.clone(), Ok(rows)))
        .collect();
    spec_corpus::observe_handoff!(psi "psi-output/parked-descriptor-unreplaced-writable-loan-originals", assert_eq!(published, expected));

    // The exact Psi of the first program: the call, its operand and the path
    // below the loan, then the construction's formal.
    let package = lower(&cases[0].0);
    let f = body(&package, "forward");
    let (home, loan) = parked_home_loan(&package);
    let poke = f
        .operations
        .iter()
        .find(|op| {
            matches!(&op.kind, OperationKind::Call { arguments, .. } if arguments
                == &[CallArgument::Borrow { place: ProjectedPlace { root: home, path: vec![] }, access: ReferenceAccess::Mutable }])
        })
        .expect("the lending call");
    let expected = vec![
        EmbeddedLoanOrigin {
            arrival: vec![],
            source: EmbeddedLoanSource::Written {
                loan: WritableLoanSite::Call {
                    call: CallSite::Operation(poke.id),
                    argument: 0,
                },
                path: loan.path.clone(),
            },
            reborrows: vec![],
        },
        single(formal(f, 0)).remove(0),
    ];
    spec_corpus::observe_handoff!(psi "psi-output/parked-descriptor-writable-call-operand", assert_eq!(loan.origins, expected));

    // Two loans in a row: the newest is listed first.
    let package = lower(&cases[5].0);
    let (_, loan) = parked_home_loan(&package);
    let calls: Vec<_> = loan
        .origins
        .iter()
        .filter_map(|origin| match origin.source {
            EmbeddedLoanSource::Written {
                loan: WritableLoanSite::Call {
                    call: CallSite::Operation(id), ..
                },
                ..
            } => Some(id.index()),
            _ => None,
        })
        .collect();
    assert!(matches!(calls[..], [newer, older] if newer > older), "{calls:?}");
}

/// The record does not depend on what the callee is or returns: an empty
/// unit machine, a scalar result, a result that can keep the loan, a second
/// writable operand, a boundary member and a selected local member each
/// publish the same writer, the call and its operand. Lowering reads no
/// callee body, result shape or provider, so it decides neither the callee's
/// effect nor when the loan ends; L6 judges both from the callee's contract.
#[test]
fn every_kind_of_callee_publishes_the_same_writer() {
    let f2 = "machine forward(first:@u8,second:@u8) suspends";
    let r = &["Holder::r"][..];
    let same = |callee: &str| {
        vec![d(
            r,
            vec![o(0, &format!("{callee}#0"), r, 0), o(0, "f0", &[], 0)],
        )]
    };
    let cases = vec![
        (
            format!("{HOLDER} {POKE} {f2} {{let mutable h=Holder{{r:first}};poke(@mutable h);suspend wait()}}"),
            same("poke"),
        ),
        (
            format!("{HOLDER} machine poke(h:@mutable Holder)->u8 {{1u8}} {f2} {{let mutable h=Holder{{r:first}};let n=poke(@mutable h);suspend wait()}}"),
            same("poke"),
        ),
        (
            format!("{HOLDER} machine keep(h:@mutable Holder)->@mutable Holder {{h}} {f2} {{let mutable h=Holder{{r:first}};let m=keep(@mutable h);suspend wait()}}"),
            same("keep"),
        ),
        (
            format!(
                "{HOLDER} data Sink{{v:u8;}} machine poke(h:@mutable Holder, s:@mutable Sink) {{}} {f2} {{let mutable h=Holder{{r:first}};let mutable s=Sink{{v:1u8}};poke(@mutable h, @mutable s);suspend wait()}}"
            ),
            same("poke"),
        ),
        (
            format!(
                "{HOLDER} boundary trait Sink {{ machine poke(h:@mutable Holder) reaches Sink crashes none; }} machine forward(first:@u8,second:@u8) suspends reaches Sink {{let mutable h=Holder{{r:first}};Sink::poke(@mutable h);suspend wait()}}"
            ),
            same("member"),
        ),
    ];
    writers_agree("forward", &cases);
    let local = format!(
        "{HOLDER} trait Poke {{ machine poke(h:@mutable Holder); }} machine p(h:@mutable Holder) {{}} Local satisfies Poke {{ Poke::poke=p; }} machine use_it<const E:Poke>(first:@u8) suspends {{let mutable h=Holder{{r:first}};E::poke(@mutable h);suspend wait()}} {f2} {{suspend use_it::<Local>(first)}}"
    );
    writers_agree("use_it", &[(local, same("member"))]);
}

/// A named writable loan of the home is a possible writer at every later
/// crossing it reaches, whether its holder passes it on, keeps it across the
/// crossing, replaces the record through it or was chosen on one arm. A store
/// through the loan is not a store into the home: the borrow stays the
/// origin and L6 reads the store through it. A store into the home after any loan
/// replaces it: an owner can store only while no writable loan of the place
/// is live, which L6 checks at the store, so lowering does not ask when the
/// loan ended. The same holds for a call whose result or second writable
/// operand could keep the loan.
#[test]
fn a_named_writable_loan_is_a_possible_writer_until_a_store_replaces_it() {
    let f2 = "machine forward(first:@u8,second:@u8) suspends";
    let f3 = "machine forward(first:@u8,second:@u8,c:bool) suspends";
    let keep = "machine keep(h:@mutable Holder) {}";
    let r = &["Holder::r"][..];
    let cases = vec![
        (
            format!("{HOLDER} {keep} {f2} {{let mutable h=Holder{{r:first}};let m=@mutable h;keep(m);suspend wait()}}"),
            vec![d(r, vec![o(0, "borrow", r, 0), o(0, "f0", &[], 0)])],
        ),
        (
            format!("{HOLDER} {keep} {f2} {{let mutable h=Holder{{r:first}};let m=@mutable h;suspend wait();keep(m)}}"),
            vec![d(r, vec![o(0, "borrow", r, 0), o(0, "f0", &[], 0)])],
        ),
        (
            format!("{HOLDER} {f2} {{let mutable h=Holder{{r:first}};let m=@mutable h;$m=Holder{{r:second}};suspend wait()}}"),
            vec![d(r, vec![o(0, "borrow", r, 0), o(0, "f0", &[], 0)])],
        ),
        (
            format!(
                "{HOLDER} {keep} {f3} {{let mutable h=Holder{{r:first}};let mutable g=Holder{{r:second}};let m=match c {{true -> @mutable h, false -> @mutable g,}};keep(m);suspend wait()}}"
            ),
            vec![
                d(r, vec![o(1, "borrow", r, 0), o(1, "f0", &[], 0), o(1, "f0", &[], 0)]),
                d(r, vec![o(1, "f1", &[], 0), o(1, "borrow", r, 0), o(1, "f1", &[], 0)]),
            ],
        ),
    ];
    writers_agree("forward", &cases);

    // The archived programs of the former gap: a loan named after the first
    // crossing, and a store after a named loan, a call that returns the loan
    // and a call with a second writable operand, lent or moved in.
    let stored = || vec![d(r, vec![o(0, "f1", &[], 0)])];
    let expected = vec![
        vec![
            d(r, vec![o(0, "f1", &[], 0)]),
            d(r, vec![o(0, "borrow", r, 0), o(0, "f1", &[], 0)]),
        ],
        stored(),
        stored(),
        stored(),
        stored(),
    ];
    let published: Vec<_> = OUTLIVING_ORIGINALS
        .iter()
        .map(|text| {
            let package = lower(text);
            (writer_summary(&package, "forward"), replayed_summary(&package, "forward"))
        })
        .collect();
    let expected: Vec<_> = expected
        .into_iter()
        .map(|rows| (rows.clone(), Ok(rows)))
        .collect();
    spec_corpus::observe_handoff!(psi "psi-output/parked-descriptor-outliving-writable-loan-originals", assert_eq!(published, expected));

    // The exact Psi of the first program, and the loan kept across the
    // crossing in the second stays a retained loan of the activation.
    let package = lower(&cases[0].0);
    let f = body(&package, "forward");
    let (home, loan) = parked_home_loan(&package);
    let borrow = f
        .operations
        .iter()
        .find(|op| {
            matches!(&op.kind, OperationKind::BorrowPlace { source, access: ReferenceAccess::Mutable } if *source == ProjectedPlace { root: home, path: vec![] })
        })
        .expect("the named loan");
    let expected = vec![
        EmbeddedLoanOrigin {
            arrival: vec![],
            source: EmbeddedLoanSource::Written {
                loan: WritableLoanSite::Borrow(borrow.id),
                path: loan.path.clone(),
            },
            reborrows: vec![],
        },
        single(formal(f, 0)).remove(0),
    ];
    spec_corpus::observe_handoff!(psi "psi-output/parked-descriptor-named-writable-loan", assert_eq!(loan.origins, expected));
    let package = lower(&cases[1].0);
    let f = body(&package, "forward");
    let [site] = &f.suspension_sites[..] else { panic!("one parked call") };
    let OperationResult::Structural(named) = f
        .operations
        .iter()
        .find(|op| matches!(op.kind, OperationKind::BorrowPlace { .. }))
        .expect("the named loan")
        .result
    else {
        panic!("a loan place")
    };
    assert!(
        site.retained_loans
            .iter()
            .any(|kept| kept.descriptor == named && kept.needed_after_completion)
    );
    let package = lower(OUTLIVING_ORIGINALS[1]);
    let f = body(&package, "forward");
    let (_, loan) = parked_home_loan(&package);
    spec_corpus::observe_handoff!(psi "psi-output/parked-descriptor-store-after-named-loan", assert_eq!(loan.origins, single(formal(f, 1))));
}

/// The Psi-only reader replays writers from the operations alone. It rejects
/// each corrupted row, and each corrupted product whose rows were left alone:
/// a lending operand made shared, a named loan made shared, and the lent
/// place moved to another storage.
#[test]
fn the_arrival_reader_rejects_a_changed_writer() {
    let f2 = "machine forward(first:@u8,second:@u8) suspends";
    let call = format!("{HOLDER} {POKE} {f2} {{let mutable h=Holder{{r:first}};poke(@mutable h);suspend wait()}}");
    let twice = format!("{HOLDER} {POKE} {f2} {{let mutable h=Holder{{r:first}};poke(@mutable h);poke(@mutable h);suspend wait()}}");
    let named = format!("{HOLDER} machine keep(h:@mutable Holder) {{}} {f2} {{let mutable h=Holder{{r:first}};let m=@mutable h;keep(m);suspend wait()}}");
    let rows = |text: &str, change: &dyn Fn(&mut Vec<EmbeddedLoanOrigin>)| {
        let mut package = lower(text);
        change(origins_mut(&mut package));
        parked_descriptor_arrivals(&package, "forward").map(|_| ())
    };
    let product = |text: &str, change: &dyn Fn(&mut MachineBody)| {
        let mut package = lower(text);
        change(body_mut(&mut package, "forward"));
        parked_descriptor_arrivals(&package, "forward").map(|_| ())
    };
    let site = |o: &mut EmbeddedLoanOrigin, change: &dyn Fn(&mut WritableLoanSite, &mut Vec<PlaceProjection>)| {
        let EmbeddedLoanSource::Written { loan, path } = &mut o.source else {
            panic!("a writer")
        };
        change(loan, path);
    };
    assert!(parked_descriptor_arrivals(&lower(&call), "forward").is_ok());
    assert_eq!(rows(&call, &|o| drop(o.remove(0))), Err("a possible writer is missing"));
    assert_eq!(rows(&call, &|o| drop(o.pop())), Err("an arrival names a different origin"));
    assert_eq!(rows(&call, &|o| o.swap(0, 1)), Err("an arrival lists its writers out of walk order"));
    assert_eq!(
        rows(&call, &|o| o.insert(0, o[0].clone())),
        Err("a published writer forms no writable loan of this storage")
    );
    assert_eq!(
        rows(&call, &|o| site(&mut o[0], &|_, path| path.clear())),
        Err("a writer names a different loan")
    );
    assert_eq!(
        rows(&call, &|o| site(&mut o[0], &|loan, _| {
            let WritableLoanSite::Call { argument, .. } = loan else { unreachable!() };
            *argument = 1;
        })),
        Err("a writer names a different loan")
    );
    assert_eq!(
        rows(&call, &|o| site(&mut o[0], &|loan, _| {
            let WritableLoanSite::Call {
                call: CallSite::Operation(id), ..
            } = *loan
            else {
                unreachable!()
            };
            *loan = WritableLoanSite::Borrow(id);
        })),
        Err("a writer names a different loan")
    );
    assert!(parked_descriptor_arrivals(&lower(&twice), "forward").is_ok());
    assert_eq!(rows(&twice, &|o| o.swap(0, 1)), Err("an arrival lists its writers out of walk order"));
    assert!(parked_descriptor_arrivals(&lower(&named), "forward").is_ok());
    assert_eq!(rows(&named, &|o| drop(o.remove(0))), Err("a possible writer is missing"));

    let shared_operand = |f: &mut MachineBody| {
        for op in &mut f.operations {
            if let OperationKind::Call { arguments, .. } = &mut op.kind {
                for argument in arguments {
                    if let CallArgument::Borrow { access, .. } = argument {
                        *access = ReferenceAccess::Shared;
                    }
                }
            }
        }
    };
    assert_eq!(
        product(&call, &shared_operand),
        Err("a published writer forms no writable loan of this storage")
    );
    let other_storage = |f: &mut MachineBody| {
        let first = formal(f, 0);
        for op in &mut f.operations {
            if let OperationKind::Call { arguments, .. } = &mut op.kind
                && let [CallArgument::Borrow { place, .. }] = arguments.as_mut_slice()
            {
                place.root = first;
            }
        }
    };
    assert_eq!(product(&call, &other_storage), Err("a published writer forms no writable loan of this storage"));
    let shared_borrow = |f: &mut MachineBody| {
        for op in &mut f.operations {
            if let OperationKind::BorrowPlace { access, .. } = &mut op.kind {
                *access = ReferenceAccess::Shared;
            }
        }
    };
    assert_eq!(
        product(&named, &shared_borrow),
        Err("a published writer forms no writable loan of this storage")
    );
}

/// Lowering emits no cycle inside a body. Given one by hand, the Psi-only
/// reader stops with a refusal rather than walk around it.
#[test]
fn the_arrival_reader_refuses_a_cycle() {
    let [_, dominating, _] = original_programs();
    let mut package = lower(&dominating);
    let f = body_mut(&mut package, "forward");
    let Parameter::Scalar(condition) = f.parameters[2] else {
        panic!("the Boolean formal")
    };
    let next = EdgeId(std::num::NonZeroUsize::new(f.diagnostics.edges.len() + 1).unwrap());
    let arm = f
        .blocks
        .iter()
        .position(|block| block.id.get() != 1 && matches!(block.terminator, BlockTerminator::Jump(_)))
        .unwrap();
    let id = f.blocks[arm].id;
    let placeholder = BlockTerminator::Return {
        id: next,
        result: omega_lowering::ReturnEdge::Unit,
        cleanup: vec![],
    };
    let BlockTerminator::Jump(exit) = std::mem::replace(&mut f.blocks[arm].terminator, placeholder) else {
        unreachable!()
    };
    f.blocks[arm].terminator = BlockTerminator::Conditional {
        condition,
        when_true: omega_lowering::SuccessorEdge {
            selected_case: None,
            requires_adjacency: false,
            id: next,
            target: id,
            arguments: vec![],
            scalar_qualification_changes: vec![],
            domain_completions: vec![],
            structural_arguments: vec![],
            cleanup: vec![],
        },
        when_false: exit,
    };
    assert_eq!(parked_descriptor_arrivals(&package, "forward").map(|_| ()), Err("this reader refuses a cycle"));
}

/// A window names only its own lender. A store through another `@mutable` lender
/// is neither a restoration nor an overlap.
#[test]
fn a_parked_window_names_only_its_own_lender() {
    let text = "data T{v:u8;} data Cell{t:T; u:T;} machine wait() suspends {} machine f(p:@mutable Cell, q:@mutable Cell) suspends { let x = move p.t; q.t = T{v:3u8}; suspend wait(); p.t = move x; }";
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
        lower("data Leaf{v:u8;} data Node{next:@mutable Leaf;} machine read(c:bool, x:Node, y:Node)->@mutable Leaf{match c {true -> x.next, false -> y.next,}}");
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

/// The deepest loan authorizes the write. An intact affine value transfers
/// once and the write-only path never extracts the displaced storage.
#[test]
fn a_structural_store_takes_its_operation_from_the_deepest_loan() {
    for parent in ["Node", "@mutable Node"] {
        let text = format!("{DEEPEST} data Node{{next:@write Leaf;}} machine f(x:{parent}){{ x.next.t = T{{v:1u8}}; }}");
        let mut pkg = lower(&text);
        let f = body(&pkg, "f");
        assert!(borrowed_moves(f).is_empty());
        let [(destination, disposition)] = stores(f)[..] else { panic!("one replacement") };
        assert_eq!(destination.root, formal(f, 0));
        assert_eq!(*disposition, StructuralStoreDisposition::ReplaceWriteOnly(omega_lowering::WriteOnlyCleanup::NoCodeAffine));
        let observed = write_only_stores(&pkg, "f").unwrap();
        let [written] = observed.as_slice() else { panic!("one write") };
        spec_corpus::observe_handoff!(psi "psi-output/write-only-affine-replacement", assert_eq!(
            (written.crossing, written.field.as_ref().map(|field| field.path.as_str()), written.formal),
            (if parent == "Node" { 1 } else { 2 }, Some("Leaf::t"), 0),
        ));
        let schema = pkg.data.iter_mut().find(|row| row.declaration.path == "T").unwrap();
        schema.write_only_cleanup = None;
        assert!(write_only_stores(&pkg, "f").is_err());
    }
}

#[test]
fn replacing_a_copy_record_through_a_write_only_loan_preserves_its_value_and_destination() {
    let text = "data C [copy] {v:u8;} data Leaf{c:C;} machine f(x:@write Leaf){ x.c = C{v:1u8}; }";
    let package = lower(text);
    let f = body(&package, "f");
    let [literal, construct, store] = f.operations.as_slice() else {
        panic!("one literal, one record construction and one write")
    };
    let OperationKind::StructuralStore {
        destination,
        value,
        disposition,
    } = &store.kind
    else {
        panic!("a structural replacement")
    };
    let OperationKind::EstablishRecord { fields } = &construct.kind else {
        panic!("the right-hand side establishes C")
    };
    let [field] = fields.as_slice() else { panic!("C has one initializer") };
    let OperationResult::Scalar(one) = literal.result else {
        panic!("the initializer is scalar")
    };
    let TypeShape::Nominal(data) = package.types.get(f.places[value.index()].ty).unwrap() else {
        panic!("the incoming value is a record")
    };
    let observed = write_only_stores(&package, "f").expect("read the store through the write-only loan");
    let [written] = observed.as_slice() else {
        panic!("exactly one write through the formal")
    };
    assert_eq!(written.formal, 0);
    assert_eq!(written.crossing, 0);
    assert_eq!(written.path, [PlaceProjection::Referent]);
    assert_eq!(written.field.as_ref().unwrap().path, "Leaf::c");
    assert_eq!(written.value, Parameter::Structural(*value));
    spec_corpus::observe_handoff!(
        psi "psi-output/write-only-copy-record",
        assert_eq!(
            (destination.root, spell(&destination.path), disposition, construct.result, package.data[data.get()-1].declaration.path.as_str(),
             field.field.path.as_str(), &field.operand, &literal.kind),
            (formal(f, 0), vec!["*".to_string(), "Leaf::c".to_string()], &StructuralStoreDisposition::ReplaceUnrestricted,
             OperationResult::Structural(*value), "C", "C::v", &CallArgument::Scalar(one), &OperationKind::Constant(omega_lowering::ScalarLiteral::Integer(1.into())))
        ),
    );
}

#[test]
fn an_intact_structural_write_keeps_every_crossing_to_the_destination() {
    const TYPES: &str = "data C [copy]{v:u8;} data Leaf{c:C;} data Node{next:@write Leaf;}";
    for (parent, crossing) in [("Node", false), ("@mutable Node", true)] {
        let text = format!("{TYPES} machine f(x:{parent},n:C){{x.next.c=n;}}");
        let package = lower(&text);
        let f = body(&package, "f");
        let [(place, disposition)] = stores(f)[..] else { panic!("one intact store") };
        let mut path = Vec::new();
        if crossing {
            path.push("*");
        }
        path.extend(["Node::next", "*", "Leaf::c"]);
        assert_eq!(place.root, formal(f, 0));
        assert_eq!(spell(&place.path), path);
        assert_eq!(*disposition, StructuralStoreDisposition::ReplaceUnrestricted);
        assert!(borrowed_moves(f).is_empty());
    }
}

#[test]
fn an_intact_copy_replacement_walks_the_stored_schema() {
    for declaration in [
        "data I [copy]{v:u8;} data C [copy]{a:I;b:I;}",
        "data C [copy]{v:[u8;4];}",
        "data C [copy]{case A(v:u8);case B(v:u16);}",
        "data C [copy]{}",
    ] {
        let text = format!("{declaration} machine f(x:@write C,n:C){{ $x=n; }}");
        let package = lower(&text);
        let f = body(&package, "f");
        let [(place, disposition)] = stores(f)[..] else { panic!("one intact store") };
        assert_eq!(place.root, formal(f, 0));
        assert_eq!(place.path, [PlaceProjection::Referent]);
        assert_eq!(*disposition, StructuralStoreDisposition::ReplaceUnrestricted);
        assert!(borrowed_moves(f).is_empty());
    }
}

#[test]
fn replacing_a_copy_value_can_still_require_descriptor_cleanup() {
    for text in [
        "data C [copy]{r:@u8;} data Leaf{c:C;} machine f(x:@write Leaf,n:C){ x.c=n; }",
        "data C [copy]{case Plain(v:u8);case Held(r:@u8);} data Leaf{c:C;} machine f(x:@write Leaf){ x.c=C::Plain{v:1u8}; }",
    ] {
        let mut pkg = lower(text);
        let f = body(&pkg, "f");
        assert!(borrowed_moves(f).is_empty());
        let [(_, disposition)] = stores(f)[..] else { panic!("one intact replacement") };
        assert_eq!(*disposition, StructuralStoreDisposition::ReplaceWriteOnly(omega_lowering::WriteOnlyCleanup::DescriptorCleanup));
        let observed = write_only_stores(&pkg, "f").unwrap();
        let [written] = observed.as_slice() else { panic!("one whole store") };
        let Parameter::Structural(value) = written.value else { panic!("expected structural source") };
        let incoming = if f.parameters.len() == 2 {
            // A [copy] incoming value keeps its source descriptor at n.
            matches!(f.places[value.index()].origin, PlaceOrigin::Operation(id)
                if matches!(&f.operations[id.index()].kind, OperationKind::CopyStructural { source } if source.root == formal(f, 1) && source.path.is_empty()))
                || value == formal(f, 1)
        } else {
            // The incoming Plain payload carries no loan. The resident value may
            // still be Held, so it owes exactly the resident roster at x.c.
            matches!(f.places[value.index()].origin, PlaceOrigin::Operation(id)
                if matches!(&f.operations[id.index()].kind, OperationKind::EstablishCase { case, .. } if case.path == "C::Plain"))
        };
        spec_corpus::observe_handoff!(psi "psi-output/write-only-descriptor-replacement", assert_eq!(
            (written.formal, written.crossing, written.field.as_ref().map(|field| field.path.as_str()), incoming),
            (0, 0, Some("Leaf::c"), true),
        ));
        let schema = pkg.data.iter_mut().find(|row| row.declaration.path == "C").unwrap();
        schema.write_only_cleanup = Some(omega_lowering::WriteOnlyCleanup::NoCodeAffine);
        assert!(write_only_stores(&pkg, "f").is_err());
    }
}

/// `@mutable x.left` and `@write x.left` in a `Ch::A` arm lend a payload field with
/// the access L3 admitted, along the same `Case` path a shared borrow uses. An
/// owned `let mutable` sum lends from its home without a crossing. Lowering used to
/// refuse the exclusive borrows only because its own permission walk could not
/// step through a `Case`. Through `x:@Ch` L3 refuses `@mutable x.left`.
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
        ("@mutable ", ReferenceAccess::Mutable, "@mutable Ch"),
        ("@write ", ReferenceAccess::WriteOnly, "@mutable Ch"),
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
        "{SUM} machine g(t:@mutable Tk){{}} machine f(){{ let mutable x = Ch::A{{left:Tk{{v:1u8}}, right:Tk{{v:2u8}}}}; match x {{Ch::A{{..}} -> g(@mutable x.left), Ch::B{{..}} -> g(@mutable x.right),}} }}"
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
            "{SUM} machine g(t:@mutable Tk){{}} machine f(x:@Ch){{ match x {{Ch::A{{..}} -> g(@mutable x.left), Ch::B{{..}} -> g(@mutable x.right),}} }}"
        )),
        "place requires writable authority"
    );
}

#[test]
fn an_unrestored_payload_move_reaches_its_own_return() {
    let package = lower(
        "data Tk{v:u8;} data Ch{case A(left:Tk,right:Tk); case B(left:Tk,right:Tk);}
         machine f(x:@mutable Ch)->Tk{match x {Ch::A{..} -> move x.left, Ch::B{..} -> move x.right,}}",
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
