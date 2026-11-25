//! Prepare typed source for lowering. Specialization binds static calls, then the
//! domain pass resolves the declarations used by those bodies.

#![forbid(unsafe_code)]

#[cfg(test)]
use source_declarations::declaration_selection;
#[cfg(test)]
use source_symbol_table::source_inputs;
#[cfg(test)]
use source_symbol_table::source_symbols;
use typed_trees::source_state;

mod passes;
pub use prepared_source::{
    EntryApplication, PreparedTarget, PreparedTypedTrees, PreparedValue, TargetInputs, ValueInputs,
};

use crate::source_state::TypedTrees;
use diagnostics::Diagnostic;
use prepared_source_issuer::PREPARATION;

/// A provider whose bodies the selected target needs. A generic provider
/// keeps its complete argument tuple in the typed source's application table.
#[derive(Clone, Copy)]
pub struct SelectedProvider {
    pub declaration: symbols::SymbolHandle,
    pub application:
        Option<arena::Handle<source_state::product_operand::ProductOperandApplication>>,
}

/// Declarations selected by the consuming product for ordinary specialization.
#[derive(Clone, Copy)]
pub enum SelectedDeclaration {
    Provider(SelectedProvider),
    MachineApplication(EntryApplication),
}

/// Prepare the typed program that stage 05 will lower.
///
/// Cloning the result retains the same source owner for later judgments.
///
/// ```
/// use typed_trees::source_state::TypedTrees;
/// use typed_trees_to_prepared_typed_trees::prepare_typed_trees;
/// let prepared = prepare_typed_trees(TypedTrees::default(), &[], &[]).unwrap();
/// assert!(std::sync::Arc::ptr_eq(prepared.trees(), prepared.clone().trees()));
/// ```
pub fn prepare_typed_trees(
    mut program: TypedTrees,
    values: &[ValueInputs],
    selected: &[SelectedDeclaration],
) -> Result<PreparedTypedTrees, Vec<Diagnostic>> {
    let entries = passes::specialization::prepare_selected(&mut program, selected)?;
    let first_requirement_type = program.type_reference_table.type_reference_count();
    program
        .prepare_conformance_requirement_headers()
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
    // Substituting a provider's receiver creates fresh type occurrences in its
    // parameters and result. Lowering needs the data declaration at each one.
    typed_trees::data_applications::prepare_from(&mut program, first_requirement_type)
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
    program
        .settle_applied_service_lineage()
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
    let first_domain_type = program.type_reference_table.type_reference_count();
    let domain_schemas = passes::domain_applications::prepare(&mut program);
    // Applying a domain substitutes its carrier and the types used by its predicates.
    // Retain the data members at those new occurrences while the typed trees are mutable.
    typed_trees::data_applications::prepare_from(&mut program, first_domain_type)
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
    PreparedTypedTrees::new(PREPARATION, program, values, entries, domain_schemas)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_domain_family_prepares_each_retained_carrier_without_rewriting_the_source() {
        use crate::source_state::data::{TypeParameter, TypeParameterKind};
        use crate::source_state::domain::DomainDefinition;
        use crate::source_state::name::Identifier;
        use crate::source_state::types::TypeReferenceNode;
        use crate::source_symbols::SymbolTableBuilder;
        use symbols::{SymbolKind, SymbolNameRef};
        let mut builder = SymbolTableBuilder::new();
        let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let declarations = builder.insert_children(
            root,
            crate::source_symbols::builtin_type_symbols()
                .into_iter()
                .chain([(SymbolKind::Domain, SymbolNameRef::Static("Distance"))]),
        );
        let domain = declarations.handle_at(symbols::BUILTIN_TYPE_COUNT as u32);
        let children = builder.insert_children(
            domain,
            [
                (SymbolKind::TypeParameter, SymbolNameRef::Static("T")),
                (SymbolKind::Parameter, SymbolNameRef::Static("self")),
            ],
        );
        let parameter = children.handle_at(0);
        let subject = children.handle_at(1);
        let mut program = TypedTrees {
            symbols: builder.finish(),
            ..Default::default()
        };
        let target = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: parameter,
                name: Identifier::generated("T"),
            });
        let parameters = program.data_type_parameters.insert_many([TypeParameter {
            symbol: parameter,
            name: Identifier::generated("T"),
            kind: TypeParameterKind::Type,
            ..Default::default()
        }]);
        let identity = program.semantic_domains.intern("Distance");
        program.push_domain_definition(DomainDefinition {
            symbol: domain,
            name: Identifier::generated("Distance"),
            target_type: target,
            type_parameters: parameters,
            subject_symbol: subject,
            semantic_id: identity,
            ..Default::default()
        });
        let mut retained = Vec::new();
        for width in [32, 64] {
            let symbol = program
                .symbols
                .builtin_atom_symbol(symbols::BuiltinTypeAtom::Integer(
                    numerics::integer_carrier::FixedInteger::unsigned(width),
                ))
                .unwrap();
            let carrier = program
                .type_reference_table
                .insert(TypeReferenceNode::Named {
                    symbol,
                    name: Identifier::generated("diagnostic alias"),
                });
            let identity = program
                .retain_domain_application(domain, carrier, &[])
                .unwrap();
            assert_eq!(
                program
                    .retain_domain_application(domain, carrier, &[])
                    .unwrap(),
                identity
            );
            assert!(
                program
                    .domain_definition_for_instance(domain, identity)
                    .is_none()
            );
            assert!(
                program
                    .retain_domain_application(domain, carrier, &[carrier])
                    .is_err()
            );
            retained.push((carrier, identity));
        }
        assert_ne!(retained[0].1, retained[1].1);
        let prepared = prepare_typed_trees(program, &[], &[]).unwrap();
        let prepared = prepared.trees();
        for (carrier, identity) in retained {
            let theory = prepared
                .domain_definition_for_instance(domain, identity)
                .unwrap();
            assert!(prepared.domain_type_parameters(theory).is_empty());
            assert_eq!(
                prepared.package_qualified_type_identity(theory.target_type),
                prepared.package_qualified_type_identity(carrier)
            );
            assert_eq!(
                prepared.symbols.authored_origin(theory.subject_symbol),
                subject
            );
            assert_eq!(
                theory.symbol, domain,
                "application retains the declaring owner"
            );
        }
        let declaration = prepared.domain_definition(domain).unwrap();
        assert_eq!(declaration.target_type, target);
        assert_eq!(declaration.subject_symbol, subject);
        assert_eq!(prepared.domain_type_parameters(declaration).len(), 1);
    }

    #[test]
    fn preparation_preserves_the_program_identity() {
        let program = TypedTrees::default();
        let identity = program.identity;
        let frozen = prepare_typed_trees(program, &[], &[]).unwrap();
        assert_eq!(frozen.trees().identity.get(), identity.get());
    }

    #[test]
    fn a_program_no_pass_has_work_for_crosses_unchanged() {
        // A retained declaration no pass reads is not a reason to refuse.
        let mut program = TypedTrees::default();
        program
            .tables
            .mathematical_definitions
            .insert(crate::source_state::mathematical::MathematicalDefinition::default());
        let before = program.clone();
        let frozen = prepare_typed_trees(program, &[], &[]).unwrap();
        assert_eq!(frozen.into_inner(), before);
    }
}
