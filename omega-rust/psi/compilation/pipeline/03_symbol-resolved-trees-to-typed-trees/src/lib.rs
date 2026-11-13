/*
We attach type and signature meaning to symbol-resolved Omega trees
(`lower_symbol_resolved_trees`), and extend retained typed bases across
generated-source continuations (`lower_seeded_extension` and
`lower_symbol_resolved_trees_to_seeded_base`).
*/

/*
Our typing route runs through two shared cohort operations in `lowerer.rs`
(`type_resolved_cohort` and `close_typed_cohort`). Before erasing resolved
membership nodes, `equality_and_exhaustiveness.rs` validates bare case
equality, synthesized `Equatable` conformances, and match exhaustiveness.
`declarations.rs` then lowers every declaration family through
`contracts_and_signatures.rs`, `type_reference.rs`, `expression_lowering.rs`,
`statement_lowering.rs`, and `call_and_product_selections.rs`.
*/

/*
Stage 03 (`symbol-resolved-trees-to-typed-trees`) consumes `SymbolResolvedTrees`
from `syntax-trees-to-symbol-resolved-trees` (Stage 02) and produces
`TypedTrees` for `typed-trees-to-typed-trees` (Stage 04).
*/

#![forbid(unsafe_code)]

// Typing retains declarations and their exact identities. Any operation that
// still needs a retired checker judgment refuses at the requested construct;
// it does not imply that lowering or a later consumer has proved it.
fn unavailable_judgment(construct: &str) -> diagnostics::Diagnostic {
    diagnostics::Diagnostic::error(format!(
        "no product route yet: {construct} requires a lowered-form judgment"
    ))
}

mod call_and_product_selections;
mod contracts_and_signatures;
mod declarations;
mod equality_and_exhaustiveness;
mod expression_lowering;
mod lowerer;
mod statement_lowering;
mod type_reference;
mod typing_queries;

pub(crate) use contracts_and_signatures::{contracts, signatures};

pub(crate) mod expressions {
    pub(crate) use crate::call_and_product_selections::{
        build_selections, call_results, capture_product_operands, construction_destinations,
        installed_interrupts, product_queries, product_selections, qualification_casts,
    };
    pub(crate) use crate::equality_and_exhaustiveness::{
        equality, equatable, exhaustiveness, proposition,
    };
    pub(crate) use crate::expression_lowering::expression;
    pub(crate) use crate::statement_lowering::statement;
}

pub(crate) mod access_borrow_factory {
    use crate::tables::type_constraints;
    use diagnostics::Diagnostic;
    use symbol_resolved_trees as resolved;
    use typed_trees as typed;

    pub fn access_borrow_factory(
        program: &resolved::SymbolResolvedTrees,
        target_symbol: symbols::SymbolHandle,
    ) -> Result<Option<typed::expression::AccessBorrowOperationRequest>, Diagnostic> {
        let found = if target_symbol.is_valid()
            && program.symbols.get(target_symbol).kind == symbols::SymbolKind::Machine
        {
            program.machines.by_symbol(target_symbol)
        } else {
            program.machines.iter().find(|machine| {
                machine.symbol == target_symbol
                    || program
                        .machine_state_handles(machine.states)
                        .iter()
                        .any(|state| program.machine_state(*state).symbol == target_symbol)
            })
        };
        let Some(machine) = found else {
            return Ok(None);
        };
        access_borrow_factory_of(program, machine)
    }

    pub fn access_borrow_factory_of(
        program: &resolved::SymbolResolvedTrees,
        machine: &resolved::machine::Machine,
    ) -> Result<Option<typed::expression::AccessBorrowOperationRequest>, Diagnostic> {
        let (access, resident) = match machine.name.as_str().rsplit("::").next() {
            Some("view") => (language_core::ReferenceAccess::Shared, false),
            Some("view_mut") => (language_core::ReferenceAccess::Mutable, false),
            Some("view_resident_mut") => (language_core::ReferenceAccess::Mutable, true),
            _ => return Ok(None),
        };
        let sealed = |symbol| {
            program
                .symbols
                .symbol_source_span(symbol)
                .and_then(|span| program.symbols.source_file(span))
                .is_some_and(|source| {
                    symbols::core_source_at(source)
                        == Some(language_core::core_vocabulary::CoreSource::Layout)
                })
        };
        if !sealed(machine.symbol)
            || !sealed(machine.attached_data_symbol)
            || program.symbols.name(machine.attached_data_symbol) != "Access"
        {
            return Ok(None);
        }
        let malformed =
            |reason: &str| Diagnostic::error(format!("sealed borrowed View prototype: {reason}"));
        let [policy, schema] = program.machine_type_parameters(machine) else {
            return Err(malformed("must declare exactly two type parameters"));
        };
        if !machine.is_public
            || !matches!(policy.kind, resolved::data::TypeParameterKind::Type)
            || !matches!(schema.kind, resolved::data::TypeParameterKind::Type)
            || !machine.lifetime_parameters.is_empty()
            || machine.body_is_present
            || machine.spelling.is_some()
            || machine.target.is_some()
            || machine.authored_target.is_some()
            || machine.suspends
            || machine.blocks
            || machine.has_structural_type_equations
            || machine.termination_plan
                != (language_semantics::MachineTerminationPlan {
                    interface: language_semantics::TerminationInterface::Published(
                        language_semantics::TerminationGuarantee::NoGuarantee,
                    ),
                    ..Default::default()
                })
            || machine.service_reach_is_installation_bound
            || !program
                .service_reach_rows
                .services(machine.service_reach_row)
                .is_empty()
            || !machine.invokes.is_empty()
            || !machine.satisfies.is_empty()
            || !machine.conformance_bounds.is_empty()
            || !machine.ranking_subjects.is_empty()
            || machine.supply_mode != language_semantics::MachineSupplyMode::Boundary
        {
            return Err(malformed(
                "noncanonical type binders or declaration/effect clauses",
            ));
        }
        let states = program.machine_state_handles(machine.states);
        let [state] = states else {
            return Err(malformed("must declare exactly one state"));
        };
        let state = program.machine_state(*state);
        let [parameter] = program.state_parameters(state.parameters) else {
            return Err(malformed("must declare exactly one source parameter"));
        };
        if parameter.is_const
            || parameter.is_mutable != (access == language_core::ReferenceAccess::Mutable)
            || parameter.relevance.is_erased()
            || !machine.contracts.is_empty()
            || !state.contracts.is_empty()
        {
            return Err(malformed(
                "source parameter flags or contract clauses differ",
            ));
        }

        let resolved::types::TypeReference::Reference(source) = &parameter.type_reference else {
            return Err(malformed("source parameter must be a reference"));
        };
        if source.access != access || source.lifetime.is_some() {
            return Err(malformed("source reference polarity or lifetime differs"));
        }
        let backing = if resident {
            resident_backing(program, source.referee, policy.symbol, schema.symbol)
            .ok_or_else(|| malformed("resident source must retain exact core Extent, Granted and Resident<P, T> declarations"))?
        } else {
            if !matches!(program.child_type_reference(source.referee), resolved::types::TypeReference::Named { symbol, .. } if *symbol == schema.symbol)
            {
                return Err(malformed(
                    "source reference must name exactly the schema formal",
                ));
            }
            typed::expression::AccessBorrowBacking::Ordinary
        };
        let Some(resolved::types::TypeReference::Reference(result)) = &state.return_type else {
            return Err(malformed("result must be a reference"));
        };
        if result.access != access || result.lifetime.is_some() {
            return Err(malformed("result reference polarity or lifetime differs"));
        }
        let resolved::types::TypeReference::Generic(view) =
            program.child_type_reference(result.referee)
        else {
            return Err(malformed(
                "result referent must retain the formal Access application",
            ));
        };
        let [result_policy, result_schema] = program.child_type_references(view.arguments) else {
            return Err(malformed(
                "result Access application must have two arguments",
            ));
        };
        if view.base_symbol != machine.attached_data_symbol
            || !view.lifetime_arguments.is_empty()
            || !matches!(result_policy, resolved::types::TypeReference::Named { symbol, .. } if *symbol == policy.symbol)
            || !matches!(result_schema, resolved::types::TypeReference::Named { symbol, .. } if *symbol == schema.symbol)
        {
            return Err(malformed(
                "result does not preserve exact policy/schema formal identities",
            ));
        }
        Ok(Some(typed::expression::AccessBorrowOperationRequest {
            factory: machine.symbol,
            access,
            backing,
        }))
    }

    fn resident_backing(
        program: &resolved::SymbolResolvedTrees,
        source: arena::Handle<resolved::types::TypeReference>,
        policy: symbols::SymbolHandle,
        schema: symbols::SymbolHandle,
    ) -> Option<typed::expression::AccessBorrowBacking> {
        use resolved::types::{TypeConstraint, TypeReference};
        let TypeReference::Constrained(qualified) = program.child_type_reference(source) else {
            return None;
        };
        let TypeReference::Named { symbol: extent, .. } =
            program.child_type_reference(qualified.base_type)
        else {
            return None;
        };
        if program.symbols.core_vocabulary(*extent)
            != Some(language_core::core_vocabulary::CoreVocabulary::Extent)
        {
            return None;
        }
        let domain = |name: &str| {
            let mut definitions = program.domain_definitions.iter().filter(|definition| {
            program
                .symbols
                .core_vocabulary(definition.symbol)
                .is_some_and(|vocabulary| vocabulary.is_extent_domain_named(name))
                && matches!(&definition.target_type, TypeReference::Named { symbol, .. } if symbol == extent)
        });
            let definition = definitions.next()?;
            definitions.next().is_none().then_some(definition)
        };
        let granted = domain("Granted")?;
        let resident = domain("Resident")?;
        let constraints = type_constraints(program).span(qualified.constraints)?;
        if constraints.len() != 2 {
            return None;
        }
        let names = |name: &str, definition: &resolved::domain::DomainDefinition| {
            let declared = definition.name.as_str();
            name.strip_prefix("Extent::").unwrap_or(name)
                == declared.strip_prefix("Extent::").unwrap_or(declared)
        };
        let mut has_granted = false;
        let mut has_resident = false;
        for constraint in constraints {
            match constraint {
                TypeConstraint::Named(name) if names(name.as_str(), granted) && !has_granted => {
                    has_granted = true;
                }
                TypeConstraint::Domain(application)
                    if names(application.name.as_str(), granted)
                        && application.arguments.is_empty()
                        && !has_granted =>
                {
                    has_granted = true;
                }
                TypeConstraint::Domain(application)
                    if names(application.name.as_str(), resident) && !has_resident =>
                {
                    let [actual_policy, actual_schema] =
                        program.child_type_references(application.arguments)
                    else {
                        return None;
                    };
                    if !matches!(actual_policy, TypeReference::Named { symbol, .. } if *symbol == policy)
                        || !matches!(actual_schema, TypeReference::Named { symbol, .. } if *symbol == schema)
                    {
                        return None;
                    }
                    has_resident = true;
                }
                _ => return None,
            }
        }
        (has_granted && has_resident).then_some(typed::expression::AccessBorrowBacking::Resident {
            extent: *extent,
            granted: granted.symbol,
            resident: resident.symbol,
        })
    }
}

pub(crate) mod tables {
    use arena::Arena;
    use symbol_resolved_trees::SymbolResolvedTrees;
    use symbol_resolved_trees::expression::ExpressionTable as ResolvedExpressions;
    use symbol_resolved_trees::statement::StatementTable as ResolvedStatements;
    use symbol_resolved_trees::types::{TypeConstraint, TypeReferenceTable};
    use typed_trees::TypedTreeTables;
    use typed_trees::expression::ExpressionTable;
    use typed_trees::statement::StatementTable;

    pub(crate) fn expression_table(tables: &TypedTreeTables) -> &ExpressionTable {
        &tables.expression_table
    }

    pub(crate) fn expression_table_mut(tables: &mut TypedTreeTables) -> &mut ExpressionTable {
        &mut tables.expression_table
    }

    pub(crate) fn statement_table_mut(tables: &mut TypedTreeTables) -> &mut StatementTable {
        &mut tables.statement_table
    }

    pub(crate) fn resolved_expressions(source: &SymbolResolvedTrees) -> &ResolvedExpressions {
        &source.tables.bodies.expressions
    }

    pub(crate) fn resolved_statements(source: &SymbolResolvedTrees) -> &ResolvedStatements {
        &source.tables.bodies.statements
    }

    pub(crate) fn type_constraints(source: &SymbolResolvedTrees) -> &Arena<TypeConstraint> {
        &source.tables.types.constraints
    }

    pub(crate) fn type_references(source: &SymbolResolvedTrees) -> &TypeReferenceTable {
        &source.tables.types.references
    }
}

pub use access_borrow_factory::{access_borrow_factory, access_borrow_factory_of};
pub use lowerer::lower_symbol_resolved_trees;
pub use lowerer::seeded_continuation::{
    SeededContinuationError, SeededTypingBase, lower_seeded_extension,
    lower_symbol_resolved_trees_to_seeded_base,
};
