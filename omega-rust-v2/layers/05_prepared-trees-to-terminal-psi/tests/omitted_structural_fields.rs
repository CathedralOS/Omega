//! `..` on a record whose omitted fields are not scalars: a sum, a fixed array
//! of scalars, a fixed array of sums and a nested record with its own
//! `requires`. Explicit field omission requests the zero of each omitted field
//! (`wiki/spec/language/data_and_literals.md#explicit-field-omission`): a sum's
//! zero selects its first declared case, an array's zero holds a zero in every
//! element, and a nested record's zero owes that record's own predicates.
//! A record or fixed array whose every leaf is a scalar is zeroed whole by one
//! `EstablishZeroedStructural`, so its extent never sizes the output; a sum is
//! established case by case.
//!
//! The record below has the shape of the toolchain's `AccessPlan` without its
//! imports: unused capacity is an array of payload-free `Empty` slots, counts
//! are zero, and the nested reach carries a bound on its own count. Each test
//! reads the lowered operations from the constructed record down to their
//! leaves; none compares against a rendering the lowerer produced.
#[allow(dead_code)]
#[path = "support/source.rs"]
mod sources;

use omega_lowering::{
    LoweredData, LoweredPackage, MachineBody, OperationId, OperationKind, OperationResult, PlaceId, PlaceOrigin, PredicateOperator, PredicateStep,
    PsiOperation, ScalarLiteral, ScalarType, TypeShape, ValueOperand, ValueOrigin,
};
use omega_preparation::prepare_source_package;
use omega_typed::ObligationKind;

const FILE: &str = "plan.omg";
const LITERAL: &str = "Plan{id:id,..}";
const PLAN: &str = "data Slot{case Empty; case Entry(value:u8);}
data Reach requires self.service_count<=2 {services:[u64;2]; service_count:u64;}
data Holder requires self.n<=2 {slot:Slot; services:[u64;2]; n:u64;}
data Plan requires self.field_count<=2 {id:u8; fields:[Slot;2]; field_count:u64; first:Slot; reach:Reach; holder:Holder;}
machine build(id:u8)->Plan{Plan{id:id,..}}";

fn machine_body<'a>(package: &'a LoweredPackage, machine: &str) -> &'a MachineBody {
    &package.machines.iter().find(|m| m.declaration.path == machine).unwrap_or_else(|| panic!("missing machine {machine}")).body
}

fn schema<'a>(package: &'a LoweredPackage, path: &str) -> &'a LoweredData {
    package.data.iter().find(|d| d.declaration.path == path).unwrap_or_else(|| panic!("missing schema {path}"))
}

/// The operation that defines a structural operand, and whether the operand moves it.
fn defining<'a>(body: &'a MachineBody, operand: &ValueOperand) -> (&'a PsiOperation, bool) {
    let (place, moved) = match operand {
        ValueOperand::MoveWhole(place) => (*place, true),
        ValueOperand::CopyWhole(place) => (*place, false),
        other => panic!("expected a whole structural operand, found {other:?}"),
    };
    (place_operation(body, place), moved)
}

fn place_operation(body: &MachineBody, place: PlaceId) -> &PsiOperation {
    let PlaceOrigin::Operation(id) = body.places[place.index()].origin else { panic!("{place:?} is not an operation result") };
    let operation = &body.operations[id.index()];
    assert_eq!(operation.result, OperationResult::Structural(place));
    operation
}

/// The constant that defines a scalar operand, its defining operation and its type.
fn constant<'a>(package: &'a LoweredPackage, body: &'a MachineBody, operand: &ValueOperand) -> (&'a ScalarLiteral, OperationId, Option<&'a TypeShape>) {
    let ValueOperand::Scalar(value) = operand else { panic!("expected a scalar operand, found {operand:?}") };
    let ValueOrigin::Operation(id) = body.values[value.index()].origin else { panic!("{value:?} is not an operation result") };
    let OperationKind::Constant(literal) = &body.operations[id.index()].kind else { panic!("{value:?} is not a constant") };
    (literal, id, package.types.get(body.values[value.index()].ty))
}

fn result_type(body: &MachineBody, operation: &PsiOperation) -> omega_lowering::TypeId {
    let OperationResult::Structural(place) = operation.result else { panic!("expected a structural result, found {operation:?}") };
    body.places[place.index()].ty
}

const U64: TypeShape = TypeShape::PrimitiveScalar(ScalarType::Integer { signed: false, width: 64 });

/// Everything below one operand, without operation, value or place numbers:
/// two bodies that establish the same value render the same text.
fn render(package: &LoweredPackage, body: &MachineBody, operand: &ValueOperand) -> String {
    let fields = |fields: &[omega_lowering::FieldInitializer]| {
        fields.iter().map(|f| format!("{}={}", f.field.path, render(package, body, &f.operand))).collect::<Vec<_>>().join(", ")
    };
    match operand {
        ValueOperand::Scalar(value) => match &body.values[value.index()].origin {
            ValueOrigin::Operation(id) => match &body.operations[id.index()].kind {
                OperationKind::Constant(literal) => format!("{literal:?}: {:?}", package.types.get(body.values[value.index()].ty).unwrap()),
                other => panic!("unexpected scalar producer {other:?}"),
            },
            ValueOrigin::Parameter => "parameter".into(),
            other => panic!("unexpected scalar origin {other:?}"),
        },
        ValueOperand::MoveWhole(_) | ValueOperand::CopyWhole(_) => {
            let (operation, moved) = defining(body, operand);
            let transfer = if moved { "move" } else { "copy" };
            let ty = result_type(body, operation);
            let named = |kind: &str| package.data.iter().find(|d| d.carrier == ty).map(|d| format!("{kind} {}", d.declaration.path)).expect("nominal result");
            match &operation.kind {
                OperationKind::EstablishRecord { fields: operands } => format!("{transfer} {}{{{}}}", named("record"), fields(operands)),
                OperationKind::EstablishCase { case, fields: operands } => format!("{transfer} {} {}{{{}}}", named("sum"), case.path, fields(operands)),
                OperationKind::EstablishStructuralArray { elements } => {
                    // The array operation names each element place; an owned element moves into it.
                    let elements: Vec<_> = elements.iter().map(|place| render(package, body, &ValueOperand::MoveWhole(*place))).collect();
                    format!("{transfer} [{}]", elements.join(", "))
                }
                OperationKind::EstablishScalarArray { elements } => {
                    let elements: Vec<_> = elements.iter().map(|value| render(package, body, &ValueOperand::Scalar(*value))).collect();
                    format!("{transfer} [{}]", elements.join(", "))
                }
                OperationKind::EstablishZeroedStructural => format!("{transfer} zeroed {:?}", package.types.get(ty).unwrap()),
                other => panic!("unexpected structural producer {other:?}"),
            }
        }
        other => panic!("unexpected operand {other:?}"),
    }
}

/// The record a machine returns, as the operation that establishes it.
fn returned_record<'a>(package: &'a LoweredPackage, machine: &str) -> (&'a MachineBody, &'a PsiOperation) {
    let body = machine_body(package, machine);
    let plan = schema(package, "Plan");
    let records: Vec<_> = body
        .operations
        .iter()
        .filter(|op| matches!(op.kind, OperationKind::EstablishRecord { .. }) && result_type(body, op) == plan.carrier)
        .collect();
    let [record] = records.as_slice() else { panic!("expected one Plan establishment, found {records:?}") };
    (body, record)
}

#[test]
fn omitted_structural_fields_receive_the_zero_of_their_declared_type() {
    let files = [(FILE, PLAN)];
    let resolved = sources::resolve(&files);
    let prepared = prepare_source_package(&resolved).expect("the plan reaches L5");
    // L3 asks for one zero per omitted field, each at the literal that wrote `..`.
    let literal = sources::unique_span(PLAN, LITERAL);
    let requested: Vec<_> = prepared
        .typed()
        .obligations
        .iter()
        .filter_map(|o| match &o.kind {
            ObligationKind::ZeroField { field, ty } => {
                assert_eq!((o.site.span.get_start_offset(), o.site.span.get_end_offset()), (literal.start, literal.end));
                Some((field.path.as_str(), ty.to_string()))
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        requested,
        [
            ("Plan::fields", "[app\\Slot; 2]".to_owned()),
            ("Plan::field_count", "u64".to_owned()),
            ("Plan::first", "app\\Slot".to_owned()),
            ("Plan::reach", "app\\Reach".to_owned()),
            ("Plan::holder", "app\\Holder".to_owned()),
        ]
    );
    let package = sources::lower_prepared(&prepared, &files).unwrap_or_else(|error| panic!("the plan lowers: {} at {:?}", error.message, error.site));
    let (body, record) = returned_record(&package, "build");
    let (plan, slot, reach, holder) = (schema(&package, "Plan"), schema(&package, "Slot"), schema(&package, "Reach"), schema(&package, "Holder"));
    let empty = &slot.cases[0];
    assert_eq!((empty.declaration.path.as_str(), empty.fields.len(), slot.cases[1].declaration.path.as_str()), ("Slot::Empty", 0, "Slot::Entry"));

    let OperationKind::EstablishRecord { fields } = &record.kind else { unreachable!() };
    let [id, capacity, count, first, nested, held] = fields.as_slice() else { panic!("expected six declared slots, found {fields:?}") };
    for (operand, declared) in fields.iter().zip(&plan.fields) {
        assert_eq!(operand.field, declared.declaration);
    }
    // The written field is still the parameter; nothing zeroed it.
    assert!(matches!(&id.operand, ValueOperand::Scalar(v) if body.values[v.index()].origin == ValueOrigin::Parameter));

    // A sum's zero is its first declared case. `Empty` has no payload, so the
    // case operation has no operands, and `Entry` is never established.
    let is_empty_slot = |operation: &PsiOperation| {
        assert_eq!(result_type(body, operation), slot.carrier);
        assert_eq!(operation.kind, OperationKind::EstablishCase { case: empty.declaration.clone(), fields: vec![] });
    };
    let (first_slot, first_moved) = defining(body, &first.operand);
    is_empty_slot(first_slot);
    assert!(first_moved, "an affine slot transfers into the record once");

    // A fixed array of sums establishes every element: two distinct slot places.
    let (array, array_moved) = defining(body, &capacity.operand);
    assert!(array_moved);
    assert_eq!(result_type(body, array), plan.fields[1].carrier);
    assert_eq!(package.types.get(plan.fields[1].carrier), Some(&TypeShape::FixedArray { element: slot.carrier, length: 2 }));
    let OperationKind::EstablishStructuralArray { elements } = &array.kind else { panic!("expected a structural array, found {array:?}") };
    let [left, right] = elements.as_slice() else { panic!("expected one element per declared ordinal, found {elements:?}") };
    assert_ne!(left, right, "each owned element is its own completed place");
    let element_operations = [place_operation(body, *left), place_operation(body, *right)];
    for element in element_operations {
        is_empty_slot(element);
        assert_ne!(element.id, first_slot.id);
    }

    // A scalar omission keeps its existing form: a zero of the declared carrier.
    let (literal_zero, count_zero, count_type) = constant(&package, body, &count.operand);
    assert_eq!((literal_zero, count_type), (&ScalarLiteral::Integer(0.into()), Some(&U64)));

    // `Reach` has only scalar leaves, so one operation zeroes it whole. Its
    // result type selects `Reach`, whose `requires` that operation owes for
    // the all-zero value; no field operand stands in for the predicate.
    let (reach_zero, reach_moved) = defining(body, &nested.operand);
    assert_eq!((&reach_zero.kind, result_type(body, reach_zero)), (&OperationKind::EstablishZeroedStructural, reach.carrier));
    assert!(reach_moved, "an affine record transfers into the plan once");
    let service_count = &reach.fields[1].declaration;
    assert_eq!(service_count.path, "Reach::service_count");
    let bound = |field: &omega_lowering::DeclarationId| {
        vec![
            PredicateStep::SelfValue,
            PredicateStep::Field(field.clone()),
            PredicateStep::Integer(2.into()),
            PredicateStep::Apply(PredicateOperator::LessOrEqual),
        ]
    };
    let [goal] = reach.requires.as_slice() else { panic!("Reach keeps one predicate, found {:?}", reach.requires) };
    assert_eq!(goal.predicate, bound(service_count));

    // `Holder` holds a sum, which is never zeroed whole. It is established
    // from its components: the first case, the scalar array zeroed by one
    // operation, and a zero constant that its own `requires` reads.
    let (holder_record, _) = defining(body, &held.operand);
    assert_eq!(result_type(body, holder_record), holder.carrier);
    let OperationKind::EstablishRecord { fields: holder_fields } = &holder_record.kind else { panic!("expected the holder record, found {holder_record:?}") };
    let [held_slot, services, n] = holder_fields.as_slice() else { panic!("expected three holder slots, found {holder_fields:?}") };
    for (operand, declared) in holder_fields.iter().zip(&holder.fields) {
        assert_eq!(operand.field, declared.declaration);
    }
    let (holder_slot, _) = defining(body, &held_slot.operand);
    is_empty_slot(holder_slot);
    let (services_zero, services_moved) = defining(body, &services.operand);
    assert!(!services_moved, "a scalar array is copied, not transferred");
    assert_eq!(services_zero.kind, OperationKind::EstablishZeroedStructural);
    assert_eq!(result_type(body, services_zero), holder.fields[1].carrier);
    assert!(matches!(package.types.get(holder.fields[1].carrier), Some(&TypeShape::FixedArray { element, length: 2 }) if package.types.get(element) == Some(&U64)));
    let (n_zero, n_zero_id, n_type) = constant(&package, body, &n.operand);
    assert_eq!((n_zero, n_type), (&ScalarLiteral::Integer(0.into()), Some(&U64)));
    assert_ne!(n_zero_id, count_zero, "each omitted scalar field has its own zero");
    let [goal] = holder.requires.as_slice() else { panic!("Holder keeps one predicate, found {:?}", holder.requires) };
    assert_eq!(goal.predicate, bound(&n.field));
    assert_eq!(plan.requires.len(), 1, "the outer record still owes its own predicate");

    // Every operation is one this test followed from the record, every zero
    // reports the literal that requested it, and components precede their use.
    let followed: Vec<OperationId> = element_operations
        .iter()
        .map(|op| op.id)
        .chain([array.id, count_zero, first_slot.id, reach_zero.id, holder_slot.id, services_zero.id, n_zero_id, holder_record.id, record.id])
        .collect();
    assert_eq!(followed, body.operations.iter().map(|op| op.id).collect::<Vec<_>>(), "operations in establishment order");
    for operation in &body.operations {
        let site = &body.diagnostics.operations[operation.id.index()];
        assert_eq!((site.file.as_str(), site.span.get_start_offset(), site.span.get_end_offset()), (FILE, literal.start, literal.end));
    }

    // The same value, written out by hand, establishes the same sums and
    // scalars. Only the scalar-leaf aggregates differ: the hand-written arrays
    // and record list their elements, and the omitted ones are zeroed whole.
    let written = PLAN.replace(
        LITERAL,
        "Plan{id:id,fields:[Slot::Empty,Slot::Empty],field_count:0,first:Slot::Empty,reach:Reach{services:[0,0],service_count:0},\
         holder:Holder{slot:Slot::Empty,services:[0,0],n:0}}",
    );
    let authored = sources::lower(&[(FILE, &written)]).unwrap_or_else(|error| panic!("the written plan lowers: {} at {:?}", error.message, error.site));
    let (authored_body, authored_record) = returned_record(&authored, "build");
    let OperationKind::EstablishRecord { fields: authored_fields } = &authored_record.kind else { unreachable!() };
    let same = |omitted: &omega_lowering::FieldInitializer, authored_field: &omega_lowering::FieldInitializer| {
        assert_eq!(omitted.field, authored_field.field);
        assert_eq!(render(&package, body, &omitted.operand), render(&authored, authored_body, &authored_field.operand), "{}", omitted.field.path);
    };
    assert_eq!(fields.len(), authored_fields.len());
    for position in [0, 1, 2, 3] {
        same(&fields[position], &authored_fields[position]);
    }
    let (authored_holder, _) = defining(authored_body, &authored_fields[5].operand);
    let OperationKind::EstablishRecord { fields: authored_holder_fields } = &authored_holder.kind else { panic!("expected the written holder") };
    same(held_slot, &authored_holder_fields[0]);
    same(n, &authored_holder_fields[2]);
    assert_eq!(render(&package, body, &nested.operand), format!("move zeroed {:?}", package.types.get(reach.carrier).unwrap()));
    assert_eq!(render(&package, body, &services.operand), format!("copy zeroed {:?}", package.types.get(holder.fields[1].carrier).unwrap()));
}

/// Only the first declared case is established, so a reference in another
/// case's payload does not stop the zero. Declaring that case first makes the
/// zero need a borrow that `..` never supplies, and the literal is refused.
#[test]
fn a_sum_zero_establishes_only_its_first_case_and_never_a_reference() {
    let inactive = "data Slot<'r>{case Empty; case Held(at:@'r u8);}
data Plan<'r>{id:u8; first:Slot<'r>;}
machine build<'r>(id:u8){let plan=Plan::<'r>{id:id,..};}";
    let package = sources::lower(&[(FILE, inactive)]).unwrap_or_else(|error| panic!("the inactive reference lowers: {} at {:?}", error.message, error.site));
    let body = machine_body(&package, "build");
    let slot = schema(&package, "Slot");
    let cases: Vec<_> = body.operations.iter().filter_map(|op| match &op.kind {
        OperationKind::EstablishCase { case, fields } => Some((case.path.as_str(), fields.len(), result_type(body, op))),
        _ => None,
    }).collect();
    assert_eq!(cases, [("Slot::Empty", 0, slot.carrier)]);

    let active = inactive.replace("case Empty; case Held(at:@'r u8);", "case Held(at:@'r u8); case Empty;");
    sources::refuse_with_cli_message(
        &[(FILE, &active)],
        FILE,
        sources::unique_span(&active, "Plan::<'r>{id:id,..}"),
        "omitted field `Plan::first` of type `app\\Slot<app\\build<parameter 0>>` needs a borrowed reference for `Slot::Held::at`, which `..` never supplies",
        "omitted field `Plan::first` of type `root\\Slot<root\\build<parameter 0>>` needs a borrowed reference for `Slot::Held::at`, which `..` never supplies",
    );

    // With a scalar in the same payload the first case is established from its zero.
    let scalar = active.replace("at:@'r u8", "at:u8");
    let package = sources::lower(&[(FILE, &scalar)]).unwrap_or_else(|error| panic!("the scalar payload lowers: {} at {:?}", error.message, error.site));
    let body = machine_body(&package, "build");
    let held: Vec<_> = body.operations.iter().filter_map(|op| match &op.kind {
        OperationKind::EstablishCase { case, fields } => Some((case.path.as_str(), fields)),
        _ => None,
    }).collect();
    let [("Slot::Held", fields)] = held.as_slice() else { panic!("expected the first case, found {held:?}") };
    let [at] = fields.as_slice() else { panic!("expected the payload slot, found {fields:?}") };
    assert_eq!(at.field.path, "Slot::Held::at");
    let (literal, _, ty) = constant(&package, body, &at.operand);
    assert_eq!((literal, ty), (&ScalarLiteral::Integer(0.into()), Some(&TypeShape::PrimitiveScalar(ScalarType::Integer { signed: false, width: 8 }))));
}

/// The rule reaches every depth: a reference inside a nested record, or inside
/// an element of an omitted array, leaves the omitted field without a zero.
/// The refusal names the omitted field and the field that needs the borrow.
#[test]
fn a_reference_below_an_omitted_field_refuses_at_the_literal() {
    let nested = "data Reach<'r>{at:@'r u8; n:u64;}
data Plan<'r>{id:u8; reach:Reach<'r>;}
machine build<'r>(id:u8){let plan=Plan::<'r>{id:id,..};}";
    sources::refuse_with_cli_message(
        &[(FILE, nested)],
        FILE,
        sources::unique_span(nested, "Plan::<'r>{id:id,..}"),
        "omitted field `Plan::reach` of type `app\\Reach<app\\build<parameter 0>>` needs a borrowed reference for `Reach::at`, which `..` never supplies",
        "omitted field `Plan::reach` of type `root\\Reach<root\\build<parameter 0>>` needs a borrowed reference for `Reach::at`, which `..` never supplies",
    );
    let element = nested.replace("reach:Reach<'r>;", "reaches:[Reach<'r>;2];");
    sources::refuse_with_cli_message(
        &[(FILE, &element)],
        FILE,
        sources::unique_span(&element, "Plan::<'r>{id:id,..}"),
        "omitted field `Plan::reaches` of type `[app\\Reach<app\\build<parameter 0>>; 2]` needs a borrowed reference for `Reach::at`, which `..` never supplies",
        "omitted field `Plan::reaches` of type `[root\\Reach<root\\build<parameter 0>>; 2]` needs a borrowed reference for `Reach::at`, which `..` never supplies",
    );
    // Changing only the referenced field to a scalar lets the same literal lower.
    let scalar = nested.replace("at:@'r u8", "at:u8");
    let package = sources::lower(&[(FILE, &scalar)]).unwrap_or_else(|error| panic!("the scalar twin lowers: {} at {:?}", error.message, error.site));
    let body = machine_body(&package, "build");
    let reach = schema(&package, "Reach");
    let zeroed: Vec<_> = body.operations.iter().filter(|op| op.kind == OperationKind::EstablishZeroedStructural).map(|op| result_type(body, op)).collect();
    assert_eq!(zeroed, [reach.carrier]);
}

/// The extent of a scalar-leaf array never sizes the output: 65,536 bytes, a
/// terabyte of them, a nested grid and four billion records each take one
/// operation. Every one names its whole type, and a record's `requires` stays
/// on the schema that operation's result selects.
#[test]
fn a_scalar_leaf_array_or_record_is_zeroed_by_one_operation_at_any_extent() {
    let buffer = "data Buffer{length:u64; bytes:[u8;65536];}
machine build(length:u64){let buffer=Buffer{length:length,..};}";
    let package = sources::lower(&[(FILE, buffer)]).unwrap_or_else(|error| panic!("the buffer lowers: {} at {:?}", error.message, error.site));
    let body = machine_body(&package, "build");
    let [zero, record] = body.operations.as_slice() else { panic!("expected one zero and the record, found {} operations", body.operations.len()) };
    assert_eq!(zero.kind, OperationKind::EstablishZeroedStructural);
    let byte = TypeShape::PrimitiveScalar(ScalarType::Integer { signed: false, width: 8 });
    assert!(matches!(package.types.get(result_type(body, zero)), Some(&TypeShape::FixedArray { element, length: 65536 }) if package.types.get(element) == Some(&byte)));
    let OperationKind::EstablishRecord { fields } = &record.kind else { panic!("expected the buffer record, found {record:?}") };
    let [length, bytes] = fields.as_slice() else { panic!("expected both buffer slots, found {fields:?}") };
    assert!(matches!(&length.operand, ValueOperand::Scalar(v) if body.values[v.index()].origin == ValueOrigin::Parameter));
    assert_eq!((bytes.field.path.as_str(), defining(body, &bytes.operand).0.id), ("Buffer::bytes", zero.id));
    assert_eq!((body.places.len(), body.values.len()), (2, 1), "no place or value per byte");

    let table = "data Cell requires self.used<=4 {samples:[f32;4]; used:u64;}
data Table{id:u64; bytes:[u8;1099511627776]; grid:[[u32;65536];65536]; cells:[Cell;4294967296]; cell:Cell;}
machine build(id:u64){let table=Table{id:id,..};}";
    let package = sources::lower(&[(FILE, table)]).unwrap_or_else(|error| panic!("the table lowers: {} at {:?}", error.message, error.site));
    let body = machine_body(&package, "build");
    let (cell, schema) = (schema(&package, "Cell"), schema(&package, "Table"));
    let [bytes, grid, cells, one, record] = body.operations.as_slice() else { panic!("expected four zeros and the record, found {} operations", body.operations.len()) };
    let OperationKind::EstablishRecord { fields } = &record.kind else { panic!("expected the table record, found {record:?}") };
    for (position, zero) in [bytes, grid, cells, one].into_iter().enumerate() {
        assert_eq!(zero.kind, OperationKind::EstablishZeroedStructural);
        assert_eq!(result_type(body, zero), schema.fields[position + 1].carrier);
        assert_eq!(defining(body, &fields[position + 1].operand).0.id, zero.id);
    }
    let extent = |ty| match package.types.get(ty) {
        Some(&TypeShape::FixedArray { element, length }) => (element, length),
        other => panic!("expected a fixed array, found {other:?}"),
    };
    assert_eq!(extent(result_type(body, bytes)).1, 1 << 40);
    let (row, rows) = extent(result_type(body, grid));
    assert_eq!((rows, extent(row).1), (65536, 65536));
    assert_eq!(extent(result_type(body, cells)), (cell.carrier, 1 << 32));
    assert_eq!(result_type(body, one), cell.carrier);
    assert_eq!(cell.requires.len(), 1, "the zeroed cells still owe the cell predicate");
}

/// A sum is not among the shapes one operation zeroes, so an array of sums is
/// established element by element at any extent the host can hold. 70,000
/// elements is past any source-size rule and lowers.
#[test]
fn an_array_of_sums_is_established_per_element_without_a_size_rule() {
    let source = "data Slot{case Empty; case Entry(value:u8);}
data Plan{id:u8; slots:[Slot;70000];}
machine build(id:u8){let plan=Plan{id:id,..};}";
    let package = sources::lower(&[(FILE, source)]).unwrap_or_else(|error| panic!("the slots lower: {} at {:?}", error.message, error.site));
    let body = machine_body(&package, "build");
    let slot = schema(&package, "Slot");
    assert_eq!(body.operations.len(), 70002, "one case per element, the array and the record");
    let (cases, rest) = body.operations.split_at(70000);
    for case in cases {
        assert!(matches!(&case.kind, OperationKind::EstablishCase { case, fields } if case == &slot.cases[0].declaration && fields.is_empty()));
    }
    let [array, record] = rest else { unreachable!() };
    let OperationKind::EstablishStructuralArray { elements } = &array.kind else { panic!("expected the slot array, found {:?}", array.kind) };
    let places: Vec<_> = cases
        .iter()
        .map(|case| match case.result {
            OperationResult::Structural(place) => place,
            other => panic!("expected a slot place, found {other:?}"),
        })
        .collect();
    assert_eq!(elements, &places, "each element is its own place, in order");
    assert!(matches!(record.kind, OperationKind::EstablishRecord { .. }));
}

/// A float field's zero is positive zero: all-zero bits of its own format.
#[test]
fn an_omitted_float_field_receives_zero_bits_of_its_format() {
    use omega_lowering::{IeeeFloatBits, IeeeFloatFormat};
    let source = "data Options{count:u8; ratio:f32; scale:f64;}
machine build(count:u8){let options=Options{count:count,..};}";
    let package = sources::lower(&[(FILE, source)]).unwrap_or_else(|error| panic!("the options lower: {} at {:?}", error.message, error.site));
    let body = machine_body(&package, "build");
    let [ratio, scale, record] = body.operations.as_slice() else { panic!("expected two zeros and the record, found {:?}", body.operations) };
    let OperationKind::EstablishRecord { fields } = &record.kind else { panic!("expected the record, found {record:?}") };
    for (zero, field, bits, format) in [
        (ratio, &fields[1], IeeeFloatBits::Binary32(0), IeeeFloatFormat::Binary32),
        (scale, &fields[2], IeeeFloatBits::Binary64(0), IeeeFloatFormat::Binary64),
    ] {
        assert_eq!(zero.kind, OperationKind::Constant(ScalarLiteral::IeeeFloat(bits)));
        let (literal, id, ty) = constant(&package, body, &field.operand);
        assert_eq!((literal, id, ty), (&ScalarLiteral::IeeeFloat(bits), zero.id, Some(&TypeShape::PrimitiveScalar(ScalarType::IeeeFloat(format)))));
    }
}
