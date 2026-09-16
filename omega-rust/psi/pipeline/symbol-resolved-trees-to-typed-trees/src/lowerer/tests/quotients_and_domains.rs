use crate::lowerer::lower_symbol_resolved_trees;
use source_files_to_tokens::Lexer;
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
use tokens_to_syntax_trees::parse_syntax_trees;

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
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let diagnostic = lower_symbol_resolved_trees(&resolved)
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
fn sealed_quotient_define_requires_both_exact_static_identities() {
    let source = r#"
        data Representative { value: i32; }
        machine representative(value: Representative) -> Representative { value }
        machine wrapper(value: Representative) -> Representative {
            Quotient::define<representative>(value)
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let diagnostic = lower_symbol_resolved_trees(&resolved)
        .expect_err("define without an exact named conformance must reject");

    assert!(
        diagnostic
            .message
            .contains("requires exactly `F, Congruence`"),
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
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let diagnostic = lower_symbol_resolved_trees(&resolved)
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
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let diagnostic = lower_symbol_resolved_trees(&resolved)
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
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let diagnostic = lower_symbol_resolved_trees(&resolved)
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
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let diagnostic = lower_symbol_resolved_trees(&resolved)
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
fn runtime_quotient_equality_requires_a_named_lifted_operation() {
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

        machine compare(left: ExactQ, right: ExactQ) -> bool {
            left == right
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let diagnostic = lower_symbol_resolved_trees(&resolved)
        .expect_err("runtime quotient equality must not observe representatives");

    assert!(
        diagnostic
            .message
            .contains("retained representatives are opaque"),
        "unexpected diagnostic: {}",
        diagnostic.message
    );
}

#[test]
fn proof_position_quotient_equality_remains_for_congruence() {
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

        machine cite(left: ExactQ, right: ExactQ)
        requires left == right
        {
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    lower_symbol_resolved_trees(&resolved)
        .expect("logical quotient equality must remain available to congruence validation");
}

#[test]
fn proposition_application_rejects_in_runtime_value_position() {
    let source = r#"
        proposition related(left: i32, right: i32);
        machine bad(value: i32) {
            related(value, value);
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved_program =
        resolve(ResolutionRequest::new(&syntax_trees)).expect("resolution should succeed");
    let diagnostic = lower_symbol_resolved_trees(&resolved_program)
        .expect_err("runtime proposition use must fail closed");

    assert!(diagnostic.message.contains("proof-only"));
}

#[test]
fn transparent_proposition_alias_normalizes_to_its_expansion() {
    let source = r#"
        proposition related(left: i32, right: i32);
        proposition self_related(value: i32) = related(value, value);

        machine through_alias(value: i32)
        requires self_related(value)
        {
        }

        machine through_expansion(value: i32)
        requires related(value, value)
        {
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved_program =
        resolve(ResolutionRequest::new(&syntax_trees)).expect("resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved_program).expect("typing should succeed");

    let normalized = typed
        .machines()
        .iter()
        .map(|machine| {
            let [contract] = typed.machine_contracts(machine) else {
                panic!("machine should retain one contract");
            };
            let [typed_trees::domain::ProofFact::Proposition(application)] =
                typed.proof_facts.span_or_empty(contract.facts)
            else {
                panic!("contract should retain one proposition application");
            };
            typed
                .normalize_proposition_application(application, None)
                .expect("application should normalize")
                .identity_label()
        })
        .collect::<Vec<_>>();

    assert_eq!(normalized.len(), 2);
    assert_eq!(normalized[0], normalized[1]);
    assert!(!normalized[0].contains("self_related"));
    assert!(normalized[0].contains("related"));
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

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved_program =
        resolve(ResolutionRequest::new(&syntax_trees)).expect("resolution should succeed");
    let typed_trees =
        lower_symbol_resolved_trees(&resolved_program).expect("lowering should succeed");

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

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved_program =
        resolve(ResolutionRequest::new(&syntax_trees)).expect("resolution should succeed");
    let typed_trees =
        lower_symbol_resolved_trees(&resolved_program).expect("typed lowering should succeed");

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

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved_program =
        resolve(ResolutionRequest::new(&syntax_trees)).expect("resolution should succeed");
    let typed_trees =
        lower_symbol_resolved_trees(&resolved_program).expect("typed lowering should succeed");
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
fn lowers_domain_definitions() {
    let source = r#"
    domain Player::Valid
    requires
        self.health >= 0

    domain Player::Alive
    requires
        self in Player::Valid;
        self.health > 0

    domain Player::Tagged;
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved_program =
        resolve(ResolutionRequest::new(&syntax_trees)).expect("resolution should succeed");
    let typed_trees =
        lower_symbol_resolved_trees(&resolved_program).expect("lowering should succeed");

    assert_eq!(typed_trees.domain_definitions().len(), 3);
    let domain = typed_trees
        .domain_definitions()
        .iter()
        .find(|domain| domain.name.as_str() == "Player::Alive")
        .expect("alive domain should lower");
    assert!(domain.symbol.is_valid());
    assert_eq!(domain.name.as_str(), "Player::Alive");
    let facts = typed_trees.proof_facts(domain);
    assert_eq!(facts.len(), 2);
    let typed_trees::domain::ProofFact::Membership(membership) = &facts[0] else {
        panic!("first domain fact should be membership")
    };
    assert!(membership.domain_symbol.is_valid());
    assert!(domain.semantic_clause_token_count >= 3);
    assert_eq!(
        domain.predicate_body,
        language_semantics::DomainPredicateBody::Present
    );
    assert!(domain.target_type.is_valid());
    let resolved_domain = resolved_program
        .domain_definitions
        .iter()
        .find(|candidate| candidate.name.as_str() == "Player::Alive")
        .expect("resolved alive domain");
    assert_eq!(domain.semantic_roles, resolved_domain.semantic_roles);
    assert_eq!(
        domain.establishment_routes,
        resolved_domain.establishment_routes
    );
    assert!(domain.semantic_roles.is_empty());
    let tagged = typed_trees
        .domain_definitions()
        .iter()
        .find(|candidate| candidate.name.as_str() == "Player::Tagged")
        .expect("typed tagged domain");
    assert_eq!(
        tagged.predicate_body,
        language_semantics::DomainPredicateBody::Bodyless
    );
    assert!(tagged.semantic_roles.is_empty());
}

#[test]
fn lowers_case_union_domain_proofs_from_exact_resolved_symbols() {
    let source = r#"
    data Command {
        case Move(dx: i32);
        case Say(volume: i32);
    }

    domain Command::Interactive
    requires
        self in Command::Move | Command::Say;
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved_program =
        resolve(ResolutionRequest::new(&syntax_trees)).expect("resolution should succeed");
    let expected_symbols = resolved_program
        .data_definitions
        .iter()
        .find(|definition| definition.name.as_str() == "Command")
        .map(|definition| {
            let cases = resolved_program
                .data_members(definition.members)
                .iter()
                .filter_map(|member| match member {
                    symbol_resolved_trees::data::DataMember::Variant(variant) => {
                        Some(variant.symbol)
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            (definition.symbol, cases)
        })
        .expect("Command data");
    let typed_trees =
        lower_symbol_resolved_trees(&resolved_program).expect("case-union proof should lower");
    let domain = typed_trees
        .domain_definitions()
        .iter()
        .find(|domain| domain.name.as_str() == "Command::Interactive")
        .expect("interactive domain");
    let [typed_trees::domain::ProofFact::Expression(expression)] = typed_trees.proof_facts(domain)
    else {
        panic!("case union should lower as one proof expression");
    };
    let typed_trees::expression::ExpressionNode::Binary(union) =
        typed_trees.expression_table.expression(*expression)
    else {
        panic!("proof expression should remain a union");
    };

    for (expression, expected_case) in [union.left, union.right]
        .into_iter()
        .zip(expected_symbols.1)
    {
        let typed_trees::expression::ExpressionNode::Binary(equality) =
            typed_trees.expression_table.expression(expression)
        else {
            panic!("case membership should lower to exact tag equality");
        };
        let typed_trees::expression::ExpressionNode::Name(case) =
            typed_trees.expression_table.expression(equality.right)
        else {
            panic!("tag equality should retain an exact case path");
        };
        assert_eq!(case.head_symbol, expected_symbols.0);
        assert_eq!(case.symbol, expected_case);
        assert_eq!(
            typed_trees
                .expression_table
                .name_path_member_symbols(case.member_symbols),
            [expected_symbols.0, expected_case]
        );
    }
}

#[test]
fn normalizes_domain_constraints_by_short_name_and_carrier() {
    let source = r#"
    data Box<T> {
        value: T;
    }

    data Holder {
        signed: i64 in Tagged;
        unsigned: u64 in Tagged;
        boxed_signed: Box<i64 in Tagged>;
    }

    domain i64::Tagged;

    domain u64::Tagged
    requires
        self >= 0;
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax_trees)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");

    let signed_domain = typed
        .domain_definitions()
        .iter()
        .find(|domain| domain.name.as_str() == "i64::Tagged")
        .expect("signed domain");
    let unsigned_domain = typed
        .domain_definitions()
        .iter()
        .find(|domain| domain.name.as_str() == "u64::Tagged")
        .expect("unsigned domain");
    assert_eq!(
        signed_domain.predicate_body,
        language_semantics::DomainPredicateBody::Bodyless
    );
    assert_eq!(
        unsigned_domain.predicate_body,
        language_semantics::DomainPredicateBody::Present
    );

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

    let constraint_for = |type_reference| {
        let typed_trees::types::TypeReferenceNode::Constrained { constraints, .. } =
            typed.type_reference_table.type_reference(type_reference)
        else {
            panic!("constrained field")
        };
        let [typed_trees::types::TypeConstraintNode::Domain(domain)] =
            typed.type_reference_table.constraints(*constraints)
        else {
            panic!("one domain constraint")
        };
        domain
    };

    let signed = constraint_for(fields["signed"]);
    assert_eq!(signed.symbol, signed_domain.symbol);
    assert_eq!(signed.semantic_id, signed_domain.semantic_id);
    assert_eq!(signed.predicate_body, signed_domain.predicate_body);
    assert_eq!(signed.semantic_roles, signed_domain.semantic_roles);
    assert_eq!(
        signed.establishment_routes,
        signed_domain.establishment_routes
    );

    let unsigned = constraint_for(fields["unsigned"]);
    assert_eq!(unsigned.symbol, unsigned_domain.symbol);
    assert_eq!(unsigned.semantic_id, unsigned_domain.semantic_id);
    assert_eq!(unsigned.predicate_body, unsigned_domain.predicate_body);
    assert_eq!(unsigned.semantic_roles, unsigned_domain.semantic_roles);
    assert_eq!(
        unsigned.establishment_routes,
        unsigned_domain.establishment_routes
    );

    let typed_trees::types::TypeReferenceNode::Generic { arguments, .. } = typed
        .type_reference_table
        .type_reference(fields["boxed_signed"])
    else {
        panic!("generic field")
    };
    let [argument] = typed
        .type_reference_table
        .type_reference_handles(*arguments)
    else {
        panic!("one generic argument")
    };
    let boxed_signed = constraint_for(*argument);
    assert_eq!(boxed_signed.symbol, signed_domain.symbol);
    assert_eq!(boxed_signed.predicate_body, signed_domain.predicate_body);
    assert_eq!(boxed_signed.semantic_roles, signed_domain.semantic_roles);
    assert_eq!(
        boxed_signed.establishment_routes,
        signed_domain.establishment_routes
    );
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
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax_trees)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
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
fn symbol_backed_domain_spelling_cannot_spoof_compiler_subject() {
    let source = r#"
    data Holder {
        value: f64 in Finite;
    }

    domain f64::Finite;
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax_trees)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
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
    let [typed_trees::types::TypeConstraintNode::Domain(domain)] =
        typed.type_reference_table.constraints(*constraints)
    else {
        panic!("one domain constraint")
    };

    assert!(domain.symbol.is_valid());
    assert_eq!(
        domain.subject,
        typed_trees::types::DomainConstraintSubject::Declared
    );
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
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax_trees)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
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

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax_trees)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");

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

#[test]
fn parameter_domain_conjunction_synthesizes_each_membership_contract() {
    let source = r#"
    domain [u8]::Meaning;
    domain [u8]::Utf8
    requires
        valid_utf8(self);
    domain [u8]::NoNul
    requires
        no_nul(self);

    machine inspect(bytes: &[u8] in Meaning & Utf8 & NoNul) {
    }
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax_trees)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let machine = typed.machines().first().expect("inspect machine");
    let state = typed.machine_states(machine).first().expect("entry state");
    let names: Vec<_> = typed
        .state_contracts(state)
        .iter()
        .map(|contract| {
            let [typed_trees::domain::ProofFact::Membership(membership)] =
                typed.proof_facts.span_or_empty(contract.facts)
            else {
                panic!("one synthesized membership fact")
            };
            typed
                .domain_definitions()
                .iter()
                .find(|domain| domain.symbol == membership.domain_symbol)
                .expect("normalized declared domain")
                .name
                .as_str()
                .to_owned()
        })
        .collect();

    assert_eq!(
        names,
        ["[u8]::Meaning", "[u8]::Utf8", "[u8]::NoNul"],
        "bodyless and predicate-bearing constraints are all call-boundary obligations"
    );
}

#[test]
fn internal_state_domain_constraint_does_not_leak_to_machine_entry() {
    let source = r#"
    data Token {
        value: u64;
    }

    domain Token::Issued;

    machine carry(seed: u64) {
        transition { _ -> hold(Token { value: seed }) }

        state hold(token: Token in Issued) {
        }
    }
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax_trees)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let machine = typed.machines().first().expect("carry machine");
    assert!(
        typed.machine_contracts(machine).is_empty(),
        "an internal state's constraint must not become a machine-wide entry contract"
    );
    let [entry, hold] = typed.machine_states(machine) else {
        panic!("entry and hold states")
    };
    assert!(typed.state_contracts(entry).is_empty());
    assert_eq!(
        typed.state_contracts(hold).len(),
        1,
        "the constrained state retains its own implicit membership requirement"
    );
}

#[test]
fn preserves_operator_declarations() {
    let source = r#"
    operator Slice::index<T>(items: &[T], index: usize) -> T
    requires
        index < items.len;
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved_program =
        resolve(ResolutionRequest::new(&syntax_trees)).expect("resolution should succeed");
    let typed_trees =
        lower_symbol_resolved_trees(&resolved_program).expect("lowering should succeed");

    assert_eq!(typed_trees.operators().len(), 1);
    let operator = &typed_trees.operators()[0];
    assert_eq!(
        typed_trees
            .operator_path_members(operator.name)
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>(),
        ["Slice", "index"]
    );
    assert_eq!(
        typed_trees
            .data_type_parameters
            .span_or_empty(operator.type_parameters)
            .len(),
        1
    );
    assert_eq!(
        typed_trees
            .state_parameters
            .span_or_empty(operator.parameters)
            .len(),
        2
    );
    assert!(operator.symbol.is_valid());
    assert!(operator.return_type.is_valid());
    assert_eq!(
        typed_trees
            .signature_contracts
            .span_or_empty(operator.contracts)
            .len(),
        1
    );
    assert!(operator.token_count > 0);
}

#[test]
fn preserves_domain_operator_declarations() {
    let source = r#"
    data Quantity {
        value: i32;
    }

    domain Quantity::Additive
    requires
        self.value >= 0;

    operator Quantity::Additive::add(left: Quantity, right: Quantity) -> Quantity;
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved_program =
        resolve(ResolutionRequest::new(&syntax_trees)).expect("resolution should succeed");
    let typed_trees =
        lower_symbol_resolved_trees(&resolved_program).expect("lowering should succeed");
    let domain = typed_trees
        .domain_definitions()
        .iter()
        .find(|domain| domain.name.as_str() == "Quantity::Additive")
        .expect("domain should lower");
    let operators = typed_trees.domain_operators(domain);

    assert_eq!(operators.len(), 1);
    assert_eq!(
        domain.semantic_roles.denotation_dimension,
        Some(domain.semantic_id)
    );
    assert!(domain.semantic_roles.arithmetic_policy.is_none());
    assert!(operators[0].symbol.is_valid());
    assert_eq!(
        typed_trees
            .operator_path_members(operators[0].name)
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>(),
        ["add"]
    );
    assert_eq!(typed_trees.proof_facts(domain).len(), 1);
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
    use std::sync::Arc;
    use tokens_to_syntax_trees::parse_syntax_trees_with_id;

    let mut map = source::SourceMap::default();
    let mut forests = Vec::new();
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
        let tokens = Lexer::new(text).tokenize().expect("tokenize source");
        forests.push(parse_syntax_trees_with_id(source_id, &tokens).expect("parse source"));
    }
    let mut syntax = forests.remove(0);
    for forest in &forests {
        syntax.extend_from(forest);
    }
    let resolved = resolve(ResolutionRequest {
        syntax: &syntax,
        sources: Some(Arc::new(map)),
        top_level_bindings: Vec::new(),
    })
    .expect("resolve packaged sources");
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
