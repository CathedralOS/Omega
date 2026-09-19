//! Type identity tests: normalized identities, contexts, open index and constraint normalization.

use crate::type_identity::{ExactOwnerTypeIdentityRequest, TypeIdentityRequest};
use std::path::PathBuf;
use std::sync::Arc;

use super::{NormalizedDomainTerm, NormalizedTypeIdentity};
use crate::TypedTrees;
use crate::data::DataDefinition;
use crate::domain::{DomainAliasConstituent, DomainAliasDefinition, DomainDefinition};
use crate::expression::{
    BinaryExpression, BinaryOperator, Expression, ExpressionHandle, ExpressionNode, NamePath,
};
use crate::name::Identifier;
use crate::typed_trees::type_system::type_identity::identity_context::PackageQualifiedNominalOwner;
use crate::typed_trees::type_system::type_identity::identity_context::package_qualified_nominal_name;
use crate::typed_trees::{OpenIndexNormalization, OpenIndexOperationSelection};
use crate::types::{DomainConstraint, TypeConstraintNode, TypeReferenceNode};
use language_core::operator_spelling::OperatorSpelling;
use language_semantics::{
    DomainEstablishmentRoute, DomainPredicateBody, DomainSemanticRoles, ReferenceAccess,
    SemanticDomainId,
};
use source::{SourceMap, SourceOrigin, SourceSpan, Span};
use symbols::{SymbolHandle, SymbolKind, SymbolNameRef, SymbolTableBuilder, builtin_type_symbols};

#[test]
fn symbolic_range_endpoints_preserve_owner_slots_and_substitution() {
    let mut builder = SymbolTableBuilder::new();
    let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
    let owners = SymbolTableBuilder::child_handles(builder.insert_children(
        root,
        [
            (SymbolKind::Module, SymbolNameRef::Static("First")),
            (SymbolKind::Module, SymbolNameRef::Static("Second")),
        ],
    ))
    .collect::<Vec<_>>();
    let first = SymbolTableBuilder::child_handles(
        builder.insert_children(owners[0], [(SymbolKind::Data, SymbolNameRef::Static("N"))]),
    )
    .next()
    .expect("first endpoint owner");
    let second = SymbolTableBuilder::child_handles(
        builder.insert_children(owners[1], [(SymbolKind::Data, SymbolNameRef::Static("N"))]),
    )
    .next()
    .expect("second endpoint owner");
    let mut program = TypedTrees {
        symbols: builder.finish(),
        ..TypedTrees::default()
    };
    let first_expression =
        program
            .expression_table
            .insert_tree(&Expression::Name(NamePath::resolved(
                vec![Identifier::generated("N")],
                first,
                first,
            )));
    let second_expression =
        program
            .expression_table
            .insert_tree(&Expression::Name(NamePath::resolved(
                vec![Identifier::generated("N")],
                second,
                second,
            )));
    let second_reference = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: second,
            name: Identifier::generated("N"),
        });
    for qualification in [
        crate::typed_trees::type_system::type_identity::identity_context::TypeIdentityQualification::Ordinary,
        crate::typed_trees::type_system::type_identity::identity_context::TypeIdentityQualification::PackageQualified,
    ] {
        let context = crate::typed_trees::type_system::type_identity::identity_context::TypeIdentityContext {
            binders: &[],
            substitutions: &[],
            active_const_substitutions: &[],
            exact_toolchain_sources: &[],
            missing_exact_nominal_owner: None,
            qualification,
        };
        let identity = |expression, context: &crate::typed_trees::type_system::type_identity::identity_context::TypeIdentityContext<'_>| {
            crate::typed_trees::type_system::type_identity::constraint_identity::normalized_range_endpoint(&program, expression, false, context)
        };
        assert_ne!(
            identity(first_expression, &context),
            identity(second_expression, &context),
            "equal diagnostic names cannot merge distinct declaration owners"
        );
        let first_slot = [(first, "$C0".to_owned())];
        let second_slot = [(second, "$C0".to_owned())];
        let another_slot = [(second, "$C1".to_owned())];
        assert_eq!(
            identity(
                first_expression,
                &crate::typed_trees::type_system::type_identity::identity_context::TypeIdentityContext {
                    binders: &first_slot,
                    ..context
                }
            ),
            identity(
                second_expression,
                &crate::typed_trees::type_system::type_identity::identity_context::TypeIdentityContext {
                    binders: &second_slot,
                    ..context
                }
            ),
            "corresponding binder slots remain alpha-equivalent"
        );
        assert_ne!(
            identity(
                first_expression,
                &crate::typed_trees::type_system::type_identity::identity_context::TypeIdentityContext {
                    binders: &first_slot,
                    ..context
                }
            ),
            identity(
                second_expression,
                &crate::typed_trees::type_system::type_identity::identity_context::TypeIdentityContext {
                    binders: &another_slot,
                    ..context
                }
            )
        );
        let replacements = [(first, second_reference)];
        let substituted = crate::typed_trees::type_system::type_identity::identity_context::TypeIdentityContext {
            substitutions: &replacements,
            ..context
        };
        assert_eq!(
            identity(first_expression, &substituted),
            identity(second_expression, &context)
        );
        assert_eq!(
            identity(second_expression, &substituted),
            identity(second_expression, &context),
            "substitution selects a symbol, not its same-spelled sibling"
        );
        assert_ne!(
            identity(first_expression, &context),
            crate::typed_trees::type_system::type_identity::constraint_identity::normalized_range_endpoint(&program, first_expression, true, &context),
            "an open exclusive endpoint retains its end-kind"
        );
    }
}

#[test]
fn range_endpoint_substitution_normalizes_values_without_erasing_typed_failure() {
    use numerics::{
        arithmetic::ArithmeticDomain,
        literals::{IntegerLanding, IntegerLiteral, LandedIntegerType},
    };
    let mut builder = SymbolTableBuilder::new();
    let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
    let builtins =
        SymbolTableBuilder::child_handles(builder.insert_children(root, builtin_type_symbols()))
            .collect::<Vec<_>>();
    let mut program = TypedTrees {
        symbols: builder.finish(),
        ..TypedTrees::default()
    };
    program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: builtins[symbols::BuiltinTypeAtom::U8.ordinal()],
            name: Identifier::generated("u8"),
        });
    let binder = SymbolHandle::from_arena_index(501);
    let endpoint = program
        .expression_table
        .insert_tree(&Expression::Name(NamePath::resolved(
            vec![Identifier::generated("N")],
            binder,
            binder,
        )));
    let seven = program
        .expression_table
        .insert(ExpressionNode::Integer(IntegerLiteral::from_value(7)));
    let eight = program
        .expression_table
        .insert(ExpressionNode::Integer(IntegerLiteral::from_value(8)));
    let eight_expression = program
        .type_reference_table
        .insert(TypeReferenceNode::ConstExpression(eight));
    let eight_atom = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: SymbolHandle::invalid(),
            name: Identifier::generated("8"),
        });
    let typed = program.expression_table.insert(ExpressionNode::Integer(
        IntegerLiteral::from_value(255).with_landing(IntegerLanding {
            landed_type: LandedIntegerType::U8,
            domain: ArithmeticDomain::Exact,
        }),
    ));
    let one = program
        .expression_table
        .insert(ExpressionNode::Integer(IntegerLiteral::from_value(1)));
    let overflow = program.expression_table.insert(ExpressionNode::Binary(
        crate::expression::TableBinaryExpression {
            left: typed,
            operator: BinaryOperator::Add,
            right: one,
        },
    ));
    let invalid_literal = program.expression_table.insert(ExpressionNode::Integer(
        IntegerLiteral::from_value(256).with_landing(IntegerLanding {
            landed_type: LandedIntegerType::U8,
            domain: ArithmeticDomain::Exact,
        }),
    ));
    let mathematical_predecessor = program
        .expression_table
        .insert(ExpressionNode::Integer(IntegerLiteral::from_value(255)));
    let invalid_references = [overflow, invalid_literal].map(|expression| {
        program
            .type_reference_table
            .insert(TypeReferenceNode::ConstExpression(expression))
    });
    for qualification in [
        crate::typed_trees::type_system::type_identity::identity_context::TypeIdentityQualification::Ordinary,
        crate::typed_trees::type_system::type_identity::identity_context::TypeIdentityQualification::PackageQualified,
    ] {
        let context = crate::typed_trees::type_system::type_identity::identity_context::TypeIdentityContext {
            binders: &[],
            substitutions: &[],
            active_const_substitutions: &[],
            exact_toolchain_sources: &[],
            missing_exact_nominal_owner: None,
            qualification,
        };
        for replacement in [eight_expression, eight_atom] {
            let substitutions = [(binder, replacement)];
            let substituted = crate::typed_trees::type_system::type_identity::identity_context::TypeIdentityContext {
                substitutions: &substitutions,
                ..context
            };
            assert_eq!(
                crate::typed_trees::type_system::type_identity::constraint_identity::normalized_range_endpoint(&program, endpoint, false, &substituted),
                crate::typed_trees::type_system::type_identity::constraint_identity::normalized_range_endpoint(&program, seven, true, &context),
                "direct binder substitution must normalize before inclusive identity"
            );
        }
        for (expression, replacement) in [overflow, invalid_literal]
            .into_iter()
            .zip(invalid_references)
        {
            assert!(
                program
                    .closed_integer_expression_value(expression)
                    .is_none()
            );
            let substitutions = [(binder, replacement)];
            let substituted = crate::typed_trees::type_system::type_identity::identity_context::TypeIdentityContext {
                substitutions: &substitutions,
                ..context
            };
            let actual = crate::typed_trees::type_system::type_identity::constraint_identity::normalized_range_endpoint(&program, endpoint, false, &substituted);
            assert_eq!(
                actual,
                crate::typed_trees::type_system::type_identity::constraint_identity::normalized_range_endpoint(&program, expression, false, &context)
            );
            assert_ne!(
                actual,
                crate::typed_trees::type_system::type_identity::constraint_identity::normalized_range_endpoint(
                    &program,
                    mathematical_predecessor,
                    true,
                    &context
                ),
                "substitution cannot repair an invalid typed endpoint"
            );
        }
    }
}
#[test]
fn closed_range_endpoints_keep_canonical_numeric_formatting() {
    let mut program = TypedTrees::default();
    let eight = program.expression_table.insert(ExpressionNode::Integer(
        numerics::literals::IntegerLiteral::from_value(8),
    ));
    let seven = program.expression_table.insert(ExpressionNode::Integer(
        numerics::literals::IntegerLiteral::from_value(7),
    ));
    for qualification in [
        crate::typed_trees::type_system::type_identity::identity_context::TypeIdentityQualification::Ordinary,
        crate::typed_trees::type_system::type_identity::identity_context::TypeIdentityQualification::PackageQualified,
    ] {
        let context = crate::typed_trees::type_system::type_identity::identity_context::TypeIdentityContext {
            binders: &[],
            substitutions: &[],
            active_const_substitutions: &[],
            exact_toolchain_sources: &[],
            missing_exact_nominal_owner: None,
            qualification,
        };
        assert_eq!(
            crate::typed_trees::type_system::type_identity::constraint_identity::normalized_range_endpoint(&program, eight, false, &context),
            crate::typed_trees::type_system::type_identity::constraint_identity::normalized_range_endpoint(&program, seven, true, &context)
        );
    }
}
#[test]
fn generated_concrete_generic_nominal_normalizes_to_its_exact_origin() {
    let mut program = TypedTrees::default();
    let generic_symbol = SymbolHandle::from_arena_index(91);
    let concrete_symbol = SymbolHandle::from_arena_index(92);
    let element = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: SymbolHandle::invalid(),
            name: Identifier::generated("i32"),
        });
    let arguments = program
        .type_reference_table
        .insert_type_reference_handles([element]);
    let generic_instance = program
        .type_reference_table
        .insert(TypeReferenceNode::Generic {
            base_symbol: generic_symbol,
            base_name: Identifier::generated("Buffer"),
            lifetime_arguments: Vec::new(),
            arguments,
        });
    let concrete = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: concrete_symbol,
            name: Identifier::generated("Buffer<i32>"),
        });
    program.push_data_definition(DataDefinition {
        symbol: concrete_symbol,
        name: Identifier::generated("Buffer<i32>"),
        generic_instance: Some(generic_instance),
        ..DataDefinition::default()
    });

    assert_eq!(
        program.normalized_type_identity(concrete),
        program.normalized_type_identity(generic_instance),
    );
    assert_eq!(
        program.package_qualified_type_identity(concrete),
        program.package_qualified_type_identity(generic_instance),
    );
}

#[test]
fn same_spelled_package_nominals_have_distinct_qualified_identities() {
    let path = "shared::Packet";
    let first =
        package_qualified_nominal_name(PackageQualifiedNominalOwner::Package([0x11; 32]), path);
    let second =
        package_qualified_nominal_name(PackageQualifiedNominalOwner::Package([0x22; 32]), path);

    assert_ne!(first, second);
    assert!(first.contains(path));
    assert!(second.contains(path));
    assert!(first.contains(&"11".repeat(32)), "{first}");
    assert!(second.contains(&"22".repeat(32)), "{second}");
}

#[test]
fn package_qualified_binder_identity_is_alpha_normalized_without_an_owner() {
    let mut program = TypedTrees::default();
    let first_symbol = SymbolHandle::from_arena_index(81);
    let second_symbol = SymbolHandle::from_arena_index(82);
    let first = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: first_symbol,
            name: Identifier::generated("Element"),
        });
    let second = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: second_symbol,
            name: Identifier::generated("RenamedElement"),
        });

    let first = program.type_identity(TypeIdentityRequest {
        binders: &[(first_symbol, "$T0".to_owned())],
        ..TypeIdentityRequest::package_qualified(first)
    });
    let second = program.type_identity(TypeIdentityRequest {
        binders: &[(second_symbol, "$T0".to_owned())],
        ..TypeIdentityRequest::package_qualified(second)
    });

    assert_eq!(first, second);
    assert_eq!(first.as_str(), "named(name($T0))");
    assert!(!first.as_str().contains("owner"));
}

#[test]
fn package_qualified_const_values_use_closed_semantics_without_display_text() {
    let mut program = TypedTrees::default();
    let first = language_semantics::const_value::CanonicalConstValue::new(
        "Unit",
        "record4:Unit4:code9:integer3:u321:7",
        "Unit { code: 7 }",
    );
    let second = language_semantics::const_value::CanonicalConstValue::new(
        "Unit",
        first.encoding.clone(),
        "diagnostic text must not be identity",
    );
    let changed = language_semantics::const_value::CanonicalConstValue::new(
        "Unit",
        "record4:Unit4:code9:integer3:u321:8",
        "Unit { code: 8 }",
    );
    let first = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: SymbolHandle::invalid(),
            name: Identifier::generated(first.atom()),
        });
    let second = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: SymbolHandle::invalid(),
            name: Identifier::generated(second.atom()),
        });
    let integer = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: SymbolHandle::invalid(),
            name: Identifier::generated("007"),
        });
    let changed = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: SymbolHandle::invalid(),
            name: Identifier::generated(changed.atom()),
        });

    let first = program.package_qualified_type_identity(first);
    let second = program.package_qualified_type_identity(second);
    let integer = program.package_qualified_type_identity(integer);
    let changed = program.package_qualified_type_identity(changed);

    assert_eq!(first, second);
    assert_ne!(first, changed);
    assert!(first.as_str().contains("canonical-const"), "{first}");
    assert!(first.as_str().contains("encoding"), "{first}");
    assert!(!first.as_str().contains("Unit { code: 7 }"), "{first}");
    assert!(!first.as_str().contains("unresolved-owner"), "{first}");
    assert!(integer.as_str().contains("integer-const(7)"), "{integer}");
    assert!(!integer.as_str().contains("unresolved-owner"), "{integer}");
}

#[test]
fn package_qualified_nominals_mark_toolchain_and_unresolved_owners() {
    let mut sources = SourceMap::default();
    let source_id = sources
        .add_with_metadata(
            PathBuf::from("toolchain/types.omg"),
            String::from("Packet"),
            PathBuf::from("toolchain"),
            None,
            SourceOrigin::Toolchain,
        )
        .source_id;
    let mut symbols = SymbolTableBuilder::with_sources(Some(Arc::new(sources)));
    let root = symbols.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
    let packet = SymbolTableBuilder::child_handles(symbols.insert_children(
        root,
        [(
            SymbolKind::Data,
            SymbolNameRef::Source(SourceSpan::new(source_id, Span::new(0, 6))),
        )],
    ))
    .next()
    .expect("toolchain nominal symbol");
    let mut program = TypedTrees {
        symbols: symbols.finish(),
        ..TypedTrees::default()
    };
    let toolchain = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: packet,
            name: Identifier::generated("ignored-diagnostic-name"),
        });
    let unresolved = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: SymbolHandle::invalid(),
            name: Identifier::generated("Pending"),
        });

    let toolchain = program.package_qualified_type_identity(toolchain);
    let unresolved = program.package_qualified_type_identity(unresolved);
    assert!(
        toolchain.as_str().contains("toolchain-owner"),
        "{toolchain}"
    );
    assert!(toolchain.as_str().contains("Packet"), "{toolchain}");
    assert!(
        !toolchain.as_str().contains("ignored-diagnostic-name"),
        "{toolchain}"
    );
    assert!(
        unresolved.as_str().contains("unresolved-owner"),
        "{unresolved}"
    );
    assert!(unresolved.as_str().contains("Pending"), "{unresolved}");
}

#[test]
fn package_qualified_builtin_types_use_closed_atoms_without_name_spoofing() {
    let mut symbols = SymbolTableBuilder::new();
    let root = symbols.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
    let children = SymbolTableBuilder::child_handles(
        symbols.insert_children(
            root,
            builtin_type_symbols()
                .into_iter()
                .chain([(SymbolKind::Data, SymbolNameRef::Static("bool"))]),
        ),
    )
    .collect::<Vec<_>>();
    let builtins = &children[..symbols::BUILTIN_TYPE_COUNT];
    let lookalike = children[symbols::BUILTIN_TYPE_COUNT];
    let mut symbols = symbols.finish();
    for atom in symbols::BuiltinTypeAtom::ALL {
        assert_eq!(
            symbols.builtin_type_atom(builtins[atom.ordinal()]),
            Some(atom),
        );
    }
    let generated =
        symbols.insert_generated_root_from(builtins[0], SymbolKind::Data, "bool$generated");
    let mut program = TypedTrees {
        symbols,
        ..TypedTrees::default()
    };
    let builtin = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: builtins[0],
            name: Identifier::generated("spoofed-diagnostic"),
        });
    let lookalike = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: lookalike,
            name: Identifier::generated("ignored"),
        });
    let generated = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: generated,
            name: Identifier::generated("ignored"),
        });

    let builtin = program.package_qualified_type_identity(builtin);
    let lookalike = program.package_qualified_type_identity(lookalike);
    let generated = program.package_qualified_type_identity(generated);

    assert!(builtin.as_str().contains("compiler-type"), "{builtin}");
    assert!(builtin.as_str().contains("bool"), "{builtin}");
    assert!(
        !builtin.as_str().contains("spoofed-diagnostic"),
        "{builtin}"
    );
    assert!(
        lookalike.as_str().contains("unresolved-owner"),
        "{lookalike}"
    );
    assert!(!lookalike.as_str().contains("compiler-type"), "{lookalike}");
    assert!(
        generated.as_str().contains("unresolved-owner"),
        "{generated}"
    );
    assert!(!generated.as_str().contains("compiler-type"), "{generated}");
}

#[test]
fn exact_toolchain_type_owners_follow_source_and_generated_provenance() {
    let mut sources = SourceMap::default();
    let source_id = sources
        .add_with_metadata(
            PathBuf::from("toolchain/std/types.omg"),
            String::from("Packet"),
            PathBuf::from("toolchain/std"),
            None,
            SourceOrigin::Toolchain,
        )
        .source_id;
    let mut builder = SymbolTableBuilder::with_sources(Some(Arc::new(sources)));
    let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
    let packet = SymbolTableBuilder::child_handles(builder.insert_children(
        root,
        [(
            SymbolKind::Data,
            SymbolNameRef::Source(SourceSpan::new(source_id, Span::new(0, 6))),
        )],
    ))
    .next()
    .expect("toolchain nominal symbol");
    let mut symbols = builder.finish();
    let generated =
        symbols.insert_generated_root_from(packet, SymbolKind::Data, "Packet$specialized");
    let mut program = TypedTrees {
        symbols,
        ..TypedTrees::default()
    };
    let packet_type = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: packet,
            name: Identifier::generated("ignored"),
        });
    let generated_type = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: generated,
            name: Identifier::generated("ignored"),
        });

    let first = program
        .exact_owner_type_identity(ExactOwnerTypeIdentityRequest {
            type_reference: packet_type,
            binders: &[],
            substitutions: &[],
            exact_toolchain_sources: &[(source_id, [0x33; 32])],
        })
        .expect("exact toolchain source owner");
    let generated = program
        .exact_owner_type_identity(ExactOwnerTypeIdentityRequest {
            type_reference: generated_type,
            binders: &[],
            substitutions: &[],
            exact_toolchain_sources: &[(source_id, [0x33; 32])],
        })
        .expect("generated exact toolchain source owner");
    let changed = program
        .exact_owner_type_identity(ExactOwnerTypeIdentityRequest {
            type_reference: packet_type,
            binders: &[],
            substitutions: &[],
            exact_toolchain_sources: &[(source_id, [0x44; 32])],
        })
        .expect("changed exact toolchain source owner");

    assert!(first.as_str().contains("toolchain-source-owner"), "{first}");
    assert!(first.as_str().contains(&"33".repeat(32)), "{first}");
    assert!(generated.as_str().contains(&"33".repeat(32)), "{generated}");
    assert!(generated.as_str().contains("Packet$specialized"));
    assert_ne!(first, changed);

    assert!(
        program
            .exact_owner_type_identity(ExactOwnerTypeIdentityRequest {
                type_reference: packet_type,
                binders: &[],
                substitutions: &[],
                exact_toolchain_sources: &[]
            })
            .is_none(),
        "exact package identity must fail closed when toolchain custody is missing"
    );

    let unresolved_type = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: SymbolHandle::invalid(),
            name: Identifier::generated("SourceFree"),
        });
    assert!(
        program
            .exact_owner_type_identity(ExactOwnerTypeIdentityRequest {
                type_reference: unresolved_type,
                binders: &[],
                substitutions: &[],
                exact_toolchain_sources: &[(source_id, [0x33; 32])]
            })
            .is_none(),
        "exact package identity must reject unresolved nominal ownership"
    );
}

#[test]
fn package_qualified_declared_domains_normalize_their_owner_and_arguments() {
    let mut program = TypedTrees::default();
    let carrier = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: SymbolHandle::invalid(),
            name: Identifier::generated("Carrier"),
        });
    let argument = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: SymbolHandle::invalid(),
            name: Identifier::generated("Argument"),
        });
    let constraints =
        program
            .type_reference_table
            .insert_constraints([TypeConstraintNode::Domain(DomainConstraint {
                name: Identifier::generated("Policy"),
                arguments: vec![argument],
                ..DomainConstraint::default()
            })]);
    let constrained = program
        .type_reference_table
        .insert(TypeReferenceNode::Constrained {
            base_type: carrier,
            constraints,
        });

    let identity = program.package_qualified_type_identity(constrained);
    assert!(identity.as_str().contains("Carrier"), "{identity}");
    assert!(identity.as_str().contains("Policy"), "{identity}");
    assert!(identity.as_str().contains("Argument"), "{identity}");
    assert_eq!(identity.as_str().matches("unresolved-owner").count(), 3);
}

#[test]
fn package_qualified_open_index_authority_qualifies_every_nominal_symbol() {
    let mut program = TypedTrees::default();
    let expression = program
        .expression_table
        .insert_tree(&Expression::Binary(Box::new(BinaryExpression {
            left: Expression::Boolean(true),
            operator: BinaryOperator::Add,
            right: Expression::Boolean(false),
        })));
    program
        .open_index_normalizations
        .push(OpenIndexNormalization {
            expression,
            index_type: arena::Handle::invalid(),
            operations: vec![OpenIndexOperationSelection {
                expression,
                spelling: OperatorSpelling::Add,
                operator: SymbolHandle::invalid(),
                operation_contract_identity: "Index::add(i32,i32)->i32".to_owned(),
                provider: SymbolHandle::invalid(),
                algebra_trait: SymbolHandle::invalid(),
                algebra_requirement: "add".to_owned(),
                algebra_alias: Some("Canonical".to_owned()),
            }],
            normalizer_version: 1,
        });
    let type_reference = program
        .type_reference_table
        .insert(TypeReferenceNode::ConstExpression(expression));

    let identity = program.package_qualified_type_identity(type_reference);
    assert!(
        identity.as_str().contains("open-index-operation"),
        "{identity}"
    );
    assert!(
        identity.as_str().contains("open-index-algebra"),
        "{identity}"
    );
    assert_eq!(identity.as_str().matches("unresolved-owner").count(), 3);
}

#[test]
fn reference_access_modes_have_distinct_normalized_identities() {
    let mut program = TypedTrees::default();
    let referee = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: SymbolHandle::invalid(),
            name: Identifier::generated("u8"),
        });
    let shared = program
        .type_reference_table
        .insert(TypeReferenceNode::Reference {
            referee,
            access: ReferenceAccess::Shared,
            lifetime: None,
        });
    let mutable = program
        .type_reference_table
        .insert(TypeReferenceNode::Reference {
            referee,
            access: ReferenceAccess::Mutable,
            lifetime: None,
        });
    let write_only = program
        .type_reference_table
        .insert(TypeReferenceNode::Reference {
            referee,
            access: ReferenceAccess::WriteOnly,
            lifetime: None,
        });

    let shared = program.normalized_type_identity(shared);
    let mutable = program.normalized_type_identity(mutable);
    let write_only = program.normalized_type_identity(write_only);
    assert!(shared.as_str().starts_with("ref("), "{shared:?}");
    assert!(mutable.as_str().starts_with("ref-mut("), "{mutable:?}");
    assert!(
        write_only.as_str().starts_with("ref-write("),
        "{write_only:?}"
    );
    assert_ne!(shared, mutable);
    assert_ne!(shared, write_only);
    assert_ne!(mutable, write_only);
}

fn declared(name: &str, semantic_id: SemanticDomainId) -> TypeConstraintNode {
    TypeConstraintNode::Domain(DomainConstraint {
        name: Identifier::generated(name),
        arguments: Vec::new(),
        subject: crate::types::DomainConstraintSubject::Declared,
        symbol: SymbolHandle::invalid(),
        semantic_id,
        classification: None,
        predicate_body: DomainPredicateBody::Present,
        semantic_roles: DomainSemanticRoles::default(),
        establishment_routes: Vec::new(),
        authored_selection: None,
    })
}

fn declared_with_metadata(
    name: &str,
    symbol: SymbolHandle,
    semantic_id: SemanticDomainId,
    predicate_body: DomainPredicateBody,
    semantic_roles: DomainSemanticRoles,
    establishment_routes: Vec<DomainEstablishmentRoute>,
) -> TypeConstraintNode {
    TypeConstraintNode::Domain(DomainConstraint {
        name: Identifier::generated(name),
        arguments: Vec::new(),
        subject: crate::types::DomainConstraintSubject::Declared,
        symbol,
        semantic_id,
        classification: None,
        predicate_body,
        semantic_roles,
        establishment_routes,
        authored_selection: None,
    })
}

fn constrained(
    program: &mut TypedTrees,
    base_type: arena::Handle<TypeReferenceNode>,
    constraints: impl IntoIterator<Item = TypeConstraintNode>,
) -> arena::Handle<TypeReferenceNode> {
    let constraints = program.type_reference_table.insert_constraints(constraints);
    program
        .type_reference_table
        .insert(TypeReferenceNode::Constrained {
            base_type,
            constraints,
        })
}

fn generic_machine(
    program: &mut TypedTrees,
    owner_symbol: SymbolHandle,
    binder_symbol: SymbolHandle,
    binder_name: &str,
    return_type: arena::Handle<TypeReferenceNode>,
) -> crate::machine::Machine {
    let mut machine = crate::machine::Machine {
        symbol: owner_symbol,
        name: Identifier::generated("I32::from_value"),
        ..crate::machine::Machine::default()
    };
    program.push_machine_type_parameter(
        &mut machine,
        crate::data::TypeParameter {
            symbol: binder_symbol,
            name: Identifier::generated(binder_name),
            ..crate::data::TypeParameter::default()
        },
    );
    let parameter_type = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: binder_symbol,
            name: Identifier::generated(binder_name),
        });
    let mut entry = crate::state::State {
        symbol: SymbolHandle::from_arena_index(owner_symbol.arena_index() + 100),
        name: Identifier::generated("from_value"),
        return_type,
        ..crate::state::State::default()
    };
    program.push_state_parameter(
        &mut entry,
        crate::signature::StateParameter {
            symbol: SymbolHandle::from_arena_index(owner_symbol.arena_index() + 200),
            name: Identifier::generated("value"),
            type_reference: parameter_type,
            ..crate::signature::StateParameter::default()
        },
    );
    program.push_machine_state(&mut machine, entry);
    machine
}

#[test]
fn domain_conjunction_identity_is_sorted_deduplicated_and_semantic() {
    let mut program = TypedTrees::default();
    let utf8 = program.semantic_domains.intern("Utf8");
    let no_nul = program.semantic_domains.intern("NoNul");
    let constraints = program.type_reference_table.insert_constraints([
        declared("AliasForNoNul", no_nul),
        declared("Utf8", utf8),
        declared("Utf8Again", utf8),
    ]);

    let normalized = program.normalized_domain_expression(constraints);
    assert_eq!(
        normalized.terms(),
        [
            NormalizedDomainTerm::Declared("NoNul".to_owned()),
            NormalizedDomainTerm::Declared("Utf8".to_owned()),
        ]
    );
}

#[test]
fn reordered_flat_and_nested_domain_conjunctions_have_one_type_identity() {
    let mut program = TypedTrees::default();
    let alpha = program.semantic_domains.intern("Alpha");
    let beta = program.semantic_domains.intern("Beta");
    let base = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: SymbolHandle::invalid(),
            name: Identifier::generated("i32"),
        });

    let flat = constrained(
        &mut program,
        base,
        [declared("Alpha", alpha), declared("Beta", beta)],
    );
    let nested_inner = constrained(&mut program, base, [declared("Beta", beta)]);
    let nested = constrained(
        &mut program,
        nested_inner,
        [declared("AlphaAlias", alpha), declared("Alpha", alpha)],
    );

    assert_eq!(
        program.normalized_type_identity(flat),
        program.normalized_type_identity(nested)
    );
}

#[test]
fn same_constraint_count_does_not_collapse_distinct_domains() {
    let mut program = TypedTrees::default();
    let alpha = program.semantic_domains.intern("Alpha");
    let beta = program.semantic_domains.intern("Beta");
    let base = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: SymbolHandle::invalid(),
            name: Identifier::generated("i32"),
        });
    let alpha_type = constrained(&mut program, base, [declared("Alpha", alpha)]);
    let beta_type = constrained(&mut program, base, [declared("Beta", beta)]);

    let alpha_identity: NormalizedTypeIdentity = program.normalized_type_identity(alpha_type);
    assert_ne!(alpha_identity, program.normalized_type_identity(beta_type));
}

#[test]
fn open_index_identity_is_structural_not_diagnostic_display() {
    let mut program = TypedTrees::default();
    let expression = |left: &str, right: &str| {
        Expression::Binary(Box::new(BinaryExpression {
            left: Expression::Name(NamePath::unresolved_from_iter([Identifier::generated(
                left,
            )])),
            operator: BinaryOperator::Divide,
            right: Expression::Name(NamePath::unresolved_from_iter([Identifier::generated(
                right,
            )])),
        }))
    };
    let a_over_b = program.expression_table.insert_tree(&expression("A", "B"));
    let same_a_over_b = program.expression_table.insert_tree(&expression("A", "B"));
    let b_over_a = program.expression_table.insert_tree(&expression("B", "A"));
    let a_over_b = program
        .type_reference_table
        .insert(TypeReferenceNode::ConstExpression(a_over_b));
    let same_a_over_b = program
        .type_reference_table
        .insert(TypeReferenceNode::ConstExpression(same_a_over_b));
    let b_over_a = program
        .type_reference_table
        .insert(TypeReferenceNode::ConstExpression(b_over_a));

    assert_eq!(
        program.normalized_type_identity(a_over_b),
        program.normalized_type_identity(same_a_over_b)
    );
    assert_ne!(
        program.normalized_type_identity(a_over_b),
        program.normalized_type_identity(b_over_a)
    );
}

#[test]
fn licensed_open_index_identity_flattens_and_sorts_exact_ac_operation() {
    fn name(value: &str) -> Expression {
        Expression::Name(NamePath::unresolved_from_iter([Identifier::generated(
            value,
        )]))
    }
    fn add(left: Expression, right: Expression) -> Expression {
        Expression::Binary(Box::new(BinaryExpression {
            left,
            operator: BinaryOperator::Add,
            right,
        }))
    }
    fn binary_nodes(
        program: &TypedTrees,
        expression: ExpressionHandle,
        output: &mut Vec<ExpressionHandle>,
    ) {
        let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
            return;
        };
        output.push(expression);
        binary_nodes(program, binary.left, output);
        binary_nodes(program, binary.right, output);
    }
    fn license(program: &mut TypedTrees, expression: ExpressionHandle, contract: &str) {
        let mut nodes = Vec::new();
        binary_nodes(program, expression, &mut nodes);
        let operations = nodes
            .into_iter()
            .map(|expression| OpenIndexOperationSelection {
                expression,
                spelling: OperatorSpelling::Add,
                operator: SymbolHandle::from_arena_index(71),
                operation_contract_identity: contract.to_owned(),
                provider: SymbolHandle::from_arena_index(72),
                algebra_trait: SymbolHandle::from_arena_index(73),
                algebra_requirement: "add".to_owned(),
                algebra_alias: Some("Canonical".to_owned()),
            })
            .collect();
        program
            .open_index_normalizations
            .push(OpenIndexNormalization {
                expression,
                index_type: arena::Handle::invalid(),
                operations,
                normalizer_version: 1,
            });
    }

    let mut program = TypedTrees::default();
    let left_associated = program
        .expression_table
        .insert_tree(&add(add(name("A"), name("B")), name("C")));
    let reordered = program
        .expression_table
        .insert_tree(&add(name("C"), add(name("B"), name("A"))));
    let different_authority = program
        .expression_table
        .insert_tree(&add(add(name("A"), name("B")), name("C")));
    license(&mut program, left_associated, "IndexAlgebra::plus");
    license(&mut program, reordered, "IndexAlgebra::plus");
    license(&mut program, different_authority, "OtherIndexAlgebra::plus");
    let left_associated = program
        .type_reference_table
        .insert(TypeReferenceNode::ConstExpression(left_associated));
    let reordered = program
        .type_reference_table
        .insert(TypeReferenceNode::ConstExpression(reordered));
    let different_authority = program
        .type_reference_table
        .insert(TypeReferenceNode::ConstExpression(different_authority));

    assert_eq!(
        program.normalized_type_identity(left_associated),
        program.normalized_type_identity(reordered)
    );
    assert_ne!(
        program.normalized_type_identity(left_associated),
        program.normalized_type_identity(different_authority)
    );
}

#[test]
fn result_dispatch_set_partitions_predicates_from_semantic_and_empty_tags() {
    let mut program = TypedTrees::default();
    let predicate = program.semantic_domains.intern("Positive");
    let semantic = program.semantic_domains.intern("Km");
    let routed = program.semantic_domains.intern("Validated");
    let empty = program.semantic_domains.intern("Marker");
    let base = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: SymbolHandle::invalid(),
            name: Identifier::generated("i32"),
        });
    let route = DomainEstablishmentRoute::CheckedRequirement {
        trait_definition: SymbolHandle::from_arena_index(41),
        requirement: SymbolHandle::from_arena_index(42),
    };
    let result = constrained(
        &mut program,
        base,
        [
            declared_with_metadata(
                "Positive",
                SymbolHandle::invalid(),
                predicate,
                DomainPredicateBody::Present,
                DomainSemanticRoles::default(),
                Vec::new(),
            ),
            declared_with_metadata(
                "Km",
                SymbolHandle::invalid(),
                semantic,
                DomainPredicateBody::Present,
                DomainSemanticRoles {
                    denotation_dimension: Some(semantic),
                    arithmetic_policy: None,
                },
                Vec::new(),
            ),
            declared_with_metadata(
                "Validated",
                SymbolHandle::invalid(),
                routed,
                DomainPredicateBody::Present,
                DomainSemanticRoles::default(),
                vec![route],
            ),
            declared_with_metadata(
                "Marker",
                SymbolHandle::invalid(),
                empty,
                DomainPredicateBody::Bodyless,
                DomainSemanticRoles::default(),
                Vec::new(),
            ),
            TypeConstraintNode::ArithmeticDomain(
                numerics::arithmetic::ArithmeticDomain::Saturating,
            ),
            declared_with_metadata(
                "MarkerAgain",
                SymbolHandle::invalid(),
                empty,
                DomainPredicateBody::Bodyless,
                DomainSemanticRoles::default(),
                Vec::new(),
            ),
            TypeConstraintNode::ArithmeticDomain(
                numerics::arithmetic::ArithmeticDomain::Saturating,
            ),
        ],
    );

    let dispatch = program.normalized_result_dispatch_set(result);
    assert_eq!(
        dispatch.terms(),
        [
            NormalizedDomainTerm::Arithmetic("Saturating".to_owned()),
            NormalizedDomainTerm::Declared("Km".to_owned()),
            NormalizedDomainTerm::Declared("Marker".to_owned()),
            NormalizedDomainTerm::Declared("Validated".to_owned()),
        ]
    );
    assert_eq!(
        dispatch.identity(),
        "arithmetic:Saturating&declared:Km&declared:Marker&declared:Validated"
    );
}

#[test]
fn result_dispatch_set_expands_aliases_before_partitioning() {
    let mut program = TypedTrees::default();
    let predicate_id = program.semantic_domains.intern("Positive");
    let marker_id = program.semantic_domains.intern("Marker");
    let alias_id = program.semantic_domains.intern("PositiveMarker");
    let predicate_symbol = SymbolHandle::from_arena_index(51);
    let marker_symbol = SymbolHandle::from_arena_index(52);
    let alias_symbol = SymbolHandle::from_arena_index(53);

    program.push_domain_definition(DomainDefinition {
        symbol: predicate_symbol,
        name: Identifier::generated("Positive"),
        semantic_id: predicate_id,
        predicate_body: DomainPredicateBody::Present,
        ..DomainDefinition::default()
    });
    program.push_domain_definition(DomainDefinition {
        symbol: marker_symbol,
        name: Identifier::generated("Marker"),
        semantic_id: marker_id,
        predicate_body: DomainPredicateBody::Bodyless,
        ..DomainDefinition::default()
    });
    program.push_domain_definition(DomainDefinition {
        symbol: alias_symbol,
        name: Identifier::generated("PositiveMarker"),
        alias: Some(DomainAliasDefinition {
            constituents: vec![
                DomainAliasConstituent {
                    domain_symbol: predicate_symbol,
                    ..DomainAliasConstituent::default()
                },
                DomainAliasConstituent {
                    domain_symbol: marker_symbol,
                    ..DomainAliasConstituent::default()
                },
            ],
        }),
        semantic_id: alias_id,
        ..DomainDefinition::default()
    });

    let base = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: SymbolHandle::invalid(),
            name: Identifier::generated("i32"),
        });
    let result = constrained(
        &mut program,
        base,
        [declared_with_metadata(
            "PositiveMarker",
            alias_symbol,
            alias_id,
            DomainPredicateBody::Bodyless,
            DomainSemanticRoles::default(),
            Vec::new(),
        )],
    );

    assert_eq!(
        program.normalized_result_dispatch_set(result).terms(),
        [NormalizedDomainTerm::Declared("Marker".to_owned())]
    );
}

#[test]
fn result_dispatch_set_flattens_qualification_shells_but_not_element_domains() {
    let mut program = TypedTrees::default();
    let outer = program.semantic_domains.intern("Outer");
    let element = program.semantic_domains.intern("Element");
    let base = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: SymbolHandle::invalid(),
            name: Identifier::generated("i32"),
        });
    let qualified_element = constrained(
        &mut program,
        base,
        [declared_with_metadata(
            "Element",
            SymbolHandle::invalid(),
            element,
            DomainPredicateBody::Bodyless,
            DomainSemanticRoles::default(),
            Vec::new(),
        )],
    );
    let array = program
        .type_reference_table
        .insert(TypeReferenceNode::FixedArray {
            element_type: qualified_element,
            length: crate::types::FixedArrayLength::Literal(4),
        });
    let result = constrained(
        &mut program,
        array,
        [declared_with_metadata(
            "Outer",
            SymbolHandle::invalid(),
            outer,
            DomainPredicateBody::Bodyless,
            DomainSemanticRoles::default(),
            Vec::new(),
        )],
    );

    assert_eq!(
        program.normalized_result_dispatch_set(result).terms(),
        [NormalizedDomainTerm::Declared("Outer".to_owned())]
    );
}

#[test]
fn named_machine_identity_normalizes_binders_and_collapses_predicate_only_results() {
    let mut program = TypedTrees::default();
    let i32_type = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: SymbolHandle::invalid(),
            name: Identifier::generated("i32"),
        });
    let positive = program.semantic_domains.intern("Positive");
    let predicate_result = constrained(
        &mut program,
        i32_type,
        [declared_with_metadata(
            "Positive",
            SymbolHandle::invalid(),
            positive,
            DomainPredicateBody::Present,
            DomainSemanticRoles::default(),
            Vec::new(),
        )],
    );
    let saturating_result = constrained(
        &mut program,
        i32_type,
        [TypeConstraintNode::ArithmeticDomain(
            numerics::arithmetic::ArithmeticDomain::Saturating,
        )],
    );
    let unqualified = generic_machine(
        &mut program,
        SymbolHandle::from_arena_index(61),
        SymbolHandle::from_arena_index(71),
        "T",
        i32_type,
    );
    let predicate_only = generic_machine(
        &mut program,
        SymbolHandle::from_arena_index(62),
        SymbolHandle::from_arena_index(72),
        "Renamed",
        predicate_result,
    );
    let saturating = generic_machine(
        &mut program,
        SymbolHandle::from_arena_index(63),
        SymbolHandle::from_arena_index(73),
        "AnotherName",
        saturating_result,
    );

    // These source-free fixtures deliberately use unattached symbol handles.
    // Their supplied declaration path remains meaningful without a module.
    assert!(!program.symbols.symbol_module(unqualified.symbol).is_valid());
    let unqualified_identity = program
        .normalized_machine_overload_identity(&unqualified)
        .expect("machine has an entry");
    let predicate_identity = program
        .normalized_machine_overload_identity(&predicate_only)
        .expect("machine has an entry");
    let saturating_identity = program
        .normalized_machine_overload_identity(&saturating)
        .expect("machine has an entry");

    assert_eq!(unqualified_identity, predicate_identity);
    assert_ne!(unqualified_identity, saturating_identity);
    assert_eq!(unqualified_identity.path(), "I32::from_value");
    assert!(unqualified_identity.result_dispatch().is_empty());
    assert_eq!(
        saturating_identity.result_dispatch().identity(),
        "arithmetic:Saturating"
    );
    assert!(unqualified_identity.parameters().contains("$T0"));
    assert!(!unqualified_identity.identity().is_empty());
}
