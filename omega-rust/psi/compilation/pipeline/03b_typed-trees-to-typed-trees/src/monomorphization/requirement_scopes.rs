//! Retain original reachable interface applications for one selected member.
//!
//! Calling retains the selected tuple and consumes the authored policy owner's
//! induced tuple. Public facts and provider correspondence consume the declaring
//! tuple. One original scope traversal and graph copier retain these associations
//! before public header publication; realization never reconstructs them from
//! spelling, private bodies or normalized shape. A missing independent evidence
//! association stays unavailable for its consumer instead of becoming authority.

use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::data::TypeParameterKind;
use typed_trees::static_applications::{
    RequirementInterfaceApplication, StaticMachineApplicationHandle, StaticMachineBinding,
    StaticMachineBindingValue, StaticMachineLifetimeBinding,
};
use typed_trees::trait_definition::AppliedTraitScope;
use typed_trees::types::TypeReferenceHandle;

pub(super) struct ProjectedRequirementScopes {
    pub(super) applications: Vec<RequirementInterfaceApplication>,
    pub(super) declaring_application: StaticMachineApplicationHandle,
}

pub(super) fn project(
    source: &mut TypedTrees,
    application: StaticMachineApplicationHandle,
    declaring_trait: SymbolHandle,
    selected_interface: SymbolHandle,
    arguments: &[TypeReferenceHandle],
    bindings: &[StaticMachineBinding],
) -> Result<ProjectedRequirementScopes, Diagnostic> {
    if selected_interface == declaring_trait {
        // A directly declared member has no ancestor declaration owner. Keep
        // its existing tuple without copying an immutable prefix for projection.
        return Ok(ProjectedRequirementScopes {
            applications: vec![RequirementInterfaceApplication {
                interface: selected_interface,
                binding_application: application,
            }],
            declaring_application: application,
        });
    }
    // One immutable original prefix supplies every scope. Appended graphs never
    // become another scope's caller operands or change simultaneous substitution.
    let original = source.clone();
    let selected = original
        .trait_definition_by_symbol(selected_interface)
        .ok_or_else(|| Diagnostic::error("inherited application lost its selected interface"))?;
    let root =
        AppliedTraitScope::from_static_application(&original, selected, application, arguments)
            .ok_or_else(|| {
                Diagnostic::error("inherited application lost its complete selected tuple")
            })?;
    root.declaring_scope(&original, declaring_trait)
        .ok_or_else(|| {
            Diagnostic::error("inherited requirement has no one exact declaring application")
        })?;
    let mut interfaces = Vec::new();
    root.visit(&original, &mut |scope| {
        if scope.declaration.is_boundary
            && scope.declaring_scope(&original, declaring_trait).is_some()
            && !interfaces.contains(&scope.declaration.symbol)
        {
            interfaces.push(scope.declaration.symbol);
        }
    });
    let mut applications = Vec::new();
    let mut declaring_application = StaticMachineApplicationHandle::invalid();
    for interface in interfaces {
        let scope = root
            .declaring_scope(&original, interface)
            .ok_or_else(|| Diagnostic::error("inherited interface has no one exact application"))?;
        let bound_correspondence = scope.declaration.conformance_bounds.iter().all(|bound| {
            original
                .trait_type_parameters(scope.declaration)
                .iter()
                .any(|parameter| parameter.symbol == bound.binder)
                || bindings.iter().any(|row| row.parameter == bound.binder)
        });
        let binding_application = if interface == selected_interface {
            application
        } else if !bound_correspondence {
            // An unused ancestor must not create a new eager source refusal.
            // Consumers that need its contract must demand genuine evidence;
            // absence here cannot authorize ordinary-native policy selection.
            StaticMachineApplicationHandle::invalid()
        } else {
            let regions = root
                .declaring_lifetime_ordinals(&original, interface)
                .ok_or_else(|| {
                    Diagnostic::error("inherited interface lost its exact region correspondence")
                })?;
            project_interface(
                &original,
                source,
                application,
                selected,
                &scope,
                regions,
                bindings,
            )?
        };
        if interface == declaring_trait {
            declaring_application = binding_application;
        }
        applications.push(RequirementInterfaceApplication {
            interface,
            binding_application,
        });
    }
    if !declaring_application.is_valid() {
        return Err(Diagnostic::error(
            "inherited application lost its original declaring evidence binder",
        ));
    }
    Ok(ProjectedRequirementScopes {
        applications,
        declaring_application,
    })
}

fn project_interface(
    original: &TypedTrees,
    source: &mut TypedTrees,
    application: StaticMachineApplicationHandle,
    selected: &typed_trees::trait_definition::TraitDefinition,
    scope: &AppliedTraitScope<'_>,
    region_ordinals: Vec<u32>,
    bindings: &[StaticMachineBinding],
) -> Result<StaticMachineApplicationHandle, Diagnostic> {
    let mut projected = Vec::new();
    for parameter in original.trait_type_parameters(scope.declaration) {
        let reference = scope.parameter_argument(original, parameter.symbol);
        if !reference.is_valid() {
            return Err(Diagnostic::error(
                "inherited application lost an original declaring operand",
            ));
        }
        let reference = super::signature_cloning::copy_scoped_type_reference(
            original, source, reference, scope,
        )?;
        let value = match &parameter.kind {
            TypeParameterKind::Type => StaticMachineBindingValue::Type(reference),
            TypeParameterKind::Const { .. } | TypeParameterKind::Value { .. } => {
                StaticMachineBindingValue::Const(reference)
            }
            TypeParameterKind::Machine { .. } | TypeParameterKind::Implementation => {
                let argument = super::application_arguments::static_argument_from_type_reference(
                    source, reference,
                )
                .ok_or_else(|| {
                    Diagnostic::error(
                        "inherited declaring operand has no retained typed application",
                    )
                })?;
                if matches!(parameter.kind, TypeParameterKind::Machine { .. }) {
                    StaticMachineBindingValue::Machine(argument)
                } else {
                    StaticMachineBindingValue::Conformance(argument)
                }
            }
            TypeParameterKind::Proposition { .. } => {
                return Err(Diagnostic::error(
                    "inherited application has no retained proposition operand",
                ));
            }
        };
        projected.push(StaticMachineBinding {
            parameter: parameter.symbol,
            value,
        });
    }
    // Independent bound evidence must have an original binder association.
    // A matching trait or public shape is not a substitute for that receipt.
    for bound in &scope.declaration.conformance_bounds {
        if projected.iter().any(|row| row.parameter == bound.binder) {
            continue;
        }
        let binding = bindings
            .iter()
            .find(|row| row.parameter == bound.binder)
            .ok_or_else(|| {
                Diagnostic::error(
                    "inherited application lost its original declaring evidence binder",
                )
            })?;
        projected.push(binding.clone());
    }
    let original_application = original
        .expression_table
        .static_machine_application(application);
    let (_, signature) = original
        .trait_machine_signature(original_application.binding_declaration)
        .ok_or_else(|| Diagnostic::error("inherited application lost its original member"))?;
    for parameter in original.state_signature_type_parameters(signature) {
        let binding = bindings
            .iter()
            .find(|row| row.parameter == parameter.symbol)
            .ok_or_else(|| {
                Diagnostic::error("inherited application lost an original member operand")
            })?;
        projected.push(binding.clone());
    }
    let lifetimes = project_lifetimes(
        original,
        application,
        selected,
        scope.declaration,
        &region_ordinals,
    )?;
    let projected_application = source.expression_table.retain_static_machine_application(
        original_application.registration_operation,
        signature.name.clone(),
        &[],
    );
    let retained = source
        .expression_table
        .static_machine_application_mut(projected_application);
    retained.binding_declaration = original_application.binding_declaration;
    retained.selected_interface = scope.declaration.symbol;
    retained.lexical_owner = original_application.lexical_owner;
    retained.captured_lexical_scope = original_application.captured_lexical_scope;
    source
        .expression_table
        .set_static_machine_application_bindings(projected_application, &projected);
    source
        .expression_table
        .set_static_machine_application_lifetimes(projected_application, &lifetimes);
    Ok(projected_application)
}

/// Captured regions follow the exact inherited ordinal map. Invocation regions
/// retain their independent original member tail, including shadow priority.
/// Names identify each declaration's binders only after that association.
pub(super) fn project_lifetimes(
    original: &TypedTrees,
    application: StaticMachineApplicationHandle,
    selected: &typed_trees::trait_definition::TraitDefinition,
    declaring: &typed_trees::trait_definition::TraitDefinition,
    region_ordinals: &[u32],
) -> Result<Vec<StaticMachineLifetimeBinding>, Diagnostic> {
    let retained = original
        .expression_table
        .static_machine_application(application);
    let (_, signature) = original
        .trait_machine_signature(retained.binding_declaration)
        .ok_or_else(|| Diagnostic::error("inherited region projection lost its original member"))?;
    let original_lifetimes = original
        .expression_table
        .static_machine_application_lifetimes(application);
    let captured_count = selected.lifetime_parameters.len();
    if declaring.lifetime_parameters.len() != region_ordinals.len()
        || !(original_lifetimes.len() == captured_count
            || original_lifetimes.len() == captured_count + signature.lifetime_parameters.len())
        || selected
            .lifetime_parameters
            .iter()
            .chain(&signature.lifetime_parameters)
            .zip(original_lifetimes)
            .any(|(parameter, row)| parameter != &row.parameter)
    {
        return Err(Diagnostic::error(
            "inherited application lost its captured/invocation region correspondence",
        ));
    }
    let mut lifetimes = Vec::new();
    for (parameter, ordinal) in declaring.lifetime_parameters.iter().zip(region_ordinals) {
        let row = usize::try_from(*ordinal)
            .ok()
            .filter(|ordinal| *ordinal < captured_count)
            .and_then(|ordinal| original_lifetimes.get(ordinal))
            .ok_or_else(|| {
                Diagnostic::error("inherited application lost a selected captured region")
            })?;
        lifetimes.push(StaticMachineLifetimeBinding {
            parameter: parameter.clone(),
            argument: row.argument.clone(),
        });
    }
    lifetimes.extend(original_lifetimes.iter().skip(captured_count).cloned());
    Ok(lifetimes)
}
