use crate::data::lower_data_definition;
use crate::domain::lower_domain_definition;
use crate::domain_constraints::normalize_domain_constraints;
use crate::invariant::lower_invariant_definition;
use crate::machine::lower_machine;
use crate::operator::lower_operator_definition;
use crate::qualification_casts::normalize_qualification_casts;
use crate::trait_definition::lower_trait_definition;
use omega_core::diagnostics::Diagnostic;
use omega_symbol_resolved_trees::SymbolResolvedTrees;
use omega_typed_trees::TypedTrees;

pub fn lower_symbol_resolved_trees(
    symbol_resolved_trees: &SymbolResolvedTrees,
) -> Result<TypedTrees, Diagnostic> {
    // Decision 11: user-written `==` against bare payload-bearing case names
    // must be rejected BEFORE membership lowering synthesizes its internal
    // tag-equality compares, which are deliberately the same typed shape.
    crate::equality::validate_equality_operands(symbol_resolved_trees)?;

    // Equatable conformance prerequisites error at the conformance item,
    // before any `==` site tries to expand against a malformed type.
    crate::equatable::validate_equatable_conformances(symbol_resolved_trees)?;

    // Exhaustiveness counting over case domains also needs the resolved
    // trees: membership is still a distinct node here, so case arms and
    // domain arms are recognizable before lowering erases them into tag
    // compares and classifier expansions.
    crate::exhaustiveness::validate_case_dispatch_exhaustiveness(symbol_resolved_trees)?;

    let mut lowerer = Lowerer {
        typed_trees: TypedTrees::default(),
        source_trees: symbol_resolved_trees,
        equality_scope: None,
    };
    lowerer.typed_trees.service_reaches = symbol_resolved_trees.service_reaches.clone();
    lowerer.typed_trees.service_reach_rows = symbol_resolved_trees.service_reach_rows.clone();
    lowerer.typed_trees.semantic_domains = symbol_resolved_trees.semantic_domains.clone();

    for invariant_definition in &symbol_resolved_trees.invariant_definitions {
        let invariant_definition = lower_invariant_definition(&mut lowerer, invariant_definition)?;
        lowerer
            .typed_trees
            .push_invariant_definition(invariant_definition);
    }

    for data_definition in &symbol_resolved_trees.data_definitions {
        let data_definition = lower_data_definition(&mut lowerer, data_definition)?;
        lowerer.typed_trees.push_data_definition(data_definition);
    }

    for domain_definition in &symbol_resolved_trees.domain_definitions {
        let domain_definition = lower_domain_definition(&mut lowerer, domain_definition)?;
        lowerer
            .typed_trees
            .push_domain_definition(domain_definition);
    }

    for machine in &symbol_resolved_trees.machines {
        let machine = lower_machine(&mut lowerer, machine)?;
        lowerer.typed_trees.push_machine(machine);
    }

    for measure in &symbol_resolved_trees.measures {
        let measure = crate::measure::lower_measure_definition(&mut lowerer, measure)?;
        lowerer.typed_trees.push_measure(measure);
    }

    for operator in &symbol_resolved_trees.operators {
        let operator = lower_operator_definition(&mut lowerer, operator)?;
        lowerer.typed_trees.push_operator(operator);
    }

    for trait_definition in &symbol_resolved_trees.traits {
        let trait_definition = lower_trait_definition(&mut lowerer, trait_definition)?;
        lowerer.typed_trees.push_trait_definition(trait_definition);
    }

    for conformance in &symbol_resolved_trees.conformances {
        let mut arguments = omega_core::arena::HandleSpan::empty();
        for argument in symbol_resolved_trees
            .tables
            .declarations
            .child_type_references
            .span_or_empty(conformance.arguments)
        {
            let argument =
                crate::type_reference::lower_type_reference_into_table(&mut lowerer, argument)?;
            lowerer
                .typed_trees
                .type_reference_table
                .push_type_reference_handle(&mut arguments, argument);
        }
        let conformance = omega_typed_trees::trait_definition::DataConformance {
            type_name: crate::name::lower_name(&conformance.type_name),
            trait_name: crate::name::lower_name(&conformance.trait_name),
            arguments,
        };
        lowerer.typed_trees.push_data_conformance(conformance);
    }

    for wire_schema in &symbol_resolved_trees.wire_schemas {
        let wire_schema = crate::wire::lower_wire_schema(&mut lowerer, wire_schema)?;
        lowerer.typed_trees.push_wire_schema(wire_schema);
    }

    lowerer.finish()
}

pub fn lower_symbol_resolved_trees_owned(
    symbol_resolved_trees: SymbolResolvedTrees,
) -> Result<TypedTrees, Diagnostic> {
    let mut typed_trees = lower_symbol_resolved_trees(&symbol_resolved_trees)?;
    typed_trees.symbols = symbol_resolved_trees.symbols;
    Ok(typed_trees)
}

pub(crate) struct Lowerer<'source> {
    pub(crate) typed_trees: TypedTrees,
    pub(crate) source_trees: &'source SymbolResolvedTrees,
    /// The value-typing scope of the state body currently being lowered;
    /// `==` expansion uses it to find an operand's data type.
    pub(crate) equality_scope: Option<crate::equatable::EqualityScope>,
}

impl Lowerer<'_> {
    pub(crate) fn finish(mut self) -> Result<TypedTrees, Diagnostic> {
        self.typed_trees.symbols = self.source_trees.symbols.clone();
        let TypedTrees {
            roots,
            tables,
            symbols,
            service_reaches,
            service_reach_rows,
            semantic_domains,
            plan_laid_layouts: _,
            placed_view_plans: _,
            wire_placements: _,
            wire_encode_obligations: _,
            wire_schema_plans: _,
            machine_specializations: _,
            boundary_calling_plans: _,
            open_index_normalizations: _,
        } = self.typed_trees;

        let mut trees = TypedTrees::with_roots(roots, tables, symbols);
        // The copied semantic interners survive the rebuild.
        trees.service_reaches = service_reaches;
        trees.service_reach_rows = service_reach_rows;
        trees.semantic_domains = semantic_domains;
        normalize_domain_constraints(self.source_trees, &mut trees)?;
        normalize_qualification_casts(&mut trees)?;
        Ok(trees)
    }
}

#[cfg(test)]
mod tests;
