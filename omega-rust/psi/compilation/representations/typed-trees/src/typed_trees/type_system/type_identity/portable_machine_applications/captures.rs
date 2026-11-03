//! An applied implementation supplies its member's outer arguments. Keep that
//! application distinct from any later invocation of the member's own binders.

use super::{
    CoordinateTag, IndexIdentityArgument, PortableMachineApplications, TypeIdentityContext, atom,
    compound, conformances, static_binding_operand,
};
use crate::TypedTrees;
use crate::expression::{StaticMachineArgument, StaticSymbolApplication};
use crate::typed_trees::CapturedMachineApplication;

impl PortableMachineApplications {
    pub(super) fn capture(
        &self,
        program: &TypedTrees,
        capture: &CapturedMachineApplication,
        context: &TypeIdentityContext<'_>,
    ) -> Result<String, String> {
        let original = program
            .machine_by_symbol(capture.template)
            .ok_or("captured member lost its original declaration")?;
        let instance = program
            .machine_by_symbol(capture.instance)
            .ok_or("captured member lost its executable declaration")?;
        let closed = &capture.conformance_application;
        let implementation = program
            .conformance_by_symbol(closed.declaration)
            .ok_or("captured member lost its selecting implementation")?;
        if capture.instance == capture.template
            || program.declaration_scope_parent(capture.template) != implementation.symbol
            || !implementation.rows.iter().any(|row| {
                row.source.owns_realization() && row.realization_machine == capture.template
            })
        {
            return Err("captured member does not belong to its selecting implementation".into());
        }
        if !capture.binding_application.is_valid() || !capture.supplying_application.is_valid() {
            return Err("captured member lost its binding or supplying application".into());
        }
        let binding = program.static_machine_application(capture.binding_application);
        let supplying = program.static_machine_application(capture.supplying_application);
        if binding.binding_declaration != implementation.symbol
            || binding.lexical_owner != supplying.lexical_owner
        {
            return Err(
                "captured member bindings changed implementation or supplying owner".into(),
            );
        }
        let context = TypeIdentityContext {
            application_owner: binding.lexical_owner,
            application_argument_owner: binding.lexical_owner,
            ..*context
        };
        let parameters = program.conformance_type_parameters(implementation);
        let arguments = program.static_machine_application_arguments(capture.binding_application);
        let bindings = program.static_machine_application_bindings(capture.binding_application);
        if parameters.len() != bindings.len() || parameters.len() != arguments.len() {
            return Err("captured member has an incomplete implementation argument list".into());
        }
        for (parameter, argument) in parameters.iter().zip(arguments) {
            let mut matching = bindings
                .iter()
                .filter(|binding| binding.parameter == parameter.symbol);
            let selected = matching
                .next()
                .ok_or("captured member is missing an implementation parameter binding")?;
            if matching.next().is_some() {
                return Err("captured member repeats an implementation parameter binding".into());
            }
            let (_, operand) = static_binding_operand(&parameter.kind, &selected.value)?;
            if super::super::substitution::index_argument(program, operand, &context)
                != super::super::substitution::index_argument(
                    program,
                    IndexIdentityArgument::StaticArgument(argument),
                    &context,
                )
            {
                return Err(format!(
                    "captured member binding for `{}` differs from its selected argument",
                    parameter.name
                ));
            }
        }
        let lifetimes = program
            .static_application_lifetimes_for_declaration(
                capture.binding_application,
                implementation.symbol,
            )
            .map_err(str::to_owned)?;
        if lifetimes.len() != implementation.lifetime_parameters.len() {
            return Err("captured member has an incomplete implementation lifetime list".into());
        }
        let mut lifetime_arguments = Vec::with_capacity(lifetimes.len());
        for parameter in &implementation.lifetime_parameters {
            let argument = lifetimes
                .iter()
                .find(|binding| binding.parameter == *parameter)
                .ok_or("captured member lost an implementation lifetime binding")?;
            lifetime_arguments.push(argument.argument.clone());
        }
        let selected = StaticMachineArgument {
            symbol: implementation.symbol,
            application: Some(Box::new(StaticSymbolApplication {
                member: Default::default(),
                arguments: arguments.into(),
                lifetime_arguments: lifetime_arguments.into(),
            })),
            ..Default::default()
        };
        let implementation_identity = conformances::project(program, closed, &selected, &context)?;

        // The capture preserves the member's own generic declaration. Verify
        // its explicit copied-binder map without serializing temporary symbols.
        let source_parameters = program
            .machine_type_parameters(original)
            .iter()
            .map(|parameter| parameter.symbol)
            .chain(original.conformance_bounds.iter().map(|bound| bound.binder))
            .collect::<Vec<_>>();
        let instance_parameters = program
            .machine_type_parameters(instance)
            .iter()
            .map(|parameter| parameter.symbol)
            .chain(instance.conformance_bounds.iter().map(|bound| bound.binder))
            .collect::<Vec<_>>();
        if source_parameters.len() != capture.parameters.len()
            || instance_parameters.len() != capture.parameters.len()
            || capture
                .parameters
                .iter()
                .zip(source_parameters.iter().zip(&instance_parameters))
                .any(
                    |((source, realized), (expected_source, expected_instance))| {
                        source != expected_source || realized != expected_instance
                    },
                )
        {
            return Err("captured member changed its own parameter correspondence".into());
        }
        let states = program
            .captured_machine_states
            .span_or_empty(capture.states);
        if states.len() != program.machine_states(original).len()
            || states.len() != program.machine_states(instance).len()
            || states.iter().enumerate().any(|(ordinal, state)| {
                states[..ordinal].iter().any(|earlier| {
                    earlier.template == state.template || earlier.instance == state.instance
                }) || !program.instantiated_machine_state_matches(
                    capture.instance,
                    capture.template,
                    state.template,
                    state.instance,
                )
            })
        {
            return Err("captured member changed its explicit entry correspondence".into());
        }
        let origin = self.origin(
            program,
            capture.instance,
            capture.template,
            &capture.normalized_template_identity,
            &context,
        )?;
        Ok(compound(
            CoordinateTag::CapturedCallable,
            [
                origin,
                atom(CoordinateTag::Declaration, &implementation_identity),
            ],
        ))
    }
}
