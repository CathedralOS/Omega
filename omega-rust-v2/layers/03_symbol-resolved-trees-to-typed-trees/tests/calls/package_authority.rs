use omega_resolver::Dependency;
use omega_typer::{TypeError, TypedTrees, type_stage};

fn type_carried_point(
    source: &str,
    direct: bool,
) -> Result<TypedTrees<'_>, TypeError> {
    let dependencies = [
        Dependency {
            requester: "app",
            alias: "dep",
            package: "facade",
        },
        Dependency {
            requester: "facade",
            alias: "geom",
            package: "geometry",
        },
        Dependency {
            requester: "app",
            alias: "chosen_geometry",
            package: "geometry",
        },
    ];
    let resolved = support::resolve_packages(
        &[
            ("app", "main.omg", source),
            ("facade", "api.omg", "public machine point() -> geom::Point { geom::Point { x: 7, y: 8 } }"),
            (
                "geometry",
                "point.omg",
                "public data Point { x: u8; y: u8; } public machine + add(a: Point, b: Point) -> Point { a }",
            ),
        ],
        &dependencies[..if direct { 3 } else { 2 }],
    );
    type_stage(&resolved)
}

#[test]
fn carrying_a_value_does_not_grant_access_to_its_fields() {
    type_carried_point("machine run() { let p = dep::point(); let q = p; }", false).unwrap();
    for (source, selection) in [
        ("machine run() -> u8 { let p = dep::point(); p.x }", "p.x"),
        ("machine run() { let mut p = dep::point(); p.x = 1; }", "p.x"),
        ("machine run() -> u8 { let p = dep::point(); let (value, _) = p; value }", "value"),
        ("machine run() -> u8 { let p = dep::point(); let (value, ..) = p; value }", "value"),
    ] {
        let error = type_carried_point(source, false).unwrap_err();
        assert_eq!(error.package, "app");
        assert_eq!(error.message, "selecting geometry::Point requires a direct dependency on geometry");
        assert_eq!(error.span.slice(source.as_bytes()), Some(selection.as_bytes()));
        type_carried_point(source, true).unwrap();
    }
}

#[test]
fn carrying_a_value_does_not_grant_access_to_its_operators() {
    let source = "machine run() { let p = dep::point(); let q = dep::point(); let sum = p + q; }";
    let error = type_carried_point(source, false).unwrap_err();
    assert_eq!(error.package, "app");
    assert_eq!(error.span.slice(source.as_bytes()), Some(b"p + q".as_slice()));
    assert!(error.message.starts_with("no matching operator +"), "{error:?}");

    let typed = type_carried_point(source, true).unwrap();
    assert!(
        typed
            .calls
            .iter()
            .any(|call| call.machine.package == "geometry" && call.machine.path == "add")
    );
}
