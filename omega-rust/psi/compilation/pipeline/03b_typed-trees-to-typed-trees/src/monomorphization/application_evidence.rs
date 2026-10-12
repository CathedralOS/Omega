//! Exact selected evidence substitutions shared by public headers and bodies.
//!
//! The original declaration bounds and retained application rows own this
//! operation. Executable candidate bookkeeping is not a declaration owner:
//! a public trait signature must consume the same selected conformance closure
//! without manufacturing a Machine candidate. Rewrites select exact authored
//! rows and retain their complete application commitment and dispatch identity;
//! they import no private implementation body fact.

use crate::monomorphization::{StateSignature, StaticMachineArgument, SymbolHandle};
use typed_trees::machine::GenericConformanceBound;
use typed_trees::static_applications::{StaticMachineBinding, StaticMachineBindingValue};

pub(crate) struct EvidenceArgumentRewrite {
    pub(crate) binder: SymbolHandle,
    pub(crate) argument: StaticMachineArgument,
    pub(crate) alias: typed_trees::name::Identifier,
}

pub(crate) fn evidence_argument_rewrites(
    program: &dyn ::validation::CheckingInput,
    bindings: &[StaticMachineBinding],
) -> Vec<EvidenceArgumentRewrite> {
    bindings
        .iter()
        .filter_map(|binding| {
            let binder = binding.parameter;
            let StaticMachineBindingValue::Conformance(binding) = &binding.value else {
                return None;
            };
            let alias = if program.symbols.get(binding.symbol).kind
                == symbols::SymbolKind::ConformanceParameter
            {
                typed_trees::name::Identifier::generated(program.symbols.name(binding.symbol))
            } else {
                let selected = program.conformance_by_symbol(binding.symbol)?;
                selected.alias.clone().unwrap_or_else(|| {
                    typed_trees::name::Identifier::generated("<unnamed-conformance>")
                })
            };
            Some(EvidenceArgumentRewrite {
                binder,
                argument: binding.clone(),
                alias,
            })
        })
        .collect()
}

/// Result-only copies and executable body copies carry the same selected
/// algebra. Substitute its exact provider with its diagnostic alias together;
/// renaming a forwarded evidence binder must not leave the callee's authority.
pub(crate) fn rewrite_index_algebra(
    program: &mut typed_trees::TypedTrees,
    expression: typed_trees::expression::ExpressionHandle,
    rewrites: &[EvidenceArgumentRewrite],
    static_rewrites: &[(SymbolHandle, StaticMachineArgument)],
) {
    if program
        .expression_table
        .index_expression_authority(expression)
        .is_none()
    {
        return;
    }
    let authority = program
        .expression_table
        .index_expression_authority_mut(expression);
    let Some(operation) = &mut authority.operation else {
        return;
    };
    if let Some(application) = &mut operation.application {
        super::body_rewriting::substitute_forwarded_machine_arguments(
            std::slice::from_mut(application),
            static_rewrites,
            &[],
        );
    }
    if let Some(rewrite) = rewrites
        .iter()
        .find(|rewrite| rewrite.binder == operation.provider)
    {
        operation.apply_provider(&rewrite.argument, rewrite.alias.as_str());
    }
}

pub(crate) struct EvidenceRequirementRewrite {
    pub(crate) placeholder: SymbolHandle,
    pub(crate) target: SymbolHandle,
    pub(crate) name: typed_trees::name::Identifier,
    pub(crate) application_arguments: Box<[StaticMachineArgument]>,
    pub(crate) dispatch: typed_trees::typed_trees::StaticRequirementDispatch,
}

pub(crate) fn evidence_requirement_rewrites(
    program: &dyn ::validation::CheckingInput,
    bounds: &[GenericConformanceBound],
    bindings: &[StaticMachineBinding],
) -> Vec<EvidenceRequirementRewrite> {
    let mut rewrites = Vec::new();
    for parameter in bounds {
        let binder = parameter.binder;
        let Some(StaticMachineBinding {
            value: StaticMachineBindingValue::Conformance(binding),
            ..
        }) = bindings.iter().find(|binding| binding.parameter == binder)
        else {
            continue;
        };
        let Some(trait_definition) = program.trait_definition_by_symbol(parameter.trait_symbol)
        else {
            continue;
        };
        let Some(selected) = program.conformance_by_symbol(binding.symbol) else {
            continue;
        };
        let rows = selected.rows.as_slice();
        let application =
            validation::conformance_applications::close_conformance_application(program, binding)
                .expect("validated selected conformance must close during specialization");
        let mut requirements = Vec::new();
        collect_evidence_requirement_closure(
            program,
            trait_definition,
            &mut Vec::new(),
            &mut requirements,
        );
        let placeholders = program.symbols.child_handles(binder).into_iter().flatten();
        for (placeholder, requirement) in placeholders.zip(requirements) {
            let Some(row) = rows
                .iter()
                .find(|row| row.requirement == requirement.symbol)
            else {
                continue;
            };
            rewrites.push(EvidenceRequirementRewrite {
                placeholder,
                target: row.realization_state,
                name: typed_trees::name::Identifier::generated(
                    row.realization_name
                        .as_str()
                        .rsplit("::")
                        .next()
                        .unwrap_or(row.realization_name.as_str()),
                ),
                application_arguments: binding
                    .application
                    .as_ref()
                    .map_or_else(Box::default, |application| application.arguments.clone()),
                dispatch: typed_trees::typed_trees::StaticRequirementDispatch {
                    application_report_fingerprint: application.report_fingerprint,
                    application_commitment: application.commitment,
                    declaring_trait: row.declaring_trait,
                    requirement: row.requirement,
                    realization_machine: row.realization_machine,
                    realization_state: row.realization_state,
                },
            });
        }
    }
    rewrites
}

pub(crate) fn collect_evidence_requirement_closure<'program>(
    program: &'program dyn ::validation::CheckingInput,
    trait_definition: &'program typed_trees::trait_definition::TraitDefinition,
    visited: &mut Vec<SymbolHandle>,
    output: &mut Vec<&'program StateSignature>,
) {
    if visited.contains(&trait_definition.symbol) {
        return;
    }
    visited.push(trait_definition.symbol);
    output.extend(program.trait_machine_signatures(trait_definition).iter());
    // A transparent refinement declares no requirements of its own; a binder
    // over it selects the base conformance's requirement namespace. This
    // order must mirror the binder placeholder symbols inserted at symbol
    // resolution (own machines, then the refined base, then parents).
    if let Some(base) = &trait_definition.refines
        && let Some(base_trait) = program.trait_definition_by_symbol(base.symbol)
    {
        collect_evidence_requirement_closure(program, base_trait, visited, output);
    }
    for parent in program.trait_requirements(trait_definition) {
        let Some(parent_trait) = program.trait_definition_by_symbol(parent.symbol) else {
            continue;
        };
        collect_evidence_requirement_closure(program, parent_trait, visited, output);
    }
}
