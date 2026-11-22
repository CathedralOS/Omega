//! A package publishes the toolchain data its closed bodies and local
//! declarations store, under the toolchain's identity, and no other.
#[allow(dead_code)]
#[path = "support/source.rs"]
mod sources;

use omega_lowering::{LoweredData, LoweredField, LoweredPackage, OperationKind, PlaceProjection, TypeId, TypeShape};
use omega_toolchain::TOOLCHAIN_PACKAGE;

const CORE: &str = "language\\core\\";

/// The toolchain's own package, which defines every schema a consumer republishes.
fn toolchain() -> LoweredPackage {
    let library = omega_toolchain::SourcePackage::toolchain_definition();
    let prepared = omega_preparation::prepare_source_package(&library).unwrap();
    omega_lowering::lower_package(&prepared).unwrap()
}

fn lower(body: &str) -> LoweredPackage {
    let source = format!("use omega\\language\\core;\n{body}");
    sources::lower(&[("consumer.omg", &source)]).unwrap()
}

fn toolchain_paths(package: &LoweredPackage) -> Vec<&str> {
    package
        .data
        .iter()
        .filter(|data| data.declaration.package == TOOLCHAIN_PACKAGE)
        .map(|data| data.declaration.path.strip_prefix(CORE).unwrap())
        .collect()
}

fn schema<'a>(
    package: &'a LoweredPackage,
    owner: &str,
    path: &str,
) -> &'a LoweredData {
    package
        .data
        .iter()
        .find(|data| data.declaration.package == owner && data.declaration.path == path)
        .unwrap_or_else(|| panic!("{owner} {path} has no schema"))
}

fn data_at(
    package: &LoweredPackage,
    ty: TypeId,
) -> &LoweredData {
    let Some(TypeShape::Nominal(id)) = package.types.get(ty) else {
        panic!("expected an owned nominal row, found {:?}", package.types.get(ty))
    };
    &package.data[id.get() - 1]
}

/// Everything a schema states apart from its package-local row numbers. A
/// field's carrier is compared through the declaration its row names.
fn assert_same_schema(
    consumer: &LoweredPackage,
    defining: &LoweredPackage,
    path: &str,
) {
    let path = format!("{CORE}{path}");
    let (ours, theirs) = (schema(consumer, TOOLCHAIN_PACKAGE, &path), schema(defining, TOOLCHAIN_PACKAGE, &path));
    let header = |data: &LoweredData| {
        (
            data.declaration.clone(),
            data.public,
            data.multiplicity,
            data.lifetime_count,
            data.numbering.clone(),
            data.requires.clone(),
            data.site.clone(),
        )
    };
    assert_eq!(header(ours), header(theirs));
    let fields = |package: &LoweredPackage, fields: &[LoweredField]| {
        fields
            .iter()
            .map(|field| {
                let stored = match package.types.get(field.carrier) {
                    Some(TypeShape::Nominal(id)) => Some(package.data[id.get() - 1].declaration.clone()),
                    _ => None,
                };
                (field.declaration.clone(), field.lifetime, field.site.clone(), stored)
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(fields(consumer, &ours.fields), fields(defining, &theirs.fields));
    assert_eq!(ours.cases.len(), theirs.cases.len());
    for (case, defined) in ours.cases.iter().zip(&theirs.cases) {
        assert_eq!(
            (&case.declaration, &case.numbering, &case.site),
            (&defined.declaration, &defined.numbering, &defined.site)
        );
        assert_eq!(fields(consumer, &case.fields), fields(defining, &defined.fields));
    }
}

/// The two programs differ only in the stored type. `Exposure` stores nothing
/// else. `FieldAccess` stores three more toolchain declarations in its cases,
/// and the nine other data declarations of `core.omg` stay unpublished.
#[test]
fn a_closed_body_publishes_the_toolchain_data_it_stores_and_no_other() {
    let defining = toolchain();
    for (stored, expected) in [
        ("Exposure", &["Exposure"][..]),
        ("FieldAccess", &["AtomicOperations", "Exposure", "ExternalRead", "FieldAccess"][..]),
    ] {
        let package = lower(&format!("machine keep(value:core\\{stored})->core\\{stored} {{ value }}"));
        assert_eq!(toolchain_paths(&package), expected, "{stored}");
        assert_eq!(package.data.len(), expected.len(), "{stored}: no local schema");
        for path in expected {
            assert_same_schema(&package, &defining, path);
        }
        let [keep] = package.machines.as_slice() else { panic!("one machine") };
        assert_eq!(keep.declaration.package, "app");
        let formal = data_at(&package, keep.parameter_types[0]);
        assert_eq!(
            (formal.declaration.package.as_str(), formal.declaration.path.as_str()),
            (TOOLCHAIN_PACKAGE, format!("{CORE}{stored}").as_str())
        );
        assert_eq!(keep.result_type, Some(keep.parameter_types[0]));
    }
}

/// `BoundaryReach requires self.service_count <= 32` is written in the
/// toolchain's file. The consumer's copy states that clause at that site, and
/// the body reads the toolchain's field.
#[test]
fn a_republished_schema_keeps_the_predicate_of_its_defining_file() {
    let defining = toolchain();
    let package = lower("machine count(reach:@core\\BoundaryReach)->u64 { reach.service_count }");
    assert_eq!(toolchain_paths(&package), ["BoundaryReach"]);
    assert_same_schema(&package, &defining, "BoundaryReach");
    let reach = schema(&package, TOOLCHAIN_PACKAGE, "language\\core\\BoundaryReach");
    let original = schema(&defining, TOOLCHAIN_PACKAGE, r"language\core\BoundaryReach");
    spec_corpus::observe_handoff!(psi "psi-output/imported-data-predicate", assert_eq!(reach.requires, original.requires));
    let [clause] = reach.requires.as_slice() else {
        panic!("one default predicate")
    };
    assert_eq!(
        (clause.site.package.as_str(), clause.site.file.as_str()),
        (TOOLCHAIN_PACKAGE, "language/core/core.omg")
    );
    let [count] = package.machines.as_slice() else { panic!("one machine") };
    let [read] = count.body.operations.as_slice() else { panic!("one field read") };
    let OperationKind::ProjectedScalarRead { source } = &read.kind else {
        panic!("{:?}", read.kind)
    };
    let [PlaceProjection::Referent, PlaceProjection::Field(field)] = source.path.as_slice() else {
        panic!("{:?}", source.path)
    };
    assert_eq!(field, &reach.fields[1].declaration);
    assert_eq!(
        (field.package.as_str(), field.path.as_str()),
        (TOOLCHAIN_PACKAGE, "language\\core\\BoundaryReach::service_count")
    );
}

/// A local record's field and a body's local are demands too, even when no
/// machine signature names the toolchain type.
#[test]
fn a_local_field_and_a_body_local_demand_their_toolchain_data() {
    let defining = toolchain();
    let record = lower("data Entry { access:core\\FieldAccess; count:u8; }");
    assert_eq!(toolchain_paths(&record), ["AtomicOperations", "Exposure", "ExternalRead", "FieldAccess"]);
    let entry = schema(&record, "app", "Entry");
    let access = data_at(&record, entry.fields[0].carrier);
    assert_eq!(
        access.declaration,
        schema(&defining, TOOLCHAIN_PACKAGE, "language\\core\\FieldAccess").declaration
    );

    let local = lower(
        "machine exported()->bool { let e = core\\Exposure::Exported{}; match e { core\\Exposure::Exported -> true, core\\Exposure::BindingPrivate -> false, } }",
    );
    assert_eq!(toolchain_paths(&local), ["Exposure"]);
    assert_same_schema(&local, &defining, "Exposure");
}

/// Identity is the declaring package and path, never the spelling. A local
/// `Exposure` with the toolchain's cases publishes only itself, and a body that
/// stores both keeps two schemas and two rows.
#[test]
fn a_same_spelled_local_declaration_is_not_the_toolchain_declaration() {
    // No `use` here: the local name and the toolchain's would compete for `Exposure`.
    let twin = "data Exposure { case Exported; case BindingPrivate; }";
    let lower = |machine: &str| sources::lower(&[("consumer.omg", &format!("{twin} {machine}"))]).unwrap();
    let local = lower("machine keep(value:Exposure)->Exposure { value }");
    assert!(toolchain_paths(&local).is_empty());
    assert_eq!(
        data_at(&local, local.machines[0].parameter_types[0])
            .declaration
            .package,
        "app"
    );

    let both = lower("machine keep(mine:Exposure, theirs:omega\\language\\core\\Exposure)->omega\\language\\core\\Exposure { theirs }");
    let [keep] = both.machines.as_slice() else { panic!("one machine") };
    let owners = keep
        .parameter_types
        .iter()
        .map(|&ty| data_at(&both, ty).declaration.package.as_str())
        .collect::<Vec<_>>();
    assert_eq!(owners, ["app", TOOLCHAIN_PACKAGE]);
    assert_ne!(keep.parameter_types[0], keep.parameter_types[1]);
    assert_eq!(keep.result_type, Some(keep.parameter_types[1]));
    assert_eq!(both.data.len(), 2);
}

/// An open template names toolchain types by declaration and has no rows, so
/// it publishes no schema. `Never` needs no process-exit call to be published.
#[test]
fn a_template_demands_nothing_and_never_needs_no_exit_call() {
    let template = lower(
        "use omega\\language\\core\\reflect; use omega\\language\\core\\access;
        machine peek<'t>(schema:@@TypeSchema<'t>)->u64 { 7 }",
    );
    assert!(template.data.is_empty() && template.machines.is_empty());
    let [peek] = template.machine_templates.as_slice() else {
        panic!("one open template")
    };
    assert_eq!(peek.declaration.package, "app");

    let never = lower("machine keep(value:core\\Never)->core\\Never { value }");
    assert_eq!(toolchain_paths(&never), ["Never"]);
    assert_same_schema(&never, &toolchain(), "Never");
}

#[test]
fn imported_schema_fields_retain_each_applied_optional_type_and_its_case_identity() {
    use omega_lowering::{DataArgument, ScalarType};
    let package = lower(r"machine count(s:@core\layout\Schema)->u64 { s.field_count }
                          machine some()->core\Optional<u16> {core\Optional::Some {value:7u16}}");
    let field = schema(&package, TOOLCHAIN_PACKAGE, r"language\core\layout\SchemaField");
    let identity = field.fields.iter().find(|f| f.declaration.path.ends_with("::identity")).unwrap();
    let field_optional = data_at(&package, identity.carrier);
    let some = package.machines.iter().find(|m| m.declaration.path == "some").unwrap();
    let returned_optional = data_at(&package, some.result_type.unwrap());
    assert_ne!(field_optional.id, returned_optional.id);
    for (row, width) in [(field_optional, 64), (returned_optional, 16)] {
        assert_eq!(row.declaration.path, r"language\core\Optional");
        assert_eq!(row.declaration.package, TOOLCHAIN_PACKAGE);
        let [DataArgument::Type(ty)] = row.type_arguments.as_slice() else { panic!("one type argument") };
        assert_eq!(package.types.get(*ty), Some(&TypeShape::PrimitiveScalar(ScalarType::Integer {signed:false,width})));
        let [none, some] = row.cases.as_slice() else { panic!("two cases in declared order") };
        assert!(none.declaration.path.ends_with("::None") && none.fields.is_empty());
        assert!(some.declaration.path.ends_with("::Some"));
        let [value] = some.fields.as_slice() else { panic!("one active payload") };
        assert_eq!(value.carrier, *ty);
        assert_eq!(row.numbering.as_ref().unwrap().members.iter().map(|m| (m.number,m.declaration_index)).collect::<Vec<_>>(), [(0,0),(1,1)]);
    }
    assert!(toolchain_paths(&package).iter().all(|path| !path.ends_with("LayoutPlan")));
}
