use super::*;
type Error = TypeIdentityVisitError;
type Result<T = ()> = std::result::Result<T, Error>;

struct Visitor {
    owners: Vec<[u8; 32]>,
    names: Vec<String>,
    nodes: usize,
    depth: usize,
    maximum_nodes: usize,
    maximum_depth: usize,
    owned: usize,
    maximum_owned: usize,
    accepted_owner: Option<[u8; 32]>,
}

impl Default for Visitor {
    fn default() -> Self {
        Self {
            owners: Vec::new(),
            names: Vec::new(),
            nodes: 0,
            depth: 0,
            maximum_nodes: 4096,
            maximum_depth: 128,
            owned: 0,
            maximum_owned: 1024 * 1024,
            accepted_owner: None,
        }
    }
}

impl TypeIdentityPackageOwnerVisitor for Visitor {
    fn enter(&mut self) -> Result {
        if self.nodes >= self.maximum_nodes || self.depth >= self.maximum_depth {
            return Err(Error::ResourceLimitExceeded);
        }
        self.nodes += 1;
        self.depth += 1;
        Ok(())
    }
    fn leave(&mut self) {
        self.depth -= 1;
    }
    fn reserve(&mut self, bytes: usize) -> Result {
        self.owned = self
            .owned
            .checked_add(bytes)
            .filter(|total| *total <= self.maximum_owned)
            .ok_or(Error::ResourceLimitExceeded)?;
        Ok(())
    }
    fn package_owner(&mut self, owner: [u8; 32]) -> Result {
        if self
            .accepted_owner
            .is_some_and(|accepted| accepted != owner)
        {
            return Err(Error::UnknownPackage);
        }
        self.owners.push(owner);
        Ok(())
    }
    fn embedded_name(&mut self, name: &str) -> Result {
        if let Some(runtime) = name.strip_prefix("test-runtime:") {
            return visit_type_identity_package_owners(runtime, self);
        }
        self.names.push(name.to_owned());
        Ok(())
    }
}

fn nominal(marker: u8, path: &str) -> String {
    compound(
        "nominal",
        [
            byte_atom("package-owner", &[marker; 32]),
            atom("path", path),
        ],
    )
}

fn named(marker: u8) -> String {
    compound("named", [atom("name", &nominal(marker, "Foreign::Value"))])
}

fn owners(identity: &str) -> Vec<[u8; 32]> {
    let mut visitor = Visitor::default();
    visit_type_identity_package_owners(identity, &mut visitor).unwrap();
    assert_eq!(visitor.depth, 0);
    visitor.owners
}

#[test]
fn canonical_runtime_shapes_visit_only_semantic_package_positions() {
    let compiler_type = compound("compiler-type", [atom("atom", "u64")]);
    assert_eq!(owners(&compiler_type), Vec::<[u8; 32]>::new());
    for tag in ["ref", "ref-mut", "ref-write", "slice"] {
        assert_eq!(owners(&compound(tag, [named(1)])), [[1; 32]]);
    }
    let generic = compound(
        "generic",
        [
            atom("name", &nominal(2, "Wrapper")),
            named(3),
            "unit".into(),
        ],
    );
    assert_eq!(owners(&generic), [[2; 32], [3; 32]]);
    let array = compound(
        "array",
        [generic, atom("const-parameter", &nominal(4, "Count"))],
    );
    assert_eq!(owners(&array), [[2; 32], [3; 32], [4; 32]]);
    let dynamic = compound(
        "dynamic-trait",
        [
            atom("name", &nominal(5, "Trait")),
            atom("conformance", &nominal(6, "Implementation")),
        ],
    );
    assert_eq!(owners(&dynamic), [[5; 32], [6; 32]]);
    assert_eq!(owners("named(name($T0))"), Vec::<[u8; 32]>::new());
    assert_eq!(owners("named(integer-const(7))"), Vec::<[u8; 32]>::new());
}

#[test]
fn declared_domains_ranges_and_layout_schemas_retain_nested_owners() {
    let application = compound(
        "declared-domain",
        [atom("name", &nominal(2, "Domain")), named(3)],
    );
    let domain = compound("declared-domain", [atom("name", &application)]);
    let range = compound(
        "range",
        [
            atom("minimum", &atom("const-name", &nominal(4, "Minimum"))),
            atom(
                "maximum",
                &compound(
                    "add",
                    [
                        atom("integer", "1"),
                        atom("const-name", &nominal(5, "Maximum")),
                    ],
                ),
            ),
        ],
    );
    let layout = compound(
        "compiler-domain",
        [
            atom("family", "omega-layout"),
            atom("grammar", "derived"),
            compound("schema", [named(6)]),
        ],
    );
    assert_eq!(
        owners(&compound("constrained", [named(1), domain, range, layout])),
        [[1; 32], [2; 32], [3; 32], [4; 32], [5; 32], [6; 32]]
    );
    for domain in [
        "arithmetic-domain(name(Checked))",
        "named-constraint(name(opaque))",
        "compiler-domain(family(carry),permission(any-cpu))",
        "compiler-domain(family(value),domain(finite))",
    ] {
        assert_eq!(
            owners(&compound("constrained", ["unit".into(), domain.into()])),
            Vec::<[u8; 32]>::new()
        );
    }
}

#[test]
fn licensed_index_operations_visit_algebra_and_operation_not_contract_text() {
    let misleading = nominal(9, "NotASelectedPackage");
    let operation = compound(
        "open-index-operation",
        [
            atom("symbol", &nominal(1, "add")),
            atom("contract", &misleading),
        ],
    );
    let algebra = compound(
        "open-index-algebra",
        [
            atom("provider", &nominal(2, "Provider")),
            atom("trait", &nominal(3, "Algebra")),
            atom("requirement", "add"),
            atom("alias", &misleading),
        ],
    );
    let expression = compound(
        "add",
        [
            atom("operation", &operation),
            atom("algebra", &algebra),
            atom("const-name", &nominal(4, "Count")),
            atom("integer", "1"),
            atom("integer", "2"),
        ],
    );
    assert_eq!(
        owners(&compound("index-expression", [expression])),
        [[1; 32], [2; 32], [3; 32], [4; 32]]
    );
    for tag in [
        "add",
        "and",
        "bitwise-and",
        "bitwise-or",
        "bitwise-xor",
        "divide",
        "equal",
        "greater",
        "greater-or-equal",
        "less",
        "less-or-equal",
        "modulo",
        "multiply",
        "not-equal",
        "or",
        "shift-left",
        "shift-right",
        "subtract",
    ] {
        let expression = compound(
            tag,
            [atom("const-name", &nominal(7, "N")), atom("integer", "0")],
        );
        assert_eq!(
            owners(&compound("index-expression", [expression])),
            [[7; 32]]
        );
    }
}

#[test]
fn escaped_path_value_and_string_payloads_never_become_package_references() {
    let misleading = nominal(9, "NotASelectedPackage");
    assert_eq!(owners(&nominal(1, &misleading)), [[1; 32]]);
    assert_eq!(
        owners(&compound(
            "index-expression",
            [byte_atom("string", misleading.as_bytes())]
        )),
        Vec::<[u8; 32]>::new()
    );
    let value = compound(
        "canonical-const",
        [atom("type", &misleading), atom("encoding", &misleading)],
    );
    assert_eq!(owners(&compound("named", [value])), Vec::<[u8; 32]>::new());
    assert_eq!(
        owners(&compound(
            "constrained",
            [
                "unit".into(),
                compound("named-constraint", [atom("name", &misleading)])
            ]
        )),
        Vec::<[u8; 32]>::new()
    );
    let toolchain = compound(
        "nominal",
        [
            byte_atom("toolchain-source-owner", &[9; 32]),
            atom("path", &misleading),
        ],
    );
    assert_eq!(owners(&toolchain), Vec::<[u8; 32]>::new());
}

#[test]
fn embedded_binder_callbacks_share_nodes_depth_and_precharged_storage() {
    let value = compound(
        "named",
        [atom("name", &format!("test-runtime:{}", named(7)))],
    );
    let mut visitor = Visitor::default();
    visit_type_identity_package_owners(&value, &mut visitor).unwrap();
    assert_eq!(visitor.owners, [[7; 32]]);
    assert!(visitor.owned > 0);
    assert_eq!(visitor.depth, 0);
    let mut exact = Visitor {
        maximum_owned: visitor.owned,
        maximum_nodes: visitor.nodes,
        ..Visitor::default()
    };
    visit_type_identity_package_owners(&value, &mut exact).unwrap();
    for mut restricted in [
        Visitor {
            maximum_owned: visitor.owned - 1,
            ..Visitor::default()
        },
        Visitor {
            maximum_nodes: visitor.nodes - 1,
            ..Visitor::default()
        },
        Visitor {
            maximum_depth: 2,
            ..Visitor::default()
        },
    ] {
        assert_eq!(
            visit_type_identity_package_owners(&value, &mut restricted),
            Err(Error::ResourceLimitExceeded)
        );
        assert_eq!(restricted.depth, 0);
    }
    let mut borrowed = Visitor {
        maximum_owned: 0,
        ..Visitor::default()
    };
    visit_type_identity_package_owners(&nominal(7, "Plain"), &mut borrowed).unwrap();
    assert_eq!(borrowed.owned, 0);
}

#[test]
fn malformed_unknown_deep_and_absent_owner_inputs_fail_closed() {
    let bad_owner = format!("nominal(package-owner(32:{}),path(Value))", "0".repeat(64));
    for malformed in [
        "",
        "unit,unit",
        "ref(unit,unit)",
        "array(unit)",
        "named(name(a\\q))",
        "named(name(a(b)))",
        "named(name(a,b))",
        "mystery(unit)",
        "nominal(package-owner(1:01),path(Value))",
        "nominal(package-owner(032:01),path(Value))",
        "index-expression(string(1:FF))",
        "index-expression(string(2:00))",
        "nominal(unresolved-owner,path(Value))",
        "nominal(toolchain-owner,path(Value))",
        &bad_owner,
    ] {
        let mut visitor = Visitor::default();
        assert!(
            visit_type_identity_package_owners(malformed, &mut visitor).is_err(),
            "{malformed}"
        );
        assert_eq!(visitor.depth, 0);
    }
    let deep = format!("{}unit{}", "ref(".repeat(1024), ")".repeat(1024));
    let mut visitor = Visitor {
        maximum_depth: 16,
        ..Visitor::default()
    };
    assert_eq!(
        visit_type_identity_package_owners(&deep, &mut visitor),
        Err(Error::ResourceLimitExceeded)
    );
    assert_eq!(visitor.depth, 0);
    let mut visitor = Visitor {
        accepted_owner: Some([2; 32]),
        ..Visitor::default()
    };
    assert_eq!(
        visit_type_identity_package_owners(&named(1), &mut visitor),
        Err(Error::UnknownPackage)
    );
    assert_eq!(visitor.depth, 0);
}
