//! Typed obligation input to Psi output, one representative kind per test.
//!
//! Each case lowers a real source package through the ordinary path. The test
//! first locates the obligation in the prepared trees by kind and source site,
//! then lowers those trees and drops them. The Psi operation retained at that
//! same site must have the expected variant, subject and operands. Source twins
//! change one authored choice. Each twin's expected Psi is written from the
//! source, not read back from the product. These tests do not judge the
//! obligation; L6 does.
#[path = "obligation_outputs/pipeline.rs"]
mod pipeline;
#[allow(dead_code)]
#[path = "support/source.rs"]
mod sources;

use omega_lowering::{
    CallArgument, CallRequirement, CallSite, CallTarget, OperationKind, OperationResult, Parameter, PlaceProjection, ReferenceAccess, ReturnEdge, ScalarType,
    TypeShape,
};
use omega_typed::{Access, ObligationKind, ValueType};
use pipeline::{operation_at, place_producer, returns, value_producer};

/// Receipt for the L5 readiness report, printed after the checks above it pass.
/// `tools/spec-corpus/lowering_evidence.py` registers each label with its test
/// and credits it only when that test passes.
fn observed(label: &str) {
    if std::env::var_os("OMEGA_LAYER_REPORT").is_some() {
        println!("OMEGA_L5_ASSERT\t{label}");
    }
}

#[test]
fn a_zero_value_establishes_the_selected_nominal_home_at_its_binding() {
    for name in ["First", "Other"] {
        let binding = format!("let cell:{name};");
        let source = format!("data First {{v:u8;}} data Other {{v:u8;}} machine init(){{{binding}}}");
        let lowered = pipeline::lower(&source);
        let obligations = lowered.obligations("ZeroValue");
        let [zero] = obligations.as_slice() else {
            panic!("expected the local's zero-value obligation")
        };
        assert_eq!(lowered.spelling(&zero.site), binding);
        assert!(
            matches!(&zero.kind, ObligationKind::ZeroValue(ValueType::Nominal(id)) if id.package == "app" && id.path == name),
            "{:?}",
            zero.kind
        );

        let body = &lowered.machine("init").body;
        let establish = operation_at(body, &zero.site);
        assert_eq!(establish.kind, OperationKind::EstablishZeroedStructural);
        let OperationResult::Structural(home) = establish.result else {
            panic!("expected a structural home")
        };
        let carrier = body.places[home.index()].ty;
        let schema = lowered
            .package
            .data
            .iter()
            .find(|schema| schema.carrier == carrier)
            .unwrap();
        assert_eq!(schema.declaration.path, name);
        assert!(matches!(&body.blocks[0].terminator, omega_lowering::BlockTerminator::Return { cleanup, .. }
            if cleanup == &[omega_lowering::CleanupAction::DiscardRoot(home)]));
    }
    observed("obligation-output/zero-value");
}

/// A zeroed local of a predicate-bearing record lowers whether or not zero
/// satisfies the predicate, because the binding's `ZeroValue` verdict belongs to
/// the checker. Zero fails `self.value > 0` for `Nonzero` and satisfies
/// `self.value < 10` for `Small`; both bindings produce the same zeroed home, and
/// that home's schema keeps its own clause for the checker to evaluate at zero.
/// Removing the predicate leaves the same operation with no clause, so the clause
/// is the only difference the checker sees.
#[test]
fn a_zero_value_carries_its_record_predicate_to_the_checker() {
    use omega_lowering::{BlockTerminator, CleanupAction, PredicateOperator, PredicateStep};
    let mut compared = false;
    for (name, predicate, bound, operator) in [
        ("Nonzero", "self.value > 0", 0, PredicateOperator::GreaterThan),
        ("Small", "self.value < 10", 10, PredicateOperator::LessThan),
    ] {
        let binding = format!("let mutable cell:{name};");
        let declaration = format!("data {name} requires {predicate} {{value:u8;}}");
        for qualified in [true, false] {
            let source = if qualified {
                declaration.clone()
            } else {
                format!("data {name} {{value:u8;}}")
            } + &format!(" machine init(){{{binding}}}");
            let lowered = pipeline::lower(&source);
            let obligations = lowered.obligations("ZeroValue");
            let [zero] = obligations.as_slice() else {
                panic!("{source}: expected the local's zero-value obligation")
            };
            assert_eq!(lowered.spelling(&zero.site), binding);
            assert!(
                matches!(&zero.kind, ObligationKind::ZeroValue(ValueType::Nominal(id)) if id.path == name),
                "{:?}",
                zero.kind
            );
            let body = &lowered.machine("init").body;
            let establish = operation_at(body, &zero.site);
            assert_eq!(establish.kind, OperationKind::EstablishZeroedStructural, "{source}");
            let OperationResult::Structural(home) = establish.result else {
                panic!("expected a structural home")
            };
            assert_eq!(body.mutable_homes, [home]);
            assert!(matches!(&body.blocks[0].terminator, BlockTerminator::Return { cleanup, .. } if cleanup == &[CleanupAction::DiscardRoot(home)]));
            let schema = lowered
                .package
                .data
                .iter()
                .find(|schema| schema.carrier == body.places[home.index()].ty)
                .unwrap();
            assert_eq!(schema.declaration.path, name);
            if !qualified {
                assert!(schema.requires.is_empty(), "{source}");
                continue;
            }
            let [clause] = schema.requires.as_slice() else {
                panic!("{source}: expected one default-domain clause")
            };
            assert_eq!(lowered.spelling(&clause.site), format!("requires {predicate}"));
            let field = schema.fields[0].declaration.clone();
            assert_eq!(field.path, format!("{name}::value"));
            spec_corpus::observe_handoff!(
                psi "psi-output/zero-value-record-predicate",
                assert_eq!(
                    clause.predicate,
                    [PredicateStep::SelfValue, PredicateStep::Field(field), PredicateStep::Integer(bound.into()), PredicateStep::Apply(operator)]
                ),
            );
            compared = true;
        }
    }
    assert!(compared);

    // Gating propagates through containment: a copyable predicate-bearing field
    // lowers inside the zeroed home and its own schema keeps the clause.
    let binding = "let mutable cell:Holder;";
    let source = format!("data Inner [copy] requires self.v > 0 {{v:u8;}} data Holder {{inner:Inner;}} machine init(){{{binding}}}");
    let lowered = pipeline::lower(&source);
    let [zero] = lowered.obligations("ZeroValue")[..] else {
        panic!("expected the local's zero-value obligation")
    };
    let body = &lowered.machine("init").body;
    let establish = operation_at(body, &zero.site);
    assert_eq!(establish.kind, OperationKind::EstablishZeroedStructural);
    let OperationResult::Structural(home) = establish.result else {
        panic!("expected a structural home")
    };
    let schema = |carrier| {
        lowered
            .package
            .data
            .iter()
            .find(|schema| schema.carrier == carrier)
            .unwrap()
    };
    let holder = schema(body.places[home.index()].ty);
    assert_eq!((holder.declaration.path.as_str(), holder.requires.len()), ("Holder", 0));
    let inner = schema(holder.fields[0].carrier);
    assert_eq!(inner.declaration.path, "Inner");
    let [clause] = inner.requires.as_slice() else {
        panic!("expected the contained clause")
    };
    assert_eq!(lowered.spelling(&clause.site), "requires self.v > 0");
}

/// `(signed, width)` of a fixed-width integer, spelled as in source.
type Integer = (bool, u64);

fn spelled((signed, width): Integer) -> String {
    format!("{}{width}", if signed { "i" } else { "u" })
}

fn typed((signed, width): Integer) -> ValueType {
    ValueType::Integer { signed, width: Some(width) }
}

fn scalar((signed, width): Integer) -> ScalarType {
    ScalarType::Integer { signed, width }
}

const CALLEES: &str = "machine checked(first:u8, second:u8)->u8 requires first > 0 requires second > first {second}
                       machine relaxed(first:u8, second:u8)->u8 {second}";

/// A call whose callee has two `requires` clauses carries one `CallPrecondition`
/// naming that callee at the call site. The Psi operation at that site calls the
/// same machine with the authored operand order and asks for clause 0 then clause 1
/// at that operation. Swapping the operands changes only the arguments. Calling a
/// callee without clauses produces neither the obligation nor any requirement.
#[test]
fn call_precondition_selects_the_callee_operands_and_ordered_clauses_at_its_call() {
    // (call, selected callee, caller formal supplying each operand, requirement clauses)
    for (call, callee, slots, clauses) in [
        ("checked(b, a)", "checked", [1, 0], &[0, 1][..]),
        ("checked(a, b)", "checked", [0, 1], &[0, 1][..]),
        ("relaxed(b, a)", "relaxed", [1, 0], &[][..]),
    ] {
        let source = format!("{CALLEES} machine caller(a:u8, b:u8)->u8 {{{call}}}");
        let lowered = pipeline::lower(&source);

        let preconditions = lowered.obligations("CallPrecondition");
        if clauses.is_empty() {
            assert!(preconditions.is_empty(), "{call}: a callee without requirements owes no precondition");
        } else {
            let [obligation] = preconditions.as_slice() else {
                panic!("{call}: expected one call precondition, found {preconditions:?}")
            };
            assert_eq!(lowered.spelling(&obligation.site), call);
            let ObligationKind::CallPrecondition(machine) = &obligation.kind else {
                unreachable!()
            };
            assert_eq!((machine.package.as_str(), machine.path.as_str()), (pipeline::PACKAGE, callee));
        }

        let selected = lowered.machine(callee);
        assert_eq!(
            (selected.declaration.package.as_str(), selected.declaration.path.as_str()),
            (pipeline::PACKAGE, callee)
        );
        let body = &lowered.machine("caller").body;
        let [Parameter::Scalar(a), Parameter::Scalar(b)] = body.parameters[..] else {
            panic!("{call}: expected two scalar formals")
        };
        let formals = [a, b];
        let [operation] = body.operations.as_slice() else {
            panic!("{call}: expected one ordinary call, found {:?}", body.operations)
        };
        let site = &body.diagnostics.operations[operation.id.index()];
        assert_eq!(lowered.spelling(site), call);
        if let [obligation] = preconditions.as_slice() {
            assert_eq!(
                operation_at(body, &obligation.site),
                operation,
                "{call}: the precondition site is the call operation"
            );
        }
        let OperationKind::Call {
            callee: CallTarget::Machine(target),
            arguments,
            requirements,
        } = &operation.kind
        else {
            panic!("{call}: expected a direct machine call, found {:?}", operation.kind)
        };
        assert_eq!(*target, selected.id, "{call}");
        assert_eq!(*arguments, slots.map(|slot| CallArgument::Scalar(formals[slot])), "{call}");
        let expected: Vec<_> = clauses
            .iter()
            .map(|&clause| CallRequirement {
                site: CallSite::Operation(operation.id),
                clause,
            })
            .collect();
        assert_eq!(*requirements, expected, "{call}");
        assert_eq!(selected.contract.requires.len(), clauses.len(), "{call}: one requirement per authored clause");
    }
    observed("obligation-output/call-precondition");
}

/// An integer value cast carries one `ExactCast` from the operand type to the
/// destination carrier at the cast. The Psi operation at that site is an exact
/// integer conversion of the selected formal, typed by the source and the
/// destination carriers, and its value is returned. Twins select the other
/// formal, narrow from a signed carrier, or change only signedness.
#[test]
fn exact_cast_converts_the_selected_operand_to_the_destination_carrier_at_its_cast() {
    // (formal carrier, cast, selected formal, destination)
    for (from, cast, slot, to) in [
        ((false, 8), "x as u16", 0, (false, 16)),
        ((false, 8), "y as u16", 1, (false, 16)),
        ((true, 16), "y as u8", 1, (false, 8)),
        ((false, 8), "x as i8", 0, (true, 8)),
    ] {
        let source = format!("machine convert(x:{0}, y:{0})->{1} {{ {cast} }}", spelled(from), spelled(to));
        let lowered = pipeline::lower(&source);

        let casts = lowered.obligations("ExactCast");
        let [obligation] = casts.as_slice() else {
            panic!("{cast}: expected one exact cast, found {casts:?}")
        };
        assert_eq!(lowered.spelling(&obligation.site), cast);
        assert_eq!(
            obligation.kind,
            ObligationKind::ExactCast {
                source: typed(from),
                destination: typed(to),
            },
            "{cast}"
        );
        assert!(lowered.obligations("Recast").is_empty(), "{cast}: a value cast is not a recast");

        let package = &lowered.package;
        let body = &lowered.machine("convert").body;
        let [Parameter::Scalar(x), Parameter::Scalar(y)] = body.parameters[..] else {
            panic!("{cast}: expected two scalar formals")
        };
        let operand = [x, y][slot];
        let [operation] = body.operations.as_slice() else {
            panic!("{cast}: expected one conversion, found {:?}", body.operations)
        };
        assert_eq!(operation_at(body, &obligation.site), operation, "{cast}: the cast site is the conversion");
        assert_eq!(operation.kind, OperationKind::ExactNumericCast { operand }, "{cast}");
        let OperationResult::Scalar(result) = operation.result else {
            panic!("{cast}: a conversion yields a scalar")
        };
        assert_eq!(
            package.types.get(body.values[operand.index()].ty),
            Some(&TypeShape::PrimitiveScalar(scalar(from))),
            "{cast}"
        );
        assert_eq!(
            package.types.get(body.values[result.index()].ty),
            Some(&TypeShape::PrimitiveScalar(scalar(to))),
            "{cast}"
        );
        assert_eq!(returns(body), [ReturnEdge::Scalar(result)], "{cast}");
    }
    observed("obligation-output/exact-cast");
}

/// A shared reference cast between equal-width integers carries one `Recast`
/// between the two reference types at the cast. The Psi operation at that site
/// reinterprets the loan of the selected field: its source is a shared borrow of
/// that field of the formal, with the width and both signednesses of the typed
/// pair, and the view is returned. Twins select the other field or the opposite
/// signedness. A value cast of the same field selects `ExactCast` and an exact
/// integer conversion of the field read, never a recast.
#[test]
fn recast_reinterprets_the_selected_borrowed_field_at_its_cast() {
    let referent = |package: &omega_lowering::LoweredPackage, ty: omega_lowering::TypeId| match package.types.get(ty) {
        Some(TypeShape::ScalarReference { referent, access }) => Some((*referent, *access)),
        _ => None,
    };
    // (field declarations, selected field, source referent, destination referent)
    for (fields, field, from, to) in [
        ("low:u8; high:u8;", "high", (false, 8), (true, 8)),
        ("low:u8; high:u8;", "low", (false, 8), (true, 8)),
        ("low:i32; high:i32;", "high", (true, 32), (false, 32)),
    ] {
        let cast = format!("@cell.{field} as @{}", spelled(to));
        let source = format!("data Cell {{{fields}}} machine view(cell:@Cell)->@{} {{ {cast} }}", spelled(to));
        let lowered = pipeline::lower(&source);

        let recasts = lowered.obligations("Recast");
        let [obligation] = recasts.as_slice() else {
            panic!("{cast}: expected one recast, found {recasts:?}")
        };
        assert_eq!(lowered.spelling(&obligation.site), cast);
        assert_eq!(
            obligation.kind,
            ObligationKind::Recast {
                source: ValueType::Reference(Access::Shared, Box::new(typed(from))),
                destination: ValueType::Reference(Access::Shared, Box::new(typed(to))),
            },
            "{cast}"
        );
        assert!(lowered.obligations("ExactCast").is_empty(), "{cast}: a recast converts no value");

        let package = &lowered.package;
        let body = &lowered.machine("view").body;
        let [Parameter::Structural(cell)] = body.parameters[..] else {
            panic!("{cast}: the borrowed record is a descriptor")
        };
        let recast = operation_at(body, &obligation.site);
        let OperationKind::ScalarBitRecast {
            source: borrowed,
            source_referent,
            destination_referent,
        } = recast.kind
        else {
            panic!("{cast}: expected a bit recast, found {:?}", recast.kind)
        };
        assert_eq!((source_referent, destination_referent), (scalar(from), scalar(to)), "{cast}");
        assert_eq!(from.1, to.1);
        let OperationKind::BorrowPlace { source: place, access } = &place_producer(body, borrowed).kind else {
            panic!("{cast}: the recast source is a fresh borrow")
        };
        assert_eq!(*access, ReferenceAccess::Shared, "{cast}");
        assert_eq!(place.root, cell, "{cast}");
        let selected = format!("Cell::{field}");
        assert!(
            matches!(place.path.as_slice(), [PlaceProjection::Referent, PlaceProjection::Field(f)] if f.path == selected),
            "{cast}: borrowed path {:?}",
            place.path
        );
        let OperationResult::Structural(view) = recast.result else {
            panic!("{cast}: a recast yields a descriptor")
        };
        assert_eq!(
            referent(package, body.places[borrowed.index()].ty),
            Some((scalar(from), ReferenceAccess::Shared)),
            "{cast}"
        );
        assert_eq!(
            referent(package, body.places[view.index()].ty),
            Some((scalar(to), ReferenceAccess::Shared)),
            "{cast}"
        );
        assert!(
            matches!(returns(body).as_slice(), [ReturnEdge::Structural { source, .. }] if *source == view),
            "{cast}: the view is returned"
        );
        assert!(
            !body
                .operations
                .iter()
                .any(|op| matches!(op.kind, OperationKind::ExactNumericCast { .. })),
            "{cast}: no value conversion"
        );
    }

    // Value twin: the same field and carriers without the reference marks.
    let cast = "cell.high as i8";
    let source = format!("data Cell {{low:u8; high:u8;}} machine view(cell:Cell)->i8 {{ {cast} }}");
    let lowered = pipeline::lower(&source);
    assert!(lowered.obligations("Recast").is_empty(), "a value cast owes no recast");
    let casts = lowered.obligations("ExactCast");
    let [obligation] = casts.as_slice() else {
        panic!("expected one exact cast, found {casts:?}")
    };
    assert_eq!(lowered.spelling(&obligation.site), cast);
    assert_eq!(
        obligation.kind,
        ObligationKind::ExactCast {
            source: typed((false, 8)),
            destination: typed((true, 8)),
        }
    );
    let body = &lowered.machine("view").body;
    let [Parameter::Structural(cell)] = body.parameters[..] else {
        panic!("the owned record is structural")
    };
    assert!(
        !body
            .operations
            .iter()
            .any(|op| matches!(op.kind, OperationKind::ScalarBitRecast { .. } | OperationKind::BorrowPlace { .. })),
        "a value cast neither borrows nor reinterprets"
    );
    let conversion = operation_at(body, &obligation.site);
    let OperationKind::ExactNumericCast { operand } = conversion.kind else {
        panic!("expected an exact conversion, found {:?}", conversion.kind)
    };
    let OperationKind::ProjectedScalarRead { source: read } = &value_producer(body, operand).kind else {
        panic!("the converted operand is the field read")
    };
    assert_eq!(read.root, cell);
    assert!(
        matches!(read.path.as_slice(), [PlaceProjection::Field(f)] if f.path == "Cell::high"),
        "read path {:?}",
        read.path
    );
    observed("obligation-output/recast");
}
