#[allow(dead_code)]
#[path = "obligation_outputs/pipeline.rs"]
mod pipeline;
#[path = "support/source.rs"]
mod sources;
use omega_lowering::{BlockTerminator, CallArgument, CleanupAction, OperationKind, OperationResult, Parameter, PlaceProjection, ProjectedPlace, ReturnEdge};
use omega_typed::ObligationKind;
const PAIR: &str = "data Token { value:u8; } data Pair { left:Token; right:Token; }";

/// Receipt for the L5 readiness report, printed after the checks above it pass.
fn observed(label: &str) {
    if std::env::var_os("OMEGA_LAYER_REPORT").is_some() {
        println!("OMEGA_L5_ASSERT\t{label}");
    }
}

#[test]
fn moving_a_field_returns_its_claim_and_disposes_the_opposite_field() {
    for (selected, remaining) in [("left", "right"), ("right", "left")] {
        let source = format!("{PAIR} machine pick(p:Pair)->Token{{move p.{selected}}}");
        let (package, transfer) = {
            let resolved = sources::resolve(&[("pick.omg", &source)]);
            let prepared = omega_preparation::prepare_source_package(&resolved).unwrap();
            let typed = prepared.typed();
            let tree = typed.trees().next().unwrap();
            let file = tree.resolved().file;
            let (operand, claimed, transfer) = typed
                .obligations
                .iter()
                .find_map(|o| match &o.kind {
                    omega_typed::ObligationKind::Move { place, ty } => Some((file.nodes[o.node.node].children[0], (place, ty), o.site.clone())),
                    _ => None,
                })
                .unwrap();
            assert_eq!(
                typed
                    .obligations
                    .iter()
                    .filter(|o| matches!(o.kind, omega_typed::ObligationKind::Move { .. }))
                    .count(),
                1
            );
            assert_eq!(transfer.span.slice(source.as_bytes()).unwrap(), format!("move p.{selected}").as_bytes());
            assert_eq!(claimed.0.span.slice(source.as_bytes()).unwrap(), format!("p.{selected}").as_bytes());
            assert_eq!(claimed.1.nominal_parts().unwrap().0.path, "Token");
            let (capture, projection) = typed.place_path(file.node_id(operand));
            assert_eq!(file.span(capture.node).slice(source.as_bytes()), Some(b"p".as_slice()));
            assert!(
                matches!(projection.as_slice(),[step] if matches!(&step.projection,Some(omega_typed::Projection::Field(f)) if f.path==format!("Pair::{selected}")))
            );
            (sources::lower_prepared(&prepared, &[("pick.omg", &source)]).unwrap(), transfer)
        };
        let pick = &package.machines[0];
        let [Parameter::Structural(root)] = pick.body.parameters[..] else {
            panic!("owned pair expected")
        };
        let (source, cleanup) =
            psi_handoff_consumer::returned_move(&pick.body, &pick.body.blocks[0]).expect("the returned claim must come from the selected field");
        assert_eq!(source.root, root);
        assert!(matches!(source.path.as_slice(),[PlaceProjection::Field(f)] if f.path==format!("Pair::{selected}")));
        let [CleanupAction::DiscardResidual { place, ty }] = cleanup else {
            panic!("only the live sibling should be discarded")
        };
        assert_eq!(place.root, root);
        assert!(matches!(place.path.as_slice(),[PlaceProjection::Field(f)] if f.path==format!("Pair::{remaining}")));
        assert_eq!(Some(*ty), pick.result_type);

        // The operation retained at the prepared move's own site transfers the field
        // its obligation names, and that transferred place is the returned claim.
        let moved = pipeline::operation_at(&pick.body, &transfer);
        assert!(
            matches!(&moved.kind, OperationKind::MoveStructural { source: taken } if taken == source),
            "{selected}: {:?}",
            moved.kind
        );
        let BlockTerminator::Return {
            result: ReturnEdge::Structural { source: claim, .. },
            ..
        } = pick.body.blocks[0].terminator
        else {
            panic!("structural return expected")
        };
        assert_eq!(moved.result, OperationResult::Structural(claim));
        assert_eq!(Some(pick.body.places[claim.index()].ty), pick.result_type, "the claim has the moved type");
    }
    observed("obligation-output/move");
}

#[test]
fn each_return_arm_owns_only_its_field_complement() {
    let source = format!("{PAIR} machine pick(p:Pair,first:bool)->Token{{match first {{true->move p.left,false->move p.right,}}}}");
    let package = sources::lower(&[("arms.omg", &source)]).unwrap();
    let body = &package.machines[0].body;
    let residuals: Vec<_> = body
        .blocks
        .iter()
        .filter_map(|block| {
            let BlockTerminator::Return { cleanup, .. } = &block.terminator else {
                return None;
            };
            let [CleanupAction::DiscardResidual { place, .. }] = cleanup.as_slice() else {
                panic!("expected single remaining field")
            };
            let [PlaceProjection::Field(field)] = place.path.as_slice() else {
                panic!("field expected")
            };
            Some(field.path.as_str())
        })
        .collect();
    assert_eq!(residuals, ["Pair::right", "Pair::left"]);
}

#[test]
fn transferring_a_field_to_a_call_does_not_dispose_the_argument_twice() {
    for (field, sibling) in [("right", "left"), ("left", "right")] {
        let source = format!("{PAIR} machine take(t:Token) {{}} machine apply(p:Pair){{take(move p.{field});}}");
        let package = sources::lower(&[("call.omg", &source)]).unwrap();
        let apply = package
            .machines
            .iter()
            .find(|m| m.declaration.path == "apply")
            .unwrap();
        // `negative_controls.rs` redirects or removes this residual cleanup and requires this witness to reject it.
        psi_handoff_consumer::product_witness::transfer_leaves_sibling(&apply.body, &format!("Pair::{field}"), &format!("Pair::{sibling}")).unwrap();
    }
}

#[test]
fn nominal_cleanup_invokes_the_selected_target_with_the_original_subject() {
    for target in ["finish", "release"] {
        let source = format!(
            "data Lease dropped by {target} {{ mark:u8; }} machine finish(x:Lease) {{let Lease {{mark as _}}=x;}} machine release(y:Lease) {{let Lease {{mark as _}}=y;}} machine use_it(r:Lease) {{}}"
        );
        let lowered = pipeline::lower(&source);
        let package = &lowered.package;
        let selected = package
            .machines
            .iter()
            .find(|m| m.declaration.path == target)
            .unwrap();
        let caller = package
            .machines
            .iter()
            .find(|m| m.declaration.path == "use_it")
            .unwrap();
        let [Parameter::Structural(input)] = caller.body.parameters[..] else {
            panic!("owned parameter expected")
        };
        let BlockTerminator::Return { cleanup, .. } = &caller.body.blocks[0].terminator else {
            panic!("return expected")
        };
        spec_corpus::observe_handoff!(
            "cleanup-selection/consumer",
            assert!(
                matches!(cleanup.as_slice(),[CleanupAction::Invoke{place,ty,target}] if place.root==input && place.path.is_empty() && *ty==caller.parameter_types[0] && *target==selected.id)
            )
        );
        assert!(matches!(selected.body.operations[0].kind,OperationKind::DecomposeRecord{ref fields,..} if fields[0].field.path=="Lease::mark"));
        assert!(matches!(&selected.body.blocks[0].terminator,BlockTerminator::Return{cleanup,..} if cleanup.is_empty()));

        // The one cleanup contract sits on Lease's authored selection and names the
        // selected machine. Psi records that machine on the same declaration's schema,
        // whose carrier is the subject the normal exit hands to it.
        let contracts = lowered.obligations("CleanupContract");
        let [contract] = contracts.as_slice() else {
            panic!("{target}: expected one cleanup contract, found {contracts:?}")
        };
        let ObligationKind::CleanupContract(machine) = &contract.kind else {
            unreachable!()
        };
        assert_eq!((machine.package.as_str(), machine.path.as_str()), (pipeline::PACKAGE, target));
        assert_eq!(selected.declaration.path, machine.path);
        assert_eq!(lowered.spelling(&contract.site), format!("dropped by {target}"));
        let lease = package
            .data
            .iter()
            .find(|d| d.declaration.path == "Lease")
            .unwrap();
        assert!(
            lease.site.span.get_start_offset() <= contract.site.span.get_start_offset()
                && contract.site.span.get_end_offset() <= lease.site.span.get_end_offset(),
            "{target}: the contract belongs to Lease"
        );
        assert_eq!(lease.cleanup_target, Some(selected.id));
        assert_eq!(lease.carrier, caller.parameter_types[0]);
    }
    observed("obligation-output/cleanup-contract");
}

#[test]
fn building_a_record_moves_each_field_once_in_authored_order() {
    let source = format!("{PAIR} machine pack(p:Pair)->Pair{{Pair{{right:move p.left,left:move p.right}}}}");
    let package = sources::lower(&[("pack.omg", &source)]).unwrap();
    let body = &package.machines[0].body;
    let [left, right, record] = body.operations.as_slice() else {
        panic!("two field moves followed by construction")
    };
    let OperationKind::EstablishRecord { fields } = &record.kind else {
        panic!("record expected")
    };
    for (input, field, name) in [
        (right, &fields[0], "Pair::left"),
        (left, &fields[1], "Pair::right"),
    ] {
        assert_eq!(field.field.path, name);
        assert!(matches!(field.operand,CallArgument::MoveWhole(p) if input.result==OperationResult::Structural(p)));
    }
    assert!(matches!(&body.blocks[0].terminator,BlockTerminator::Return{cleanup,..} if cleanup.is_empty()));

    // Moving `p.left` twice is the checker's refusal at the second transfer
    // (terminal-psi/ownership.md#partial-ownership-and-residuals). Psi keeps both
    // transfers at their own authored sites and feeds each one to its field, so the
    // checker can find the overlap. Normal cleanup names only `p.right`, the field no
    // transfer claimed.
    let duplicate = source.replace("left:move p.right", "left:move p.left");
    let package = sources::lower(&[("duplicate.omg", &duplicate)]).unwrap();
    let body = &package.machines[0].body;
    let [Parameter::Structural(root)] = body.parameters[..] else {
        panic!("one owned pair")
    };
    let [first, second, record] = body.operations.as_slice() else {
        panic!("two field moves followed by construction: {:?}", body.operations)
    };
    let OperationKind::EstablishRecord { fields } = &record.kind else {
        panic!("record expected")
    };
    let span = |op: &omega_lowering::PsiOperation| {
        let site = &body.diagnostics.operations[op.id.index()].span;
        site.get_start_offset()..site.get_end_offset()
    };
    let field_of = |place: &omega_lowering::ProjectedPlace| match place.path.as_slice() {
        [PlaceProjection::Field(f)] if place.root == root => Some(f.path.clone()),
        _ => None,
    };
    let transfers = [first, second].map(|op| match &op.kind {
        OperationKind::MoveStructural { source } => (field_of(source), span(op)),
        other => panic!("field transfer expected: {other:?}"),
    });
    let supplied: Vec<_> = fields
        .iter()
        .map(|f| {
            let CallArgument::MoveWhole(input) = f.operand else {
                panic!("owned field expected")
            };
            let ordinal = [first, second]
                .iter()
                .position(|op| op.result == OperationResult::Structural(input));
            (f.field.path.clone(), ordinal)
        })
        .collect();
    let BlockTerminator::Return { cleanup, .. } = &body.blocks[0].terminator else {
        panic!("return expected")
    };
    let residuals: Vec<_> = cleanup
        .iter()
        .map(|action| match action {
            CleanupAction::DiscardResidual { place, .. } => field_of(place),
            other => panic!("only residual disposal expected: {other:?}"),
        })
        .collect();
    // A reader that sees only Psi pairs the second transfer with the first, so the
    // overlap is recoverable without the source.
    let overlaps = psi_handoff_consumer::overlapping_transfers(body);
    let left = || Some("Pair::left".to_owned());
    spec_corpus::observe_handoff!(psi "psi-output/duplicate-field-transfer", assert_eq!(
        (transfers, supplied, residuals, overlaps),
        (
            [
                (left(), sources::span_within(&duplicate, "right:move p.left", "move p.left")),
                (left(), sources::span_within(&duplicate, "left:move p.left", "move p.left")),
            ],
            vec![("Pair::left".to_owned(), Some(1)), ("Pair::right".to_owned(), Some(0))],
            vec![Some("Pair::right".to_owned())],
            vec![(first.id, second.id)],
        )
    ));
}

#[test]
fn residual_cleanup_invokes_the_hook_on_the_exact_live_field() {
    let source = "data Token {value:u8;} data Lease dropped by finish {mark:u8;} data Pair {left:Token;right:Lease;} machine finish(x:Lease){let Lease{mark as _}=x;} machine pick(p:Pair)->Token{move p.left}";
    let package = sources::lower(&[("residual.omg", source)]).unwrap();
    let hook = package
        .machines
        .iter()
        .find(|m| m.declaration.path == "finish")
        .unwrap();
    let body = &package
        .machines
        .iter()
        .find(|m| m.declaration.path == "pick")
        .unwrap()
        .body;
    let (taken, actions) = psi_handoff_consumer::returned_move(body, &body.blocks[0]).unwrap();
    assert!(
        matches!(actions,[CleanupAction::Invoke{place,target,ty}] if *target==hook.id && *ty==hook.parameter_types[0] && place.root==taken.root && matches!(place.path.as_slice(),[PlaceProjection::Field(f)] if f.path=="Pair::right"))
    );
}

#[test]
fn owned_replacement_distinguishes_a_vacant_field_from_its_live_sibling() {
    for (destination, vacant) in [("left", true), ("right", false)] {
        let source = format!("{PAIR} machine replace(p:Pair,new:Token)->Token{{let mutable q=move p;let old=move q.left;q.{destination}=move new;old}}");
        let package = sources::lower(&[("replace.omg", &source)]).unwrap();
        let body = &package.machines[0].body;
        let (written, disposition) = body
            .operations
            .iter()
            .find_map(|op| match &op.kind {
                OperationKind::StructuralStore { destination, disposition, .. } => Some((destination, disposition)),
                _ => None,
            })
            .unwrap();
        assert!(matches!(written.path.as_slice(),[PlaceProjection::Field(f)] if f.path==format!("Pair::{destination}")));
        let BlockTerminator::Return { cleanup, .. } = &body.blocks[0].terminator else {
            panic!("expected return")
        };
        if vacant {
            assert_eq!(disposition, &omega_lowering::StructuralStoreDisposition::FillVacancy);
            assert_eq!(cleanup, &[CleanupAction::DiscardRoot(written.root)]);
        } else {
            assert!(
                matches!(disposition,omega_lowering::StructuralStoreDisposition::Dispose(actions) if matches!(actions.as_slice(),[CleanupAction::DiscardResidual{place,..}] if place==written))
            );
            assert!(matches!(cleanup.as_slice(),[CleanupAction::DiscardResidual{place,..}] if place==written));
        }
    }
}

#[test]
fn transferring_an_owned_match_result_uses_the_join_parameter() {
    let source = format!("{PAIR} machine select(p:Pair,flag:bool)->Token{{let chosen=match flag{{true->move p.left,false->move p.left,}};chosen}}");
    let package = sources::lower(&[("join.omg", &source)]).unwrap();
    let body = &package.machines[0].body;
    let join = body
        .blocks
        .iter()
        .find(|b| !b.structural_parameters.is_empty())
        .unwrap();
    let [parameter] = join.structural_parameters[..] else {
        panic!("one joined claim")
    };
    let mut inputs = Vec::new();
    for block in &body.blocks {
        if let BlockTerminator::Jump(edge) = &block.terminator
            && edge.target == join.id
        {
            let [CallArgument::MoveWhole(p)] = edge.structural_arguments[..] else {
                panic!("an owned join must transfer its input")
            };
            inputs.push(p);
        }
    }
    assert_eq!(inputs.len(), 2);
    assert_ne!(inputs[0], inputs[1]);
    assert!(
        body.operations
            .iter()
            .any(|o| matches!(&o.kind,OperationKind::MoveStructural{source} if source.root==parameter && source.path.is_empty()))
    );
    // Both arms move `p.left`, so both arrivals leave `p.right` and the join keeps that residual.
    let witness = psi_handoff_consumer::product_witness::join_arrivals_move_fields;
    witness(body, "Pair::left", "Pair::left").unwrap();
    witness(body, "Pair::left", "Pair::right").expect_err("the false arm moves left, not right");
    let BlockTerminator::Return { cleanup, .. } = &join.terminator else {
        panic!("the join returns the claim")
    };
    assert!(
        matches!(cleanup.as_slice(), [CleanupAction::DiscardResidual { place, .. }] if matches!(place.path.as_slice(), [PlaceProjection::Field(f)] if f.path == "Pair::right"))
    );

    // When the arms move different fields, one arrival leaves `p.right` and the
    // other `p.left`. Arrivals that disagree at a join are the checker's refusal
    // (terminal-psi/control_flow.md#graph-and-frontier), so Psi must keep each arm's
    // own transfer and its undisposed remainder up to the join for that comparison.
    let different = source.replace("false->move p.left", "false->move p.right");
    let package = sources::lower(&[("join.omg", &different)]).unwrap();
    let body = &package.machines[0].body;
    spec_corpus::observe_handoff!(psi "psi-output/join-arrival-transfers", assert_eq!(witness(body, "Pair::left", "Pair::right"), Ok(())));
    witness(body, "Pair::left", "Pair::left").expect_err("the false arm moves right");
    // Lowering schedules the cleanup after the join from its first arrival, the true
    // arm. The checker refuses at the join before judging that cleanup.
    let join = body
        .blocks
        .iter()
        .find(|b| !b.structural_parameters.is_empty())
        .unwrap();
    assert!(matches!(&join.terminator, BlockTerminator::Return { cleanup, .. }
        if matches!(cleanup.as_slice(), [CleanupAction::DiscardResidual { place, .. }] if matches!(place.path.as_slice(), [PlaceProjection::Field(f)] if f.path == "Pair::right"))));
}

#[test]
fn reinitializing_an_earlier_local_preserves_reverse_declaration_cleanup_order() {
    let source = "data Token {n:u8;} machine order(a:Token,b:Token,c:Token){let mutable first=move a;let second=move b;let removed=move first;first=move c;}";
    let package = sources::lower(&[("order.omg", source)]).unwrap();
    let body = &package.machines[0].body;
    let declared: Vec<_> = body
        .operations
        .iter()
        .filter_map(|o| match o.result {
            OperationResult::Structural(p) => Some(p),
            _ => None,
        })
        .take(3)
        .collect();
    let BlockTerminator::Return { cleanup, .. } = &body.blocks[0].terminator else {
        panic!("return expected")
    };
    assert_eq!(
        cleanup,
        &[
            CleanupAction::DiscardRoot(declared[2]),
            CleanupAction::DiscardRoot(declared[1]),
            CleanupAction::DiscardRoot(declared[0])
        ]
    );
}

#[test]
fn an_unrestricted_field_read_preserves_the_whole_owned_record() {
    for (formal, expected) in [("p:Envelope", true), ("p:@Envelope", false)] {
        let source = format!("data Byte [copy] {{n:u8;}} data Envelope {{byte:Byte;}} machine get({formal})->Byte{{p.byte}}");
        let lowered = pipeline::lower(&source);
        let package = &lowered.package;
        let body = &package.machines[0].body;
        let [Parameter::Structural(root)] = body.parameters[..] else {
            panic!("source expected")
        };
        assert!(
            matches!(&body.operations[0].kind,OperationKind::CopyStructural{source} if source.root==root && source.path.iter().any(|p|matches!(p,PlaceProjection::Field(f) if f.path=="Envelope::byte")))
        );
        let BlockTerminator::Return { cleanup, .. } = &body.blocks[0].terminator else {
            panic!("return expected")
        };
        if expected {
            assert_eq!(cleanup, &[CleanupAction::DiscardRoot(root)]);
        } else {
            assert!(cleanup.is_empty());
        }

        // Only the read through the loan owes `CopyRead`. The operation retained at
        // that read's site copies the field through the referent, and the copy is
        // returned. The owned twin copies the same field without crossing a loan.
        let copy = &body.operations[0];
        let OperationKind::CopyStructural { source: copied } = &copy.kind else {
            unreachable!()
        };
        let reads = lowered.obligations("CopyRead");
        if expected {
            assert!(reads.is_empty(), "{formal}: an owned field read crosses no loan");
            assert!(
                matches!(copied.path.as_slice(), [PlaceProjection::Field(f)] if f.path == "Envelope::byte"),
                "{formal}: {:?}",
                copied.path
            );
        } else {
            let [read] = reads.as_slice() else {
                panic!("{formal}: expected one copy read, found {reads:?}")
            };
            assert_eq!(lowered.spelling(&read.site), "p.byte");
            let ObligationKind::CopyRead(ty) = &read.kind else { unreachable!() };
            assert_eq!(ty.nominal_parts().unwrap().0.path, "Byte");
            assert_eq!(pipeline::operation_at(body, &read.site), copy, "{formal}: the read site is the copy");
            assert!(
                matches!(copied.path.as_slice(), [PlaceProjection::Referent, PlaceProjection::Field(f)] if f.path == "Envelope::byte"),
                "{formal}: {:?}",
                copied.path
            );
            assert!(lowered.obligations("ReturnBorrow").is_empty(), "{formal}: a copied result carries no loan");
        }
        let OperationResult::Structural(value) = copy.result else {
            panic!("{formal}: a record copy yields a place")
        };
        let byte = package
            .data
            .iter()
            .find(|d| d.declaration.path == "Byte")
            .unwrap();
        assert_eq!(body.places[value.index()].ty, byte.carrier);
        assert!(
            matches!(pipeline::returns(body).as_slice(), [ReturnEdge::Structural { source, .. }] if *source == value),
            "{formal}: the copy is returned"
        );
    }
    observed("obligation-output/copy-read");
}

#[test]
fn a_cleanup_pattern_transfers_bound_children_and_disposes_ignored_children() {
    let source = "data Token {value:u8;} data Lease dropped by release {kept:Token;ignored:Token;} machine release(x:Lease){let Lease{..}=x;} machine unpack(x:Lease)->Token{let Lease{kept,..}=x;kept}";
    let package = sources::lower(&[("parts.omg", source)]).unwrap();
    let unpack = package
        .machines
        .iter()
        .find(|m| m.declaration.path == "unpack")
        .unwrap();
    let OperationKind::DecomposeRecord { fields, .. } = &unpack.body.operations[0].kind else {
        panic!("partition expected")
    };
    let [kept, ignored] = fields.as_slice() else {
        panic!("two partition outputs")
    };
    assert_eq!(kept.field.path, "Lease::kept");
    let OperationResult::Structural(kept) = kept.result else {
        panic!("owned child")
    };
    let OperationResult::Structural(ignored) = ignored.result else {
        panic!("owned child")
    };
    assert!(matches!(&unpack.body.blocks[0].terminator,BlockTerminator::Jump(edge) if edge.cleanup==[CleanupAction::DiscardRoot(ignored)]));
    let (returned, cleanup) = psi_handoff_consumer::returned_move(&unpack.body, &unpack.body.blocks[1]).unwrap();
    assert_eq!(returned.root, kept);
    assert!(cleanup.is_empty());
    let release = package
        .machines
        .iter()
        .find(|m| m.declaration.path == "release")
        .unwrap();
    for machine in [unpack, release] {
        assert_eq!(psi_handoff_consumer::hooked_field_transfers(&package, &machine.body), Ok(vec![]));
    }

    // A field move cannot take apart a value whose declaration selects a cleanup
    // (wiki/spec/language/ownership.md, "Paths and control flow"). L6 owns that
    // verdict, so L5 publishes the exact move, the live complement and the
    // selection. No action runs `release` on the partially moved subject.
    let illegal = source.replace("let Lease{kept,..}=x;kept", "move x.kept");
    let package = sources::lower(&[("parts.omg", &illegal)]).unwrap();
    let machine = |name: &str| package.machines.iter().find(|m| m.declaration.path == name).unwrap();
    let (release, unpack) = (machine("release"), machine("unpack"));
    let lease = package.data.iter().find(|d| d.declaration.path == "Lease").unwrap();
    let [Parameter::Structural(x)] = unpack.body.parameters[..] else {
        panic!("one owned subject")
    };
    let (moved, cleanup) = psi_handoff_consumer::returned_move(&unpack.body, &unpack.body.blocks[0]).unwrap();
    let field = |place: &omega_lowering::ProjectedPlace| match place.path.as_slice() {
        [PlaceProjection::Field(f)] => f.path.clone(),
        _ => String::new(),
    };
    spec_corpus::observe_handoff!(psi "psi-output/hooked-field-move", assert_eq!(
        (lease.cleanup_target, moved.root, field(moved), cleanup.iter().map(|a| match a {
            CleanupAction::DiscardResidual { place, .. } if place.root == x => field(place),
            other => format!("{other:?}"),
        }).collect::<Vec<_>>()),
        (Some(release.id), x, "Lease::kept".to_string(), vec!["Lease::ignored".to_string()])
    ));
    let rows = psi_handoff_consumer::hooked_field_transfers(&package, &unpack.body).unwrap();
    assert!(
        matches!(rows.as_slice(), [row] if row.source == moved && row.owner.is_empty() && row.cleanup == release.id),
        "{rows:?}"
    );

    // The same rule applies below a transparent owner: the residual keeps the
    // sibling and the hooked owner's remaining field, and the reader reports
    // the hooked owner's prefix.
    let nested = format!("{} data Pair {{left:Token;right:Lease;}} machine split(p:Pair)->Token{{move p.right.kept}}", &source[..source.find("machine unpack").unwrap()]);
    let package = sources::lower(&[("parts.omg", &nested)]).unwrap();
    let split = package.machines.iter().find(|m| m.declaration.path == "split").unwrap();
    let [Parameter::Structural(p)] = split.body.parameters[..] else {
        panic!("one owned pair")
    };
    let (moved, cleanup) = psi_handoff_consumer::returned_move(&split.body, &split.body.blocks[0]).unwrap();
    let paths = |place: &omega_lowering::ProjectedPlace| {
        place
            .path
            .iter()
            .map(|step| match step {
                PlaceProjection::Field(f) => f.path.clone(),
                other => format!("{other:?}"),
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(moved.root, p);
    assert_eq!(paths(moved), ["Pair::right", "Lease::kept"]);
    let residual: Vec<_> = cleanup
        .iter()
        .map(|a| match a {
            CleanupAction::DiscardResidual { place, .. } if place.root == p => paths(place),
            other => vec![format!("{other:?}")],
        })
        .collect();
    assert_eq!(residual, [vec!["Pair::right", "Lease::ignored"], vec!["Pair::left"]]);
    let release = package.machines.iter().find(|m| m.declaration.path == "release").unwrap();
    let rows = psi_handoff_consumer::hooked_field_transfers(&package, &split.body).unwrap();
    assert!(
        matches!(rows.as_slice(), [row] if row.source == moved && paths(&ProjectedPlace{root:p,path:row.owner.to_vec()}) == ["Pair::right"] && row.cleanup == release.id),
        "{rows:?}"
    );

    // Through an exclusive loan the move is a borrowed move of the referent's
    // field. The reader reports the referent as the hooked owner.
    let borrowed = format!(
        "{} machine swap(x:@mutable Lease)->Token{{let t=move x.kept; x.kept=Token{{value:1}}; t}}",
        &source[..source.find("machine unpack").unwrap()]
    );
    let package = sources::lower(&[("parts.omg", &borrowed)]).unwrap();
    let swap = package.machines.iter().find(|m| m.declaration.path == "swap").unwrap();
    let release = package.machines.iter().find(|m| m.declaration.path == "release").unwrap();
    let [Parameter::Structural(x)] = swap.body.parameters[..] else {
        panic!("one borrowed subject")
    };
    let rows = psi_handoff_consumer::hooked_field_transfers(&package, &swap.body).unwrap();
    let [row] = rows.as_slice() else {
        panic!("{rows:?}")
    };
    assert!(swap.body.operations.iter().any(|op| matches!(&op.kind, OperationKind::BorrowedMove { source } if source == row.source)));
    assert_eq!(row.source.root, x);
    assert_eq!(row.owner, [PlaceProjection::Referent]);
    assert!(matches!(row.source.path.as_slice(), [PlaceProjection::Referent, PlaceProjection::Field(f)] if f.path == "Lease::kept"));
    assert_eq!(row.cleanup, release.id);
}

#[test]
fn terminal_transfer_disposes_the_later_root_before_the_remaining_hooked_field() {
    let source = "data Token {value:u8;} data Lease dropped by release {mark:u8;} data Pair {left:Token;right:Lease;} machine release(x:Lease){let Lease{mark as _}=x;} machine pass(t:Token)->Token{t} machine finish(p:Pair,extra:Lease)->Token{transition {_ -> pass(move p.left),}}";
    let package = sources::lower(&[("terminal.omg", source)]).unwrap();
    let body = &package
        .machines
        .iter()
        .find(|m| m.declaration.path == "finish")
        .unwrap()
        .body;
    let [Parameter::Structural(pair), Parameter::Structural(extra)] = body.parameters[..] else {
        panic!("two owners")
    };
    let actions = body
        .blocks
        .iter()
        .find_map(|block| match &block.terminator {
            BlockTerminator::TerminalCall { cleanup, .. } => Some(cleanup),
            _ => None,
        })
        .unwrap();
    let [
        CleanupAction::Invoke { place: first, target: t, .. },
        CleanupAction::Invoke { place: second, target: u, .. },
    ] = actions.as_slice()
    else {
        panic!("two selected hooks")
    };
    assert_eq!(t, u);
    assert_eq!(first.root, extra);
    assert!(first.path.is_empty());
    assert_eq!(second.root, pair);
    assert!(matches!(second.path.as_slice(),[PlaceProjection::Field(f)] if f.path=="Pair::right"));
}

#[test]
fn an_intact_subject_in_its_cleanup_retains_the_recursive_edge() {
    let source = "data Lease dropped by finish { mark:u8; } machine finish(x:Lease) {}";
    let package = sources::lower(&[("recursive.omg", source)]).unwrap();
    let finish = &package.machines[0];
    let [Parameter::Structural(p)] = finish.body.parameters[..] else {
        panic!("cleanup input expected")
    };
    let BlockTerminator::Return { cleanup, .. } = &finish.body.blocks[0].terminator else {
        panic!("normal death edge expected")
    };
    assert!(matches!(cleanup.as_slice(),[CleanupAction::Invoke{place,target,..}] if *target==finish.id && place.root==p && place.path.is_empty()));
}

#[test]
fn a_join_agrees_when_one_arm_transfers_every_field_and_the_other_transfers_the_root() {
    let source = format!(
        "{PAIR} machine rebuild(p:Pair,flag:bool)->Pair{{let joined=match flag{{true->Pair{{left:move p.left,right:move p.right}},false->move p,}};joined}}"
    );
    let package = sources::lower(&[("rebuild.omg", &source)]).unwrap();
    let body = &package.machines[0].body;
    let join = body
        .blocks
        .iter()
        .find(|block| !block.structural_parameters.is_empty())
        .unwrap();
    assert!(matches!(&join.terminator,BlockTerminator::Return{cleanup,..} if cleanup.is_empty()));
    let inputs: Vec<_> = body
        .blocks
        .iter()
        .filter_map(|block| match &block.terminator {
            BlockTerminator::Jump(e) if e.target == join.id => Some(&e.structural_arguments[..]),
            _ => None,
        })
        .collect();
    assert!(matches!(inputs.as_slice(),[[CallArgument::MoveWhole(a)],[CallArgument::MoveWhole(b)]] if a!=b));
}

#[test]
fn moving_a_sum_payload_field_disposes_only_the_selected_cases_other_field() {
    use omega_lowering::PlaceProjection::{Case, Field};
    for (selected, remaining) in [("left", "right"), ("right", "left")] {
        let source = format!(
            "data Token {{v:u8;}} data Choice {{case A(left:Token,right:Token);case B(left:Token,right:Token);}} machine pick(x:Choice)->Token{{match x{{Choice::A{{..}}->move x.{selected},Choice::B{{..}}->move x.{selected},}}}}"
        );
        let package = sources::lower(&[("sum.omg", &source)]).unwrap();
        let body = &package.machines[0].body;
        let [Parameter::Structural(root)] = body.parameters[..] else {
            panic!("sum parameter expected")
        };
        let BlockTerminator::Conditional {
            condition,
            when_true,
            when_false,
        } = &body.blocks[0].terminator
        else {
            panic!("discriminator branch expected")
        };
        assert!(body.operations.iter().any(|o|o.result==OperationResult::Scalar(*condition) && matches!(&o.kind,OperationKind::StructuralCaseMembership{source,case} if source.root==root && source.path.is_empty() && case.path=="Choice::A")));
        for (edge, case) in [(when_true, "A"), (when_false, "B")] {
            let tagged = edge.selected_case.as_ref().unwrap();
            assert_eq!(tagged.source.root, root);
            assert_eq!(tagged.case.path, format!("Choice::{case}"));
            let (taken, cleanup) = psi_handoff_consumer::returned_move(body, &body.blocks[edge.target.get() - 1]).unwrap();
            assert_eq!(taken.root, root);
            assert!(matches!(taken.path.as_slice(),[Case(c),Field(f)] if c==&tagged.case && f.path==format!("Choice::{case}::{selected}")));
            spec_corpus::observe_handoff!(
                psi "psi-output/case-remainder",
                assert!(
                    matches!(cleanup,[CleanupAction::DiscardResidual{place,..}] if place.root==root && matches!(place.path.as_slice(),[Case(c),Field(f)] if c==&tagged.case && f.path==format!("Choice::{case}::{remaining}")))
                ),
            );
        }
    }
}

#[test]
fn partial_sum_cleanup_calls_a_hook_only_if_its_field_is_in_the_active_payload() {
    let src = "data Token {v:u8;} data Lease dropped by release {mark:u8;} data Choice {case A(left:Token,right:Lease);case B(left:Token,right:Token);} machine release(x:Lease){let Lease{mark as _}=x;} machine pick(x:Choice)->Token{match x{Choice::A{..}->move x.left,Choice::B{..}->move x.left,}}";
    let package = sources::lower(&[("hooked.omg", src)]).unwrap();
    let body = &package
        .machines
        .iter()
        .find(|m| m.declaration.path == "pick")
        .unwrap()
        .body;
    let release = package
        .machines
        .iter()
        .find(|m| m.declaration.path == "release")
        .unwrap();
    let BlockTerminator::Conditional { when_true, when_false, .. } = &body.blocks[0].terminator else {
        panic!("expected discriminator")
    };
    let (_, a) = psi_handoff_consumer::returned_move(body, &body.blocks[when_true.target.get() - 1]).unwrap();
    let (_, b) = psi_handoff_consumer::returned_move(body, &body.blocks[when_false.target.get() - 1]).unwrap();
    assert!(
        matches!(a,[CleanupAction::Invoke{place,target,..}] if *target==release.id && matches!(place.path.as_slice(),[PlaceProjection::Case(c),PlaceProjection::Field(f)] if c.path=="Choice::A" && f.path=="Choice::A::right"))
    );
    assert!(
        matches!(b,[CleanupAction::DiscardResidual{place,..}] if matches!(place.path.as_slice(),[PlaceProjection::Case(c),PlaceProjection::Field(f)] if c.path=="Choice::B" && f.path=="Choice::B::right"))
    );
}

#[test]
fn partial_move_from_a_sum_field_disposes_its_active_sibling_and_the_holders_other_field() {
    let src = "data Token{v:u8;} data Choice{case A(left:Token,right:Token);case B(left:Token,right:Token);} data Holder{choice:Choice;spare:Token;} machine pick(h:Holder)->Token{match h.choice{Choice::A{..}->move h.choice.left,Choice::B{..}->move h.choice.left,}}";
    let package = sources::lower(&[("held.omg", src)]).unwrap();
    let body = &package.machines[0].body;
    let BlockTerminator::Conditional { when_true, when_false, .. } = &body.blocks[0].terminator else {
        panic!("expected nested sum discriminator")
    };
    for (edge, case) in [(when_true, "Choice::A"), (when_false, "Choice::B")] {
        let (source, cleanup) = psi_handoff_consumer::returned_move(body, &body.blocks[edge.target.get() - 1]).unwrap();
        spec_corpus::observe_handoff!(
            "subject-and-field-identity-across-matches/consumer",
            assert!(
                matches!(source.path.as_slice(),[PlaceProjection::Field(holder),PlaceProjection::Case(c),PlaceProjection::Field(f)] if holder.path=="Holder::choice" && c.path==case && f.path==format!("{case}::left"))
            )
        );
        assert!(
            matches!(cleanup,[CleanupAction::DiscardResidual{place:spare,..},CleanupAction::DiscardResidual{place:sibling,..}] if matches!(spare.path.as_slice(),[PlaceProjection::Field(f)] if f.path=="Holder::spare") && matches!(sibling.path.as_slice(),[PlaceProjection::Field(holder),PlaceProjection::Case(c),PlaceProjection::Field(f)] if holder.path=="Holder::choice" && c.path==case && f.path==format!("{case}::right")))
        );
    }
}

#[test]
fn fixed_array_residuals_follow_the_declared_extent_above_four_thousand() {
    let src = "data Cell {v:u8;} machine take(items:[Cell;4097])->Cell {move items[2048]}";
    let package = sources::lower(&[("large-array.omg", src)]).unwrap();
    let body = &package.machines[0].body;
    let Parameter::Structural(items) = body.parameters[0] else {
        panic!("array input")
    };
    let BlockTerminator::Return { cleanup, .. } = &body.blocks[0].terminator else {
        panic!("normal return")
    };
    assert_eq!(cleanup.len(), 4096);
    for (action, ordinal) in cleanup.iter().zip((0..4097).rev().filter(|i| *i != 2048)) {
        assert!(matches!(action, CleanupAction::DiscardResidual{place,..} if place.root==items && place.path==[PlaceProjection::FixedIndex(ordinal)]));
    }
}

#[test]
fn a_grouped_call_argument_transfers_the_selected_field_to_the_callee() {
    spec_corpus::observe_handoff!("grouped-call-field/consumer", {
        for (property, access, refill, obligation) in [
            (" [copy]", "@", "", "CopyRead"),
            ("", "@mutable ", "h.item=B{n:0};", "RestoreBorrowed"),
        ] {
            let src = format!("data B{property} {{n:u8;}} data H{{item:B;}} data W{{b:B;}} machine wrap(b:B)->W{{W{{b:b}}}} machine run(h:{access}H)->W{{let w=wrap((h.item));{refill}w}}");
            let lowered = pipeline::lower(&src);
            let body = &lowered.package.machines.iter().find(|m| m.declaration.path == "run").unwrap().body;
            let obligations = lowered.obligations(obligation);
            let [transfer] = obligations.as_slice() else { panic!("expected one {obligation}") };
            assert_eq!(lowered.spelling(&transfer.site), "h.item");
            let operation = pipeline::operation_at(body, &transfer.site);
            let source = match (&operation.kind, access) {
                (OperationKind::CopyStructural { source }, "@") | (OperationKind::BorrowedMove { source }, "@mutable ") => source,
                _ => panic!("wrong transfer for {access}: {:?}", operation.kind),
            };
            assert!(matches!(source.path.as_slice(), [PlaceProjection::Referent, PlaceProjection::Field(field)] if field.path == "H::item"));
            let OperationResult::Structural(argument) = operation.result else { panic!("expected a field value") };
            let calls: Vec<_> = body.operations.iter().filter_map(|op| match &op.kind {
                OperationKind::Call { arguments, .. } => Some(arguments.as_slice()),
                _ => None,
            }).collect();
            let expected = match access {
                "@" => CallArgument::CopyWhole(argument),
                _ => CallArgument::MoveWhole(argument),
            };
            assert_eq!(calls, [&[expected][..]], "{access} field supplied to wrap");
        }
    });
}

#[test]
fn consuming_a_moved_record_partitions_the_move_result_and_keeps_the_other_owner() {
    for (selected, other) in [(0, 1), (1, 0)] {
        let name = ["first", "second"][selected];
        let source = format!(
            "data C dropped by finish {{ v:u8; }} machine finish(value:C){{let C{{v as _}}=move value;}}
             machine take(first:C, second:C)->u8{{let C{{v}}=(move {name}); v}}"
        );
        let product = sources::lower(&[("move.omg", &source)]).unwrap();
        let take = product.machines.iter().find(|m| m.declaration.path == "take").unwrap();
        let finish = product.machines.iter().find(|m| m.declaration.path == "finish").unwrap();
        let [transfer, partition] = take.body.operations.as_slice() else { panic!("one transfer and one partition") };
        let Parameter::Structural(chosen) = take.body.parameters[selected] else { panic!("owned record") };
        let Parameter::Structural(survivor) = take.body.parameters[other] else { panic!("owned sibling") };
        assert!(matches!(&transfer.kind, OperationKind::MoveStructural { source } if source.root == chosen && source.path.is_empty()));
        assert!(matches!(&partition.kind, OperationKind::DecomposeRecord { source, fields, .. }
            if source.path.is_empty() && OperationResult::Structural(source.root) == transfer.result
                && fields.len() == 1 && fields[0].field.path == "C::v"));
        assert!(matches!(&take.body.blocks[0].terminator, BlockTerminator::Return { cleanup, .. }
            if matches!(&cleanup[..], [CleanupAction::Invoke { place, target, .. }]
                if place.root == survivor && place.path.is_empty() && *target == finish.id)));
        assert!(matches!(&finish.body.blocks[0].terminator, BlockTerminator::Return { cleanup, .. } if cleanup.is_empty()));
    }
}

#[test]
fn consuming_a_call_result_evaluates_the_record_producer_once() {
    let source = "data C { v:u8; } machine make(v:u8)->C{C{v:v}}
                  machine take(v:u8)->u8{let C{v as result_value}=make(v);result_value}";
    let product = sources::lower(&[("call.omg", source)]).unwrap();
    let take = product.machines.iter().find(|m| m.declaration.path == "take").unwrap();
    let [call, partition] = take.body.operations.as_slice() else { panic!("one call and one partition") };
    assert!(matches!(call.kind, OperationKind::Call { .. }));
    assert!(matches!(&partition.kind, OperationKind::DecomposeRecord { source, .. }
        if source.path.is_empty() && OperationResult::Structural(source.root) == call.result));
}

#[test]
fn a_consuming_pattern_retains_a_borrowed_move_and_its_exact_backing() {
    let text = "data C dropped by finish { v:u8; } machine finish(x:C){let C{v as _}=move x;}
                data Holder{item:C;} machine get(h:@mutable Holder)->u8{let C{v}=move h.item;v}";
    let product = sources::lower(&[("loan.omg", text)]).unwrap();
    let get = product.machines.iter().find(|m| m.declaration.path == "get").unwrap();
    let [transfer, partition] = get.body.operations.as_slice() else { panic!("one borrowed move and one partition") };
    let Parameter::Structural(holder) = get.body.parameters[0] else { panic!("holder reference") };
    assert!(matches!(&transfer.kind, OperationKind::BorrowedMove { source }
        if source.root == holder && matches!(&source.path[..], [PlaceProjection::Referent, PlaceProjection::Field(f)] if f.path == "Holder::item")));
    assert!(matches!(&partition.kind, OperationKind::DecomposeRecord { source, .. }
        if source.path.is_empty() && OperationResult::Structural(source.root) == transfer.result));

    let shared = text.replace("@mutable Holder", "@Holder");
    let package = sources::resolve(&[("loan.omg", &shared)]);
    let error = omega_preparation::prepare_source_package(&package).unwrap_err();
    assert!(error.message.contains("borrowed storage requires a copy type"));
    assert_eq!(error.span.slice(shared.as_bytes()), Some(b"h.item".as_slice()));
}
