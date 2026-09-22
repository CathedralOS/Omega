//! Compiler-recognized BYTE-SEQUENCE predicate facts -- the reusable
//! building blocks a domain selects by spelling one as an ordinary fact
//! (`domain [u8]::Utf8 { valid_utf8(self); }`). Moved here from the checker's `field_domain.rs`
//! (2026-07-16) so the RUNTIME decode boundary shares ONE vocabulary with
//! the compile-time proof machinery: wire decode brings UNTRUSTED bytes
//! where no compile-time proof exists, and the decoder must evaluate the
//! same predicate the checker proves elsewhere (`holds_for`). The ENUM
//! itself lives in `language_semantics::byte_predicates` (dependency-free, so the
//! instruction kinds can carry predicate MASKS); this module owns the
//! TREE-WALKING resolution from domain declarations.

use crate::TypedTrees;
use crate::expression::{ExpressionHandle, ExpressionNode};
use crate::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};
use symbols::SymbolHandle;

pub use language_semantics::byte_predicates::ByteSequencePredicate;

/// If `domain_symbol`'s sole fact is a recognized comptime byte-predicate call
/// applied to `self` (e.g. `valid_utf8(self);`), return that primitive. Domains
/// with additional facts require general proof evaluation and are not reduced
/// to a single byte predicate. The same is true of the surrounding theory:
/// a transparent alias expands to constituent requirements, index binders or
/// instance arguments carry an identity byte extraction cannot substitute
/// into, and `established by` routes restrict who may mint membership at all
/// (`domain_requires_provenance` enforces the same list for writes) -- so any
/// of them keeps the domain a general proof obligation no byte check closes.
pub fn domain_byte_predicate(
    program: &TypedTrees,
    domain_symbol: SymbolHandle,
) -> Option<ByteSequencePredicate> {
    let domain = program
        .domain_definitions()
        .iter()
        .find(|domain| domain.symbol == domain_symbol)?;
    if domain.alias.is_some()
        || !domain.index_arguments.is_empty()
        || !domain.establishment_routes.is_empty()
        || !crate::domain::index_parameters(program, domain).is_empty()
    {
        return None;
    }
    let [crate::domain::ProofFact::Expression(expression)] =
        program.proof_facts.span_or_empty(domain.facts)
    else {
        return None;
    };

    let ExpressionNode::Call(call) = program.expression_table.expression(*expression) else {
        return None;
    };
    // A free-function predicate over `self`: no receiver, exactly one argument,
    // and that argument is the bare `self` subject the fact scrutinizes.
    if call.receiver.is_valid() {
        return None;
    }
    let predicate = ByteSequencePredicate::from_name(call.target.as_str())?;
    let arguments = program.expression_table.expression_handles(call.arguments);
    let [argument] = arguments else {
        return None;
    };
    if !expression_is_self_reference(program, *argument) {
        return None;
    }
    Some(predicate)
}

/// The DOMAIN constraints declared on a type reference (walking Reference and
/// Constrained wrappers), resolved by NAME against the program's domain
/// definitions: `(domain name, recognized predicate)` per constraint. An inner
/// `None` predicate means the domain is not exactly one recognized byte fact --
/// the decode boundary must refuse LOUDLY rather than skip validation silently.
pub fn type_reference_domain_predicates(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Vec<(String, Option<ByteSequencePredicate>)> {
    let mut predicates = Vec::new();
    let mut handle = type_reference;
    loop {
        if !handle.is_valid() {
            return predicates;
        }
        match program.type_reference_table.type_reference(handle) {
            TypeReferenceNode::Reference { referee, .. } => handle = *referee,
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                for constraint in program.type_reference_table.constraints(*constraints) {
                    let TypeConstraintNode::Domain(domain_constraint) = constraint else {
                        continue;
                    };
                    // The carrier-aware typed normalization pass already
                    // selected the declaration; never repeat a global lookup
                    // from the authored short name here.
                    let predicate = domain_constraint
                        .symbol
                        .is_valid()
                        .then(|| domain_byte_predicate(program, domain_constraint.symbol))
                        .flatten();
                    predicates.push((domain_constraint.name.as_str().to_owned(), predicate));
                }
                handle = *base_type;
            }
            _ => return predicates,
        }
    }
}

/// Whether `expression` is the bare domain subject `self` -- a single-member
/// name path spelled `self`. The predicate must apply to the domain value, not
/// to some unrelated place.
fn expression_is_self_reference(program: &TypedTrees, expression: ExpressionHandle) -> bool {
    let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
        return false;
    };
    let members = program.expression_table.name_path_members(path.members);
    matches!(members, [member] if member.is_self_receiver())
}

#[cfg(test)]
mod tests {
    use super::{ByteSequencePredicate, domain_byte_predicate, type_reference_domain_predicates};
    use crate::TypedTrees;
    use crate::domain::{DomainAliasDefinition, DomainDefinition, ProofFact};
    use crate::expression::{ExpressionHandle, ExpressionNode};
    use crate::expression::{TableCallExpression, TableNamePath};
    use crate::name::Identifier;
    use crate::types::TypeReferenceHandle;
    use crate::types::{DomainConstraint, TypeConstraintNode, TypeReferenceNode};
    use arena::HandleSpan;
    use language_semantics::DomainEstablishmentRoute;
    use symbols::SymbolHandle;

    fn self_predicate_fact(trees: &mut TypedTrees, predicate: &str) -> ProofFact {
        let mut members = HandleSpan::empty();
        trees
            .expression_table
            .push_name_path_member(&mut members, Identifier::generated("self"));
        let subject = trees
            .expression_table
            .insert(ExpressionNode::Name(TableNamePath {
                members,
                ..TableNamePath::default()
            }));
        let mut arguments = HandleSpan::empty();
        trees
            .expression_table
            .push_expression_handle(&mut arguments, subject);
        ProofFact::Expression(trees.expression_table.insert(ExpressionNode::Call(
            TableCallExpression {
                receiver: ExpressionHandle::invalid(),
                target_symbol: SymbolHandle::invalid(),
                static_machine_parameter: SymbolHandle::invalid(),
                target: Identifier::generated(predicate),
                static_requirement_dispatch: None,
                machine_arguments: Box::default(),
                quotient_operation: None,
                private_layout_operation: None,
                arguments,
                evidence_arguments: Box::default(),
                operational_acknowledgement:
                    language_semantics::CallOperationalAcknowledgement::default(),
            },
        )))
    }

    fn declare_domain(
        trees: &mut TypedTrees,
        symbol: SymbolHandle,
        domain: DomainDefinition,
    ) -> SymbolHandle {
        let mut domain = domain;
        domain.symbol = symbol;
        let handle = trees.domain_definitions.insert(domain);
        trees.roots.domain_definitions.push_contiguous(handle);
        symbol
    }

    fn utf8_domain(trees: &mut TypedTrees, symbol_index: u32) -> SymbolHandle {
        let fact = self_predicate_fact(trees, "valid_utf8");
        let mut domain = DomainDefinition {
            name: Identifier::generated("Utf8"),
            predicate_body: language_semantics::DomainPredicateBody::Present,
            ..DomainDefinition::default()
        };
        trees.proof_facts.append_to_span(&mut domain.facts, fact);
        declare_domain(trees, SymbolHandle::from_arena_index(symbol_index), domain)
    }

    #[test]
    fn sole_self_predicate_fact_reduces_to_the_primitive() {
        let mut trees = TypedTrees::default();
        let symbol = utf8_domain(&mut trees, 1);
        assert_eq!(
            domain_byte_predicate(&trees, symbol),
            Some(ByteSequencePredicate::ValidUtf8)
        );
    }

    #[test]
    fn an_additional_fact_is_not_reducible() {
        let mut trees = TypedTrees::default();
        let symbol = SymbolHandle::from_arena_index(1);
        let mut domain = DomainDefinition {
            name: Identifier::generated("StrictUtf8"),
            predicate_body: language_semantics::DomainPredicateBody::Present,
            ..DomainDefinition::default()
        };
        let first = self_predicate_fact(&mut trees, "valid_utf8");
        let second = self_predicate_fact(&mut trees, "non_empty");
        trees.proof_facts.append_to_span(&mut domain.facts, first);
        trees.proof_facts.append_to_span(&mut domain.facts, second);
        let symbol = declare_domain(&mut trees, symbol, domain);
        assert_eq!(domain_byte_predicate(&trees, symbol), None);
    }

    #[test]
    fn an_establishment_route_is_not_reducible() {
        // `domain [u8]::D requires valid_utf8(self) established by X::ingest`
        // still spells one byte-predicate fact, but membership may only be
        // minted through the authored route -- the decoder must refuse rather
        // than mint `in D` from raw bytes.
        let mut trees = TypedTrees::default();
        let fact = self_predicate_fact(&mut trees, "valid_utf8");
        let mut domain = DomainDefinition {
            name: Identifier::generated("RoutedUtf8"),
            predicate_body: language_semantics::DomainPredicateBody::Present,
            establishment_routes: vec![DomainEstablishmentRoute::ExactMachine {
                machine: SymbolHandle::from_arena_index(90),
            }],
            ..DomainDefinition::default()
        };
        trees.proof_facts.append_to_span(&mut domain.facts, fact);
        let symbol = declare_domain(&mut trees, SymbolHandle::from_arena_index(1), domain);
        assert_eq!(domain_byte_predicate(&trees, symbol), None);
    }

    #[test]
    fn an_alias_or_indexed_domain_is_not_reducible() {
        let mut trees = TypedTrees::default();
        let fact = self_predicate_fact(&mut trees, "valid_utf8");
        let mut aliased = DomainDefinition {
            name: Identifier::generated("AliasedUtf8"),
            predicate_body: language_semantics::DomainPredicateBody::Present,
            alias: Some(DomainAliasDefinition::default()),
            ..DomainDefinition::default()
        };
        trees.proof_facts.append_to_span(&mut aliased.facts, fact);
        let alias_symbol = declare_domain(&mut trees, SymbolHandle::from_arena_index(1), aliased);
        assert_eq!(domain_byte_predicate(&trees, alias_symbol), None);

        let fact = self_predicate_fact(&mut trees, "valid_utf8");
        let mut indexed = DomainDefinition {
            name: Identifier::generated("IndexedUtf8"),
            predicate_body: language_semantics::DomainPredicateBody::Present,
            index_arguments: vec![TypeReferenceHandle::invalid()],
            ..DomainDefinition::default()
        };
        trees.proof_facts.append_to_span(&mut indexed.facts, fact);
        let index_symbol = declare_domain(&mut trees, SymbolHandle::from_arena_index(2), indexed);
        assert_eq!(domain_byte_predicate(&trees, index_symbol), None);
    }

    #[test]
    fn the_decode_boundary_surfaces_an_unresolvable_domain_as_none() {
        // `&[u8] in RoutedUtf8`: the walk keeps the domain's name for the
        // refusal diagnostic while the predicate slot is None.
        let mut trees = TypedTrees::default();
        let fact = self_predicate_fact(&mut trees, "valid_utf8");
        let mut domain = DomainDefinition {
            name: Identifier::generated("RoutedUtf8"),
            predicate_body: language_semantics::DomainPredicateBody::Present,
            establishment_routes: vec![DomainEstablishmentRoute::ExactMachine {
                machine: SymbolHandle::from_arena_index(90),
            }],
            ..DomainDefinition::default()
        };
        trees.proof_facts.append_to_span(&mut domain.facts, fact);
        let symbol = declare_domain(&mut trees, SymbolHandle::from_arena_index(1), domain);

        let byte = trees.type_reference_table.insert(TypeReferenceNode::Named {
            symbol: SymbolHandle::invalid(),
            name: Identifier::generated("u8"),
        });
        let slice = trees
            .type_reference_table
            .insert(TypeReferenceNode::Slice { element_type: byte });
        let mut constraints = HandleSpan::empty();
        trees.type_reference_table.push_constraint(
            &mut constraints,
            TypeConstraintNode::Domain(DomainConstraint {
                name: Identifier::generated("RoutedUtf8"),
                symbol,
                ..DomainConstraint::default()
            }),
        );
        let constrained = trees
            .type_reference_table
            .insert(TypeReferenceNode::Constrained {
                base_type: slice,
                constraints,
            });
        let reference = trees
            .type_reference_table
            .insert(TypeReferenceNode::Reference {
                referee: constrained,
                access: language_core::ReferenceAccess::Shared,
                lifetime: None,
            });

        let predicates = type_reference_domain_predicates(&trees, reference);
        assert_eq!(predicates, vec![("RoutedUtf8".to_owned(), None)]);
    }
}
