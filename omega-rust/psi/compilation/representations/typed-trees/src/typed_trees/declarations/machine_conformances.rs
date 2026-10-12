//! Source satisfaction edges and their producer-owned outer applications.
//!
//! Executable copies preserve the universal edge verbatim. Its original family
//! owns the member telescope. A carrier-derived header retains its independent
//! member binders and their original source correspondence; each completed call
//! separately retains the outer argument graph and lexical region issuer. Full
//! member custody authenticates projection. Only the exact satisfaction's
//! contract captures and genuine attachment coordinates narrow a provider row;
//! its remaining invocation telescope stays universal.
//! This correspondence neither selects
//! a provider nor turns a concrete member invocation into a new source row.

use crate::TypedTrees;
use crate::machine::{Machine, TraitConformance};
use crate::trait_definition::{AppliedTraitScope, AppliedTraitScopePayload};
use arena::Handle;
use diagnostics::Diagnostic;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MachineConformanceApplication {
    pub source: Handle<TraitConformance>,
    pub instance: Handle<TraitConformance>,
    /// The source occurrence admitting the carrier-derived member telescope.
    pub occurrence: crate::static_applications::StaticMachineApplicationHandle,
    /// Complete original-family projection owned by this occurrence.
    pub application: crate::static_applications::StaticMachineApplicationHandle,
    pub provider: symbols::SymbolHandle,
    pub scope: AppliedTraitScopePayload,
}

pub struct MachineConformanceSource<'program> {
    pub family: &'program Machine,
    pub edge: &'program TraitConformance,
    pub source: Handle<TraitConformance>,
    pub provider: symbols::SymbolHandle,
    pub scope: AppliedTraitScope<'program>,
}

impl TypedTrees {
    /// Reuse the retained specialization's source declaration, never a generated
    /// name or its consumed live telescope. Conflicting origins are unavailable.
    pub fn authored_conformance_machine<'program>(
        &'program self,
        machine: &'program Machine,
    ) -> Result<&'program Machine, Diagnostic> {
        if machine.generic_data_template.is_valid() {
            return self
                .machine_by_symbol(machine.generic_data_template)
                .ok_or_else(|| {
                    Diagnostic::error("generic data method lost its exact authored family")
                });
        }
        if !self.machine_type_parameters(machine).is_empty()
            || self.machine_trait_conformances(machine).is_empty()
        {
            return Ok(machine);
        }
        let mut applications = self
            .machine_specializations
            .iter()
            .filter(|application| application.instance == machine.symbol);
        let Some(application) = applications.next() else {
            return Ok(machine);
        };
        let template = self.machines().iter().find(|template| {
            template.symbol == application.template
                && template.symbol != machine.symbol
                && template.type_parameters == application.template_parameters
                && !self.machine_type_parameters(template).is_empty()
                && self.machine_trait_conformances(template)
                    == self.machine_trait_conformances(machine)
        });
        template.filter(|_| applications.next().is_none()).ok_or_else(|| {
            Diagnostic::error(format!(
                "machine `{}` generic satisfaction lost its exact original family and application",
                machine.name,
            ))
        })
    }

    /// Publish a complete occurrence after the source-call producer retains its
    /// receiving graph and declaration bindings. Both early carrier synthesis
    /// and executable specialization use this same machine-owned span.
    pub fn retain_machine_conformance_application(
        &mut self,
        instance: symbols::SymbolHandle,
        application: crate::static_applications::StaticMachineApplicationHandle,
    ) {
        let Some(machine) = self.machine_by_symbol(instance) else {
            return;
        };
        let Ok(family) = self.authored_conformance_machine(machine) else {
            return;
        };
        if !self
            .expression_table
            .static_machine_application_is_valid(application)
        {
            return;
        }
        let family_symbol = family.symbol;
        let existing = self
            .machine_conformance_applications
            .span_or_empty(machine.conformance_applications)
            .iter()
            .find(|row| row.occurrence == application)
            .map(|row| row.application);
        let occurrence = application;
        let Some(application) =
            self.project_conformance_application(instance, occurrence, existing)
        else {
            // A changed occurrence cannot leave a previous complete projection
            // available after its authentic operand or receiving custody is lost.
            if existing.is_some() {
                let machine = self.machine_by_symbol(instance).expect("retained machine");
                let rows = self
                    .machine_conformance_applications
                    .span_or_empty(machine.conformance_applications)
                    .iter()
                    .filter(|row| row.occurrence != occurrence)
                    .cloned()
                    .collect::<Vec<_>>();
                let rows = self.machine_conformance_applications.insert_many(rows);
                self.machines_mut()
                    .iter_mut()
                    .find(|machine| machine.symbol == instance)
                    .expect("retained machine")
                    .conformance_applications = rows;
            }
            return;
        };
        let machine = self.machine_by_symbol(instance).expect("retained machine");
        let family = self
            .machine_by_symbol(family_symbol)
            .expect("retained source family");
        let mut rows = self
            .machine_conformance_applications
            .span_or_empty(machine.conformance_applications)
            .to_vec();
        let insertion = rows
            .iter()
            .position(|row| row.occurrence == occurrence)
            .unwrap_or(rows.len());
        rows.retain(|row| row.occurrence != occurrence);
        let mut updated = Vec::new();
        for (ordinal, edge) in self.machine_trait_conformances(family).iter().enumerate() {
            let Some(instance_edge) = self.machine_trait_conformances(machine).get(ordinal) else {
                continue;
            };
            if edge.symbol != instance_edge.symbol
                || edge.requirement_symbol != instance_edge.requirement_symbol
            {
                continue;
            }
            let Some(scope) = AppliedTraitScope::from_completed_machine_conformance(
                self,
                family,
                edge,
                application,
            ) else {
                continue;
            };
            updated.push(MachineConformanceApplication {
                source: family.satisfies.handle_at(ordinal as u32),
                instance: machine.satisfies.handle_at(ordinal as u32),
                occurrence,
                application,
                provider: machine.attached_data_symbol,
                scope: scope.retain_payload(self),
            });
        }
        rows.splice(insertion..insertion, updated);
        if rows
            == self
                .machine_conformance_applications
                .span_or_empty(machine.conformance_applications)
        {
            return;
        }
        let rows = self.machine_conformance_applications.insert_many(rows);
        self.machines_mut()
            .iter_mut()
            .find(|machine| machine.symbol == instance)
            .expect("retained machine")
            .conformance_applications = rows;
    }

    /// Join the carrier's authored attachment and independent member map under
    /// one actual call. This is a binding projection, never an executable
    /// specialization receipt or a shared storage representative application.
    fn project_conformance_application(
        &mut self,
        instance: symbols::SymbolHandle,
        occurrence: crate::static_applications::StaticMachineApplicationHandle,
        existing: Option<crate::static_applications::StaticMachineApplicationHandle>,
    ) -> Option<crate::static_applications::StaticMachineApplicationHandle> {
        use crate::static_applications::{StaticApplicationDeclaration, StaticMachineBinding};
        let machine = self.machine_by_symbol(instance)?;
        let family = self.authored_conformance_machine(machine).ok()?;
        if self.machine_trait_conformances(family).is_empty() {
            return None;
        }
        let retained = self
            .expression_table
            .static_machine_application(occurrence)
            .clone();
        if retained.binding_declaration == family.symbol {
            return Some(occurrence);
        }
        let header = self.machine_by_symbol(retained.binding_declaration)?;
        if header.generic_data_template != family.symbol
            || header.attached_data_symbol != machine.attached_data_symbol
        {
            return None;
        }
        let source_members = self
            .data_type_parameters
            .span_or_empty(header.generic_data_member_parameters);
        let declaration = StaticApplicationDeclaration::for_symbol(self, header.symbol).ok()?;
        if source_members.len() != declaration.parameters().len() {
            return None;
        }
        // The ordinary producer must owe a complete derived-header tuple before
        // its operands can issue an original-family projection.
        let operands = declaration.bind_arguments(self, occurrence).ok()?;
        let enclosing = self
            .type_reference_table
            .type_reference_handles(retained.enclosing_data_applications);
        let mut bindings = attached_family_bindings(self, family, enclosing)?;
        for (parameter, source) in declaration.parameters().iter().zip(source_members) {
            if !self
                .machine_type_parameters(family)
                .iter()
                .any(|original| original.symbol == source.symbol)
            {
                return None;
            }
            let operand = operands
                .iter()
                .find(|binding| binding.parameter == parameter.symbol)?;
            let projected = StaticMachineBinding {
                parameter: source.symbol,
                value: operand.value.clone(),
            };
            if bindings
                .iter()
                .any(|binding| binding.parameter == projected.parameter)
            {
                return None;
            }
            bindings.push(projected);
        }
        if header.conformance_bounds.len() != family.conformance_bounds.len() {
            return None;
        }
        for (bound, source) in header
            .conformance_bounds
            .iter()
            .zip(&family.conformance_bounds)
        {
            if bound.trait_symbol != source.trait_symbol {
                return None;
            }
            if bindings
                .iter()
                .any(|binding| binding.parameter == source.binder)
            {
                continue;
            }
            let operand = operands
                .iter()
                .find(|binding| binding.parameter == bound.binder)?;
            bindings.push(StaticMachineBinding {
                parameter: source.binder,
                value: operand.value.clone(),
            });
        }
        let (state_ordinal, parameter_ordinal) = self
            .machine_states(header)
            .iter()
            .enumerate()
            .find_map(|(state_ordinal, state)| {
                self.state_parameters(state)
                    .iter()
                    .position(|parameter| parameter.symbol == retained.receiving_data_parameter)
                    .map(|parameter_ordinal| (state_ordinal, parameter_ordinal))
            })?;
        let receiving_parameter = self
            .machine_states(family)
            .get(state_ordinal)
            .and_then(|state| self.state_parameters(state).get(parameter_ordinal))?
            .symbol;
        let family_symbol = family.symbol;
        let target = family.name.clone();
        let application = existing.unwrap_or_else(|| {
            self.expression_table
                .retain_static_machine_application(family_symbol, target, &[])
        });
        let projected = self
            .expression_table
            .static_machine_application_mut(application);
        projected.lexical_owner = retained.lexical_owner;
        projected.captured_lexical_scope = retained.captured_lexical_scope;
        projected.binding_declaration = family_symbol;
        self.expression_table
            .set_static_machine_application_receiving_data_parameter(
                application,
                receiving_parameter,
            );
        self.expression_table
            .set_static_machine_application_enclosing_data_applications(
                application,
                retained.enclosing_data_applications,
            );
        self.expression_table
            .set_static_machine_application_bindings(application, &bindings);
        Some(application)
    }

    /// Enumerate occurrence-owned outer applications for a shared executable
    /// body. Repeated member calls do not supply additional source edges;
    /// consumers compare the complete outer scope before collecting a row.
    pub fn machine_conformance_sources<'program>(
        &'program self,
        machine: &'program Machine,
        conformance: Handle<TraitConformance>,
    ) -> Vec<MachineConformanceSource<'program>> {
        let Some(ordinal) = self
            .machine_trait_conformances(machine)
            .iter()
            .enumerate()
            .find(|(ordinal, _)| machine.satisfies.handle_at(*ordinal as u32) == conformance)
            .map(|(ordinal, _)| ordinal)
        else {
            return Vec::new();
        };
        let Ok(family) = self.authored_conformance_machine(machine) else {
            return Vec::new();
        };
        let Some(edge) = self.machine_trait_conformances(family).get(ordinal) else {
            return Vec::new();
        };
        if edge.symbol != self.machine_trait_conformances(machine)[ordinal].symbol
            || edge.requirement_symbol
                != self.machine_trait_conformances(machine)[ordinal].requirement_symbol
        {
            return Vec::new();
        }
        let source = family.satisfies.handle_at(ordinal as u32);
        if family.symbol == machine.symbol && machine.conformance_applications.is_empty() {
            return AppliedTraitScope::from_machine_conformance(self, family, edge)
                .into_iter()
                .map(|scope| MachineConformanceSource {
                    family,
                    edge,
                    source,
                    provider: machine.attached_data_symbol,
                    scope,
                })
                .collect();
        }
        self.machine_conformance_applications
            .span_or_empty(machine.conformance_applications)
            .iter()
            .filter(|application| {
                application.instance == conformance && application.source == source
            })
            .filter_map(|application| {
                if application.provider != machine.attached_data_symbol
                    || !self
                        .expression_table
                        .static_machine_application_is_valid(application.application)
                    || self
                        .expression_table
                        .static_machine_application(application.application)
                        .binding_declaration
                        != family.symbol
                {
                    return None;
                }
                let scope = application.scope.bind(self)?;
                (scope.declaration.symbol == edge.symbol).then_some(MachineConformanceSource {
                    family,
                    edge,
                    source,
                    provider: application.provider,
                    scope,
                })
            })
            .collect()
    }
}

/// Project the source attachment's exact binder correspondence. This does not
/// invent an instance tuple. Independently owned member operands are supplied
/// separately by the authenticated derived-header correspondence.
fn attached_family_bindings(
    program: &TypedTrees,
    family: &crate::machine::Machine,
    enclosing: &[crate::types::TypeReferenceHandle],
) -> Option<Vec<crate::static_applications::StaticMachineBinding>> {
    use crate::static_applications::{StaticMachineBinding, StaticMachineBindingValue};
    use crate::types::TypeReferenceNode;
    if !program
        .type_reference_table
        .contains_type_reference(family.attached_data_application)
    {
        return None;
    }
    let TypeReferenceNode::Generic {
        base_symbol,
        arguments,
        ..
    } = program
        .type_reference_table
        .type_reference(family.attached_data_application)
    else {
        return None;
    };
    let formal = program
        .type_reference_table
        .type_reference_handles(*arguments);
    let actual = enclosing.iter().rev().find_map(|reference| {
        match program.type_reference_table.type_reference(*reference) {
            TypeReferenceNode::Generic {
                base_symbol: actual,
                arguments,
                ..
            } if actual == base_symbol => Some(
                program
                    .type_reference_table
                    .type_reference_handles(*arguments),
            ),
            _ => None,
        }
    })?;
    if formal.len() != actual.len() {
        return None;
    }
    let mut bindings = Vec::new();
    for (formal, actual) in formal.iter().zip(actual) {
        let TypeReferenceNode::Named { symbol, .. } =
            program.type_reference_table.type_reference(*formal)
        else {
            return None;
        };
        let Some(parameter) = program
            .machine_type_parameters(family)
            .iter()
            .find(|parameter| parameter.symbol == *symbol)
        else {
            // A data-owned binder resolves in the preceding enclosing scope;
            // it must not be manufactured as a member-owned binder.
            if program
                .data_definition_by_symbol(*base_symbol)
                .is_some_and(|data| {
                    program
                        .data_type_parameters(data)
                        .iter()
                        .any(|parameter| parameter.symbol == *symbol)
                })
            {
                continue;
            }
            return None;
        };
        let value = match parameter.kind {
            crate::data::TypeParameterKind::Type => StaticMachineBindingValue::Type(*actual),
            crate::data::TypeParameterKind::Const { .. } => {
                StaticMachineBindingValue::Const(*actual)
            }
            _ => return None,
        };
        let binding = StaticMachineBinding {
            parameter: *symbol,
            value,
        };
        if let Some(previous) = bindings
            .iter()
            .find(|previous: &&StaticMachineBinding| previous.parameter == *symbol)
        {
            if previous != &binding {
                return None;
            }
        } else {
            bindings.push(binding);
        }
    }
    Some(bindings)
}
