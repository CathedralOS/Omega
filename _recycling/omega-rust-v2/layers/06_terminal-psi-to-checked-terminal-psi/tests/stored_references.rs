//! A record can store a shared reference to a record that takes lifetime
//! arguments, as `Schema<'t> { storage: @'t Bytes<'t>; }` does. Its field row
//! keeps two facts: the binder the reference is held at, and the binders that
//! fill the record it refers to. These tests check a constructor that copies
//! such a record, as L5 lowers it, then change one fact of the row at a time
//! and require the proof to fail for that fact.
use omega_checker::{FieldOrigin, check_record_constructor};
use omega_psi::{
    DeclarationId, LoweredData, LoweredMachine, LoweredPackage, OperationKind, PlaceProjection, ReferenceAccess, TemplateAccess, TypeExpressionStep as Step,
    TypeShape,
};
use omega_resolver::{ParsedSource, StageSource};

const REFERRED: &str = include_str!("../../05_prepared-trees-to-terminal-psi/tests/fixtures/aggregate_regions/referred_record.omg");

/// The unit L5 lowers for `text`.
fn lowered_unit(text: &str) -> LoweredPackage {
    let tokens = omega_lexer::lex(text.as_bytes()).unwrap();
    let tree = omega_parser::parse_file(text.as_bytes(), &tokens).unwrap();
    let source = ParsedSource {
        logical_path: "referred.omg",
        text,
        tokens: &tokens,
        tree: &tree,
    };
    let package = omega_toolchain::resolve_source_package(&[StageSource { package: "app", source }]).unwrap();
    let prepared = omega_preparation::prepare_source_package(&package).unwrap();
    omega_lowering::lower_package(&prepared).expect("the package lowers")
}

fn machine<'a>(
    unit: &'a LoweredPackage,
    name: &str,
) -> &'a LoweredMachine {
    unit.machines
        .iter()
        .find(|machine| machine.declaration.package == "app" && machine.declaration.path == name)
        .unwrap_or_else(|| panic!("{name} has a body"))
}

/// The unit L5 lowers for `text`, and its constructor `build`.
fn lowered(text: &str) -> (LoweredPackage, DeclarationId) {
    let unit = lowered_unit(text);
    let build = machine(&unit, "build").declaration.clone();
    (unit, build)
}

fn data<'a>(
    unit: &'a mut LoweredPackage,
    name: &str,
) -> &'a mut LoweredData {
    unit.data
        .iter_mut()
        .find(|data| data.declaration.package == "app" && data.declaration.path == name)
        .unwrap_or_else(|| panic!("the package publishes {name}"))
}

#[test]
fn a_constructor_copies_a_record_that_stores_a_reference_to_a_record_at_its_region() {
    let (mut unit, build) = lowered(REFERRED);

    // The row under test is the one L5 published: the reference is held at
    // binder 0 of `Schema`, refers to `Bytes`, and fills binder 0 of `Bytes`
    // from binder 0 of `Schema`.
    let bytes = data(&mut unit, "Bytes").id;
    let schema = data(&mut unit, "Schema");
    let [storage] = schema.fields.as_slice() else { panic!("one stored field") };
    let (lifetime, fills, row) = (storage.lifetime, storage.aggregate_lifetimes.clone(), storage.carrier);
    assert_eq!((lifetime, fills.as_slice()), (Some(0), &[0][..]));
    assert_eq!(
        unit.types.get(row),
        Some(&TypeShape::NominalReference {
            access: ReferenceAccess::Shared,
            referent: bytes
        })
    );

    let checked = check_record_constructor(&unit, &build).unwrap();
    let region = Step::GenericParameter {
        owner: build.clone(),
        index: 0,
    };
    let named = |ty: &omega_psi::TypeExpression| -> Vec<String> {
        ty.0.iter()
            .map(|step| match step {
                Step::Named { declaration, argument_count } => format!("{}/{argument_count}", declaration.path),
                Step::Reference(TemplateAccess::Shared) => "@".to_owned(),
                step if step == &region => "'t".to_owned(),
                other => panic!("{other:?}"),
            })
            .collect()
    };
    let [formal] = checked.parameters() else { panic!("one formal") };
    assert_eq!(named(formal), ["'t", "Schema/1", "@"]);
    assert_eq!(named(&checked.result().ty), ["'t", "Plan/1"]);
    // The plan's first field is the copy of the whole schema, stored reference
    // included. The proof reads that reference's row to admit the copy.
    let [
        (first, FieldOrigin::CopySharedFormal { formal: 0, ty }),
        (_, FieldOrigin::Zero(_)),
    ] = checked.fields()
    else {
        panic!("{:?}", checked.fields())
    };
    assert_eq!(first.path, "Plan::schema");
    assert_eq!(named(ty), ["'t", "Schema/1"]);
}

#[test]
fn each_fact_of_the_stored_reference_row_is_bound_to_its_own_declaration() {
    type Mutation = fn(&mut LoweredPackage);
    let changes: &[(&str, &str, Mutation)] = &[
        (
            "the reference loses the binder it is held at",
            "a stored reference to a record at regions needs its own region",
            |unit| data(unit, "Schema").fields[0].lifetime = None,
        ),
        (
            "the reference is held at a binder its record does not declare",
            "field refers to a missing data region",
            |unit| data(unit, "Schema").fields[0].lifetime = Some(1),
        ),
        (
            "the referent's binder is left unfilled",
            "concrete field type requires its application",
            |unit| data(unit, "Schema").fields[0].aggregate_lifetimes.clear(),
        ),
        (
            "the referent is filled at a binder it does not declare",
            "concrete field type requires its application",
            |unit| data(unit, "Schema").fields[0].aggregate_lifetimes = vec![0, 0],
        ),
        (
            "the referent is filled from a binder the enclosing record does not declare",
            "field refers to a missing data region",
            |unit| data(unit, "Schema").fields[0].aggregate_lifetimes = vec![1],
        ),
        (
            "the referent's declaration gains a binder the row does not fill",
            "concrete field type requires its application",
            |unit| data(unit, "Bytes").lifetime_count = 2,
        ),
        (
            "the referent's own row names another record",
            "field type has no matching concrete data declaration",
            |unit| {
                let other = data(unit, "Plan").carrier;
                data(unit, "Bytes").carrier = other;
            },
        ),
    ];
    for (reason, refusal, change) in changes {
        let (mut unit, build) = lowered(REFERRED);
        check_record_constructor(&unit, &build).unwrap();
        change(&mut unit);
        assert_eq!(check_record_constructor(&unit, &build).unwrap_err(), *refusal, "{reason}");
    }
}

#[test]
fn copying_the_stored_reference_does_not_read_the_record_behind_it() {
    // `Bytes` holds a view that only a read through the stored reference can
    // reach. The constructor copies the reference and never crosses it, so
    // the proof asks nothing of what `Bytes` holds: neither that `Bytes` can
    // be copied, nor that its view has a region a copy could keep.
    let (mut unit, build) = lowered(REFERRED);
    check_record_constructor(&unit, &build).unwrap();
    data(&mut unit, "Bytes").multiplicity = omega_psi::Multiplicity::Affine;
    data(&mut unit, "Bytes").fields[0].lifetime = None;
    check_record_constructor(&unit, &build).unwrap();

    // The plan holds `Schema` by value, so the same loss of copy permission
    // there does reach the proof: that copy reads the record's own storage.
    let (mut unit, build) = lowered(REFERRED);
    data(&mut unit, "Schema").multiplicity = omega_psi::Multiplicity::Affine;
    assert_eq!(
        check_record_constructor(&unit, &build).unwrap_err(),
        "field construction lacks the required custody"
    );
}

#[test]
fn a_constructor_that_reads_through_the_stored_reference_is_not_the_checked_copy() {
    // The twin of `build`. `build` copies the whole schema out of its shared
    // formal, and the stored reference goes with it. `pull` owns the schema
    // and copies out the record behind the stored reference, so its field
    // holds a view that only a read through that reference supplies.
    let pull = "
public data Wrap<'t>
    requires self.count <= 2
{
    bytes: Bytes<'t>;
    count: u64;
}

public machine pull<'t>(schema: Schema<'t>) -> Wrap<'t>
    reaches none crashes none terminates
    ensures result.count == 0
{
    Wrap::<'t> { bytes: $schema.storage, .. }
}
";
    let mut unit = lowered_unit(&format!("{REFERRED}{pull}"));
    let build = machine(&unit, "build").declaration.clone();
    let pull = machine(&unit, "pull").declaration.clone();
    check_record_constructor(&unit, &build).unwrap();

    // L5 published the read as a copy through the stored field and then
    // through the reference stored there.
    let storage = data(&mut unit, "Schema").fields[0].declaration.clone();
    let copies: Vec<_> = machine(&unit, "pull")
        .body
        .operations
        .iter()
        .filter_map(|operation| match &operation.kind {
            OperationKind::CopyStructural { source } => Some(source.path.as_slice()),
            _ => None,
        })
        .collect();
    assert_eq!(copies, [&[PlaceProjection::Field(storage), PlaceProjection::Referent][..]]);

    // The constructor judgment admits a copy only through a shared formal. It
    // has no rule that lets a body claim a borrow held behind a stored reference.
    assert_eq!(
        check_record_constructor(&unit, &pull).unwrap_err(),
        "copy requires a shared reference to the indicated carrier"
    );
}
