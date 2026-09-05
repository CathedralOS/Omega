use super::shared::trait_definition_by_symbol;
use crate::symbols::TopLevelSymbols;
use crate::type_references::{
    TypeReferenceOwner, validate_type_reference_handle_with_type_parameters,
};
use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::operator::trait_operator_operand_signature;
use typed_trees::trait_definition::TraitDefinition;

pub(crate) fn validate_trait_requirements(
    program: &TypedTrees,
    symbols: &TopLevelSymbols<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for trait_definition in program.traits() {
        validate_trait_operator_bindings(program, trait_definition, diagnostics);
        let mut seen = Vec::new();
        for requirement in program.trait_requirements(trait_definition) {
            let Some(required_trait) = trait_definition_by_symbol(program, requirement.symbol)
            else {
                diagnostics.push(Diagnostic::error(format!(
                    "trait `{}` requires unknown trait `{}`",
                    trait_definition.name, requirement.name
                )));
                continue;
            };

            if seen.contains(&requirement.symbol) {
                diagnostics.push(Diagnostic::error(format!(
                    "trait `{}` names parent `{}` more than once",
                    trait_definition.name, requirement.name
                )));
            } else {
                seen.push(requirement.symbol);
            }

            let expected_lifetimes = required_trait.lifetime_parameters.len();
            let actual_lifetimes = requirement.lifetime_arguments.len();
            if actual_lifetimes != 0 && expected_lifetimes != actual_lifetimes {
                diagnostics.push(Diagnostic::error(format!(
                    "trait `{}` parent `{}` expects {expected_lifetimes} lifetime argument(s), got {actual_lifetimes}",
                    trait_definition.name, requirement.name
                )));
            }
            for lifetime in &requirement.lifetime_arguments {
                if !trait_definition
                    .lifetime_parameters
                    .iter()
                    .any(|declared| declared == lifetime)
                {
                    diagnostics.push(Diagnostic::error(format!(
                        "trait `{}` parent `{}` uses undeclared lifetime argument `'{}'",
                        trait_definition.name,
                        requirement.name,
                        lifetime.as_str()
                    )));
                }
            }

            let expected = program.trait_type_parameters(required_trait).len();
            let actual = requirement.arguments.len();
            if expected != actual {
                diagnostics.push(Diagnostic::error(format!(
                    "trait `{}` parent `{}` expects {expected} generic argument(s), got {actual}",
                    trait_definition.name, requirement.name
                )));
            } else {
                super::conformance::validate_trait_application_obligations(
                    program,
                    required_trait,
                    program
                        .type_reference_table
                        .type_reference_handles(requirement.arguments),
                    &trait_definition.conformance_bounds,
                    &format!(
                        "trait `{}` parent `{}`",
                        trait_definition.name, requirement.name
                    ),
                    diagnostics,
                );
            }

            for argument in program
                .type_reference_table
                .type_reference_handles(requirement.arguments)
            {
                validate_type_reference_handle_with_type_parameters(
                    program,
                    *argument,
                    symbols,
                    diagnostics,
                    TypeReferenceOwner::TraitParent {
                        trait_name: trait_definition.name.as_str(),
                        parent: requirement.name.as_str(),
                        generic_depth: 0,
                    },
                    program.trait_type_parameters(trait_definition),
                    &trait_definition.lifetime_parameters,
                );
            }

            if !trait_definition.is_boundary && required_trait.is_boundary {
                diagnostics.push(Diagnostic::error(format!(
                    "ordinary trait `{}` cannot inherit boundary service `{}`; declare the child as `boundary trait`",
                    trait_definition.name, requirement.name
                )));
            }
        }
    }

    let mut reported_cycle_symbols = Vec::new();
    for trait_definition in program.traits() {
        let mut path = Vec::new();
        validate_trait_requirement_cycles(
            program,
            trait_definition,
            &mut path,
            &mut reported_cycle_symbols,
            diagnostics,
        );
    }
}

fn validate_trait_operator_bindings(
    program: &TypedTrees,
    trait_definition: &TraitDefinition,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut seen = Vec::new();
    for requirement in program.trait_machine_signatures(trait_definition) {
        let Some(spelling) = requirement.spelling else {
            continue;
        };
        let operands = trait_operator_operand_signature(program, trait_definition, requirement);
        if seen.iter().any(|(prior_spelling, prior_operands)| {
            *prior_spelling == spelling && prior_operands == &operands
        }) {
            diagnostics.push(Diagnostic::error(format!(
                "trait `{}` binds operator token `{}` more than once for normalized operands `({operands})`; one trait requirement owns each token meaning",
                trait_definition.name,
                spelling.symbol(),
            )));
        } else {
            seen.push((spelling, operands));
        }
    }
}

fn validate_trait_requirement_cycles(
    program: &TypedTrees,
    trait_definition: &TraitDefinition,
    path: &mut Vec<SymbolHandle>,
    reported_cycle_symbols: &mut Vec<SymbolHandle>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if reported_cycle_symbols.contains(&trait_definition.symbol) {
        return;
    }

    if let Some(cycle_start) = path
        .iter()
        .position(|symbol| *symbol == trait_definition.symbol)
    {
        let cycle_symbols = path[cycle_start..]
            .iter()
            .copied()
            .chain(std::iter::once(trait_definition.symbol))
            .collect::<Vec<_>>();
        let mut cycle = path[cycle_start..]
            .iter()
            .filter_map(|symbol| trait_definition_by_symbol(program, *symbol))
            .map(|trait_definition| trait_definition.name.to_string())
            .collect::<Vec<_>>();
        cycle.push(trait_definition.name.to_string());

        diagnostics.push(Diagnostic::error(format!(
            "trait requirement cycle detected: {}",
            cycle.join(" -> ")
        )));
        reported_cycle_symbols.extend(cycle_symbols);
        return;
    }

    path.push(trait_definition.symbol);
    for requirement in program.trait_requirements(trait_definition) {
        let Some(required_trait) = trait_definition_by_symbol(program, requirement.symbol) else {
            continue;
        };

        validate_trait_requirement_cycles(
            program,
            required_trait,
            path,
            reported_cycle_symbols,
            diagnostics,
        );
    }
    path.pop();
}
