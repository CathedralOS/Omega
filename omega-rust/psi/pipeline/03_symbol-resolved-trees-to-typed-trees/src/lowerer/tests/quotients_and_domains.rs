use crate::lowerer::lower_symbol_resolved_trees;

#[test]
fn sealed_quotient_request_rejects_conformance_shaped_proof_discovery() {
    let source = r#"
        data Representative { value: i32; }
        trait Respects {}
        RepresentativeRespect: Representative satisfies Respects {}
        machine representative(value: Representative) -> Representative { value }
        machine wrapper(value: Representative) -> Representative {
            Quotient::define<representative, RepresentativeRespect>(value)
        }
    "#;
    let diagnostic = crate::front_end::typed_program_result(source)
        .expect_err("a conformance must not stand in for an exact theorem machine");

    assert!(
        diagnostic
            .message
            .contains("must resolve exactly to one resultless theorem machine entry"),
        "unexpected diagnostic: {}",
        diagnostic.message
    );
}

#[test]
fn sealed_quotient_namespace_cannot_be_shadowed() {
    let source = r#"
        data Representative { value: i32; }
        trait Respects {}
        RepresentativeRespect: Representative satisfies Respects {}
        machine representative(value: Representative) -> Representative { value }

        data Quotient {}
        machine Quotient::lift(value: Representative) -> Representative { value }
        machine wrapper(value: Representative) -> Representative {
            Quotient::lift<representative, RepresentativeRespect>(value)
        }
    "#;
    let diagnostic = crate::front_end::typed_program_result(source)
        .expect_err("an authored Quotient namespace must not capture the sealed wrapper");

    assert!(
        diagnostic.message.contains("cannot be shadowed"),
        "unexpected diagnostic: {}",
        diagnostic.message
    );
}

#[test]
fn quotient_cannot_declare_structural_equatable_conformance() {
    let source = r#"
        data Carrier {}
        proposition equivalent(left: Carrier, right: Carrier);
        trait Equivalence<C, proposition Relation>
        where proposition Relation(left: C, right: C);
        {
        }
        CarrierEquivalence: satisfies Equivalence<Carrier, equivalent> {}
        data ExactQ = Carrier % equivalent
        where equivalent satisfies
            Equivalence<Carrier, equivalent>
            as CarrierEquivalence;
        ExactQEquatable: ExactQ satisfies Equatable;
    "#;
    let diagnostic = crate::front_end::typed_program_result(source)
        .expect_err("a quotient must not synthesize representative equality");

    assert!(
        diagnostic
            .message
            .contains("cannot synthesize equality for a quotient type"),
        "unexpected diagnostic: {}",
        diagnostic.message
    );
}

#[test]
fn quotient_cannot_choose_a_zero_value_representative() {
    let source = r#"
        data Carrier {}
        proposition equivalent(left: Carrier, right: Carrier);
        trait Equivalence<C, proposition Relation>
        where proposition Relation(left: C, right: C);
        {
        }
        CarrierEquivalence: satisfies Equivalence<Carrier, equivalent> {}
        data ExactQ = Carrier % equivalent
        where equivalent satisfies
            Equivalence<Carrier, equivalent>
            as CarrierEquivalence;

        machine impossible_default()
        ensures zero_value<ExactQ>() == zero_value<ExactQ>();
        {
        }
    "#;
    let diagnostic = crate::front_end::typed_program_result(source)
        .expect_err("a quotient must not expose a canonical zero representative");

    assert!(
        diagnostic
            .message
            .contains("cannot observe or choose a retained quotient representative"),
        "unexpected diagnostic: {}",
        diagnostic.message
    );
}

#[test]
fn quotient_field_cannot_enter_synthesized_container_equality() {
    let source = r#"
        data Carrier {}
        proposition equivalent(left: Carrier, right: Carrier);
        trait Equivalence<C, proposition Relation>
        where proposition Relation(left: C, right: C);
        {
        }
        CarrierEquivalence: satisfies Equivalence<Carrier, equivalent> {}
        data ExactQ = Carrier % equivalent
        where equivalent satisfies
            Equivalence<Carrier, equivalent>
            as CarrierEquivalence;

        data Wrapper { value: ExactQ; }
        WrapperEquatable: Wrapper satisfies Equatable;
    "#;
    let diagnostic = crate::front_end::typed_program_result(source)
        .expect_err("container synthesis must not compare a quotient representative field");

    assert!(
        diagnostic
            .message
            .contains("field `value` of `Wrapper` has quotient type `ExactQ`"),
        "unexpected diagnostic: {}",
        diagnostic.message
    );
}

#[test]
fn proposition_application_rejects_in_runtime_value_position() {
    let source = r#"
        proposition related(left: i32, right: i32);
        machine bad(value: i32) {
            related(value, value);
        }
    "#;
    let diagnostic = crate::front_end::typed_program_result(source)
        .expect_err("runtime proposition use must fail closed");

    assert!(diagnostic.message.contains("proof-only"));
}

#[test]
fn lowers_dungeon_style_machine_program() {
    let source = r#"
    data Inventory {
        gold: u32;
    }

    pub machine Inventory::clear(&mut self, inventory: &mut Inventory) {
        inventory.gold = 0;
    }
    "#;

    let typed_trees = crate::front_end::typed_program(source);

    assert_eq!(typed_trees.data_definitions().len(), 1);
    assert_eq!(typed_trees.machines().len(), 1);
    assert_eq!(
        typed_trees.machine_states(&typed_trees.machines()[0]).len(),
        1
    );
    assert!(
        typed_trees
            .symbols
            .find_child_by_name(typed_trees.symbols.root(), "u32")
            .is_some()
    );
}

#[test]
fn lowers_slice_range_surface_into_typed_trees() {
    let source = r#"
    data Main {}

    machine Main::main(&mut self) -> usize {
        let values: [usize; 4] = [1, 2, 3, 4];
        let view: &[usize] = values.as_slice();
        let tail: &[usize] = view[1..];
        tail.len
    }
    "#;

    let typed_trees = crate::front_end::typed_program(source);

    assert!(
        typed_trees
            .machines()
            .first()
            .is_some_and(|machine| !typed_trees.machine_states(machine).is_empty())
    );
}

#[test]
fn preserves_structural_recast_targets_through_typed_lowering() {
    let source = r#"
    machine inspect(bytes: [u8; 4]) {
        let fixed: &[u8; 4] = &bytes as &[u8; 4];
        let slice: &[u8] = &bytes as &[u8];
    }
    "#;

    let typed_trees = crate::front_end::typed_program(source);
    let machine = &typed_trees.machines()[0];
    let state = &typed_trees.machine_states(machine)[0];
    let locals = typed_trees
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .filter_map(|statement| match statement {
            typed_trees::statement::StatementNode::LocalData(local) => Some(local),
            _ => None,
        })
        .collect::<Vec<_>>();

    let typed_trees::expression::ExpressionNode::Borrow(fixed_borrow) = typed_trees
        .expression_table
        .expression(locals[0].initial_value)
    else {
        panic!("fixed-array initializer should retain its shared borrow");
    };
    assert_eq!(
        fixed_borrow.access,
        language_semantics::ReferenceAccess::Shared
    );
    let typed_trees::expression::ExpressionNode::Cast(fixed) =
        typed_trees.expression_table.expression(fixed_borrow.target)
    else {
        panic!("fixed-array shared-borrow target should remain a cast");
    };
    assert!(matches!(
        typed_trees
            .type_reference_table
            .type_reference(fixed.target_type),
        typed_trees::types::TypeReferenceNode::FixedArray {
            length: typed_trees::types::FixedArrayLength::Literal(4),
            ..
        }
    ));

    let typed_trees::expression::ExpressionNode::Borrow(slice_borrow) = typed_trees
        .expression_table
        .expression(locals[1].initial_value)
    else {
        panic!("slice initializer should retain its shared borrow");
    };
    assert_eq!(
        slice_borrow.access,
        language_semantics::ReferenceAccess::Shared
    );
    let typed_trees::expression::ExpressionNode::Cast(slice) =
        typed_trees.expression_table.expression(slice_borrow.target)
    else {
        panic!("slice shared-borrow target should remain a cast");
    };
    assert!(matches!(
        typed_trees
            .type_reference_table
            .type_reference(slice.target_type),
        typed_trees::types::TypeReferenceNode::Slice { .. }
    ));
}

#[test]
fn retains_closed_compiler_domain_subjects_and_layout_schema_report_fingerprint() {
    use typed_trees::types::{
        DomainConstraintSubject, OmegaLayoutGrammar, TypeConstraintNode, TypeReferenceNode,
    };

    let source = r#"
    data Save {
        #1 value: u32;
    }

    data Holder {
        finite: f64 in Finite;
        carry: u64 in Carry::AnyCpu;
        layout: [u8; 32] in OmegaLayout<Save>;
    }
    "#;
    let typed = crate::front_end::typed_program(source);
    let holder = typed
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Holder")
        .expect("Holder");
    let fields = typed
        .data_members(holder)
        .iter()
        .filter_map(|member| match member {
            typed_trees::data::DataMember::Field(field) => {
                Some((field.name.as_str(), field.type_reference))
            }
            typed_trees::data::DataMember::Variant(_) => None,
        })
        .collect::<std::collections::HashMap<_, _>>();
    let domain_for = |type_reference| {
        let TypeReferenceNode::Constrained { constraints, .. } =
            typed.type_reference_table.type_reference(type_reference)
        else {
            panic!("constrained field")
        };
        let [TypeConstraintNode::Domain(domain)] =
            typed.type_reference_table.constraints(*constraints)
        else {
            panic!("one domain constraint")
        };
        domain
    };

    assert_eq!(
        domain_for(fields["finite"]).subject,
        DomainConstraintSubject::Value(language_semantics::value_domain::ValueDomain::Finite)
    );
    assert_eq!(
        domain_for(fields["carry"]).subject,
        DomainConstraintSubject::Carry(language_semantics::CarryPermission::AnyCpu)
    );
    let layout = domain_for(fields["layout"]);
    assert_eq!(
        layout.subject,
        DomainConstraintSubject::OmegaLayout {
            grammar: OmegaLayoutGrammar::Derived,
        }
    );
    let [schema] = layout.arguments.as_slice() else {
        panic!("one structural layout schema")
    };
    let save = typed
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Save")
        .expect("Save");
    assert!(matches!(
        typed.type_reference_table.type_reference(*schema),
        TypeReferenceNode::Named { symbol, name }
            if *symbol == save.symbol && name.as_str() == "Save"
    ));

    let finite_identity = typed.normalized_type_identity(fields["finite"]);
    assert!(finite_identity.as_str().contains("compiler-domain"));
    assert!(finite_identity.as_str().contains("finite"));
    assert!(!finite_identity.as_str().contains("Finite"));
    let carry_identity = typed.normalized_type_identity(fields["carry"]);
    assert!(carry_identity.as_str().contains("any-cpu"));
    assert!(!carry_identity.as_str().contains("Carry::AnyCpu"));
    let layout_identity = typed.normalized_type_identity(fields["layout"]);
    assert!(layout_identity.as_str().contains("omega-layout"));
    assert!(layout_identity.as_str().contains("derived"));
    assert!(layout_identity.as_str().contains("Save"));
    assert!(!layout_identity.as_str().contains("OmegaLayout"));
}

#[test]
fn carry_alias_expansion_retains_closed_invalid_symbol_atoms() {
    let source = r#"
    data Token {}
    data Holder {
        value: Token in Token::Portable;
    }
    domain Token::Portable = Carry::Portable;
    "#;
    let typed = crate::front_end::typed_program(source);
    let holder = typed
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Holder")
        .expect("Holder");
    let field = typed
        .data_members(holder)
        .iter()
        .find_map(|member| match member {
            typed_trees::data::DataMember::Field(field) => Some(field),
            typed_trees::data::DataMember::Variant(_) => None,
        })
        .expect("value field");
    let typed_trees::types::TypeReferenceNode::Constrained { constraints, .. } = typed
        .type_reference_table
        .type_reference(field.type_reference)
    else {
        panic!("constrained field")
    };
    let subjects = typed
        .type_reference_table
        .constraints(*constraints)
        .iter()
        .map(|constraint| match constraint {
            typed_trees::types::TypeConstraintNode::Domain(domain) => {
                assert!(!domain.symbol.is_valid());
                domain.subject
            }
            _ => panic!("carry alias must expand only to domain constraints"),
        })
        .collect::<Vec<_>>();

    assert_eq!(
        subjects,
        language_semantics::CarryPermission::ALL
            .map(typed_trees::types::DomainConstraintSubject::Carry)
    );
}

#[test]
fn expands_transparent_domain_aliases_before_semantic_normalization() {
    let source = r#"
    data Socket {
        connected: bool;
        authenticated: bool;
    }

    domain Socket::Connected
    requires
        self.connected;
    domain Socket::Authenticated
    requires
        self.authenticated;
    domain Socket::Usable =
        Socket::Connected & Socket::Authenticated;
    domain Socket::Ready = Socket::Usable;
    domain Socket::Prepared
    requires
        self in Socket::Ready;

    data Holder {
        aliased: Socket in Usable;
        expanded: Socket in Connected & Authenticated;
    }

    machine is_usable(socket: Socket) -> bool {
        socket in Socket::Usable
    }
    "#;

    let typed = crate::front_end::typed_program(source);

    let symbol_named = |name: &str| {
        typed
            .domain_definitions()
            .iter()
            .find(|domain| domain.name.as_str() == name)
            .map(|domain| domain.symbol)
            .expect("declared domain")
    };
    let connected = symbol_named("Socket::Connected");
    let authenticated = symbol_named("Socket::Authenticated");
    let usable = typed
        .domain_definitions()
        .iter()
        .find(|domain| domain.name.as_str() == "Socket::Usable")
        .expect("retained alias declaration");
    assert_eq!(usable.alias.as_ref().expect("alias").constituents.len(), 2);

    let prepared = typed
        .domain_definitions()
        .iter()
        .find(|domain| domain.name.as_str() == "Socket::Prepared")
        .expect("prepared domain");
    let imported = typed
        .proof_facts(prepared)
        .iter()
        .map(|fact| match fact {
            typed_trees::domain::ProofFact::Membership(membership) => membership.domain_symbol,
            typed_trees::domain::ProofFact::Expression(_) => {
                panic!("alias should expand to membership atoms")
            }
            typed_trees::domain::ProofFact::Proposition(_) => {
                panic!("domain alias should not become a proposition application")
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(imported, [connected, authenticated]);

    let holder = typed
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Holder")
        .expect("Holder");
    let fields = typed
        .data_members(holder)
        .iter()
        .filter_map(|member| match member {
            typed_trees::data::DataMember::Field(field) => {
                Some((field.name.as_str(), field.type_reference))
            }
            typed_trees::data::DataMember::Variant(_) => None,
        })
        .collect::<std::collections::HashMap<_, _>>();
    assert_eq!(
        typed.normalized_type_identity(fields["aliased"]),
        typed.normalized_type_identity(fields["expanded"]),
        "alias and explicit conjunction must have one normalized identity"
    );

    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "is_usable")
        .expect("membership machine");
    let has_atomic_conjunction = typed
        .machine_states(machine)
        .iter()
        .flat_map(|state| typed.statement_table.statements(state.statement_nodes))
        .any(|statement| {
            let typed_trees::statement::StatementNode::Expression(expression) = statement else {
                return false;
            };
            matches!(
                typed.expression_table.expression(*expression),
                typed_trees::expression::ExpressionNode::Binary(binary)
                    if binary.operator
                        == typed_trees::expression::BinaryOperator::And
            )
        });
    assert!(
        has_atomic_conjunction,
        "executable alias membership must lower to an atomic conjunction"
    );
}

/// Lower two separately-packaged sources through one resolution and return
/// the resolved and typed programs with each file's source id.
fn lower_packaged_sources(
    sources: &[(&str, &str)],
) -> (
    symbol_resolved_trees::SymbolResolvedTrees,
    typed_trees::TypedTrees,
) {
    use std::path::PathBuf;

    let mut map = source::SourceMap::default();
    let mut texts = Vec::new();
    for &(package_root, text) in sources {
        let source_id = map
            .add_with_metadata(
                PathBuf::from(format!("{package_root}/main.omg")),
                text.to_owned(),
                PathBuf::from(package_root),
                None,
                source::SourceOrigin::User,
            )
            .source_id;
        texts.push((source_id, text));
    }
    let resolved = crate::front_end::resolved_program_from_merged_source_map(map, &texts);
    let typed = lower_symbol_resolved_trees(&resolved).expect("type packaged sources");
    (resolved, typed)
}

fn utf8_constraint_on_line(
    typed: &typed_trees::TypedTrees,
) -> typed_trees::types::DomainConstraint {
    let line = typed
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Line")
        .expect("Line data");
    let [typed_trees::data::DataMember::Field(field)] = typed.data_members(line) else {
        panic!("Line carries one field")
    };
    let typed_trees::types::TypeReferenceNode::Constrained { constraints, .. } = typed
        .type_reference_table
        .type_reference(field.type_reference)
    else {
        panic!("constrained field")
    };
    let [typed_trees::types::TypeConstraintNode::Domain(domain)] =
        typed.type_reference_table.constraints(*constraints)
    else {
        panic!("one domain constraint")
    };
    domain.clone()
}

#[test]
fn same_package_domain_wins_over_foreign_same_carrier_leaf() {
    // A package-private `domain [u8; 256]::Utf8` must satisfy `in Utf8` on a
    // `[u8; 256]` field even when a foreign public declaration carries the
    // identical carrier and leaf: the occurrence's own package owns the
    // contested pool. This is the dungeon/std `Utf8` collision.
    let (resolved, typed) = lower_packaged_sources(&[
        ("dependency", "pub domain [u8; 256]::Utf8;"),
        (
            "package",
            "domain [u8; 256]::Utf8; data Line { value: [u8; 256] in Utf8; }",
        ),
    ]);
    // Capacity-specialized declarations normalize their carrier to the
    // family's const binder, so both sources declare `[u8; N]::Utf8`; the
    // package's copy is identified by provenance, not spelling.
    let package_domain = typed
        .domain_definitions()
        .iter()
        .find(|domain| {
            domain.name.as_str() == "[u8; N]::Utf8"
                && resolved
                    .symbols
                    .symbol_provenance_source_span(domain.symbol)
                    .is_some_and(|span| {
                        resolved.symbols.source_file(span).is_some_and(|file| {
                            file.package_root == std::path::Path::new("package")
                        })
                    })
        })
        .expect("the package's own domain");

    let constraint = utf8_constraint_on_line(&typed);
    assert_eq!(
        constraint.symbol, package_domain.symbol,
        "the same-package declaration must satisfy the constraint"
    );
    assert!(
        constraint.authored_selection.is_none(),
        "a normalized constraint releases its authored-selection custody"
    );
}

#[test]
fn foreign_only_same_carrier_domains_stay_contested() {
    // With no local declaration, two foreign `pub` domains sharing carrier
    // and leaf must not be silently selected: the constraint keeps its
    // authored custody and no symbol so normalized-domain validation can
    // reject it honestly.
    let (_resolved, typed) = lower_packaged_sources(&[
        ("dependency-a", "pub domain [u8; 256]::Utf8;"),
        ("dependency-b", "pub domain [u8; 256]::Utf8;"),
        ("package", "data Line { value: [u8; 256] in Utf8; }"),
    ]);

    let constraint = utf8_constraint_on_line(&typed);
    assert!(
        !constraint.symbol.is_valid(),
        "two foreign declarations must not guess an identity"
    );
    assert!(
        constraint.authored_selection.is_some(),
        "a contested constraint retains its authored selection for diagnosis"
    );
}
