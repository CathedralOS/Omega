//! Actual static operands of one selected proof application.
//!
//! The retained application owns named calls; an operator selection owns its
//! inferred tuple. This intake never selects a callable again and never uses
//! the caller's same-spelled parameters as missing callee operands.

use super::{SelectedCallContract, StructuralTerm, call_machine_substitution, owner_symbol};
use language_semantics::const_value::{CanonicalConstIdentity, DecodedCanonicalConstValue};
use symbols::SymbolHandle;
use typed_trees::data::{TypeParameter, TypeParameterKind};
use typed_trees::expression::TableCallExpression;
use typed_trees::operator::OperatorConstBinding;
use typed_trees::static_applications::StaticMachineBindingValue;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

pub(super) type MachineBindings = Vec<(
    SymbolHandle,
    TypeReferenceHandle,
    typed_trees::expression::StaticMachineArgument,
)>;

#[derive(Default)]
pub(in crate::proof_contracts::contract_entailment) struct AppliedProofBindings {
    pub types: Vec<(SymbolHandle, TypeReferenceHandle)>,
    pub constants: Vec<AppliedConstBinding>,
    /// Authenticated executable Value binders. These retain header identity,
    /// not a constant value or a new observation of their supplying storage.
    pub runtime_parameters: Vec<AppliedRuntimeBinding>,
    pub machines: MachineBindings,
    pub implementations: Vec<(SymbolHandle, typed_trees::expression::StaticMachineArgument)>,
}

/// One admitted result recipe together with the telescope its value leaves
/// actually name. Captured ordinary bindings are transported, never re-read.
pub(in crate::proof_contracts::contract_entailment) struct AppliedResultPresentation<'scope> {
    pub type_reference: TypeReferenceHandle,
    pub parameter_owner: SymbolHandle,
    pub parameters: &'scope [typed_trees::signature::StateParameter],
    pub bindings: std::borrow::Cow<'scope, [(SymbolHandle, StructuralTerm)]>,
}

/// One authenticated static value actual. Open values keep their supplying
/// declaration and carrier; they are never encoded as closed constants.
pub(in crate::proof_contracts::contract_entailment) struct AppliedConstBinding {
    pub symbol: SymbolHandle,
    value: AppliedConstValue,
}

/// The exact runtime row survives binder transport without entering the
/// constant substitution lane. Realized calls observe its executable formal;
/// open calls can name an immutable lexical Value actual directly.
pub(in crate::proof_contracts::contract_entailment) struct AppliedRuntimeBinding {
    pub symbol: SymbolHandle,
    actual: StaticMachineBindingValue,
}

impl AppliedRuntimeBinding {
    pub fn observation_subject(&self) -> Option<super::RuntimeScalarSubject> {
        let StaticMachineBindingValue::RuntimeValue {
            subject,
            expression,
            ..
        } = self.actual
        else {
            return None;
        };
        if expression.is_valid() {
            Some(super::RuntimeScalarSubject::Expression(expression))
        } else if subject.is_valid() {
            Some(super::RuntimeScalarSubject::Symbol(subject))
        } else {
            None
        }
    }
}

enum AppliedConstValue {
    Closed(CanonicalConstIdentity),
    Parameter {
        symbol: SymbolHandle,
        type_reference: TypeReferenceHandle,
    },
}

impl AppliedConstBinding {
    pub fn value_term(&self) -> Option<StructuralTerm> {
        match &self.value {
            AppliedConstValue::Parameter {
                symbol,
                type_reference,
            } => (symbol.is_valid() && type_reference.is_valid())
                .then_some(StructuralTerm::BoundValue(*symbol)),
            AppliedConstValue::Closed(value) => {
                super::super::structural_terms::admitted_const_value_term(value)
            }
        }
    }
}

impl AppliedProofBindings {
    pub fn for_operator(
        program: &dyn crate::CheckingInput,
        application: &crate::ScopedOperatorApplication<'_>,
    ) -> Option<Self> {
        use crate::ScopedOperatorApplication;
        let (parameters, types, constants) = match application {
            ScopedOperatorApplication::Declared {
                declaration,
                bindings,
            } => (
                program
                    .operator_type_parameters(declaration.operator)
                    .iter()
                    .collect::<Vec<_>>(),
                bindings.types.as_slice(),
                bindings.constants.as_slice(),
            ),
            ScopedOperatorApplication::BoundTrait(meaning) => (
                program
                    .trait_type_parameters(meaning.trait_definition)
                    .iter()
                    .chain(program.state_signature_type_parameters(meaning.requirement))
                    .collect(),
                meaning.substitutions.as_slice(),
                meaning.constants.as_slice(),
            ),
            ScopedOperatorApplication::Trait(meaning) => (
                program
                    .trait_type_parameters(meaning.trait_definition)
                    .iter()
                    .chain(program.state_signature_type_parameters(meaning.requirement))
                    .collect(),
                meaning.substitutions.as_slice(),
                meaning.constants.as_slice(),
            ),
            ScopedOperatorApplication::SelectedTrait { meaning, bindings } => (
                program
                    .trait_type_parameters(meaning.trait_definition)
                    .iter()
                    .chain(program.state_signature_type_parameters(meaning.requirement))
                    .collect(),
                bindings.types.as_slice(),
                bindings.constants.as_slice(),
            ),
        };
        Self::from_selected(program, &parameters, types, constants)
    }

    fn from_selected(
        program: &dyn crate::CheckingInput,
        parameters: &[&TypeParameter],
        types: &[(SymbolHandle, TypeReferenceHandle)],
        constants: &[OperatorConstBinding],
    ) -> Option<Self> {
        let mut result = Self::default();
        for parameter in parameters {
            match parameter.kind {
                TypeParameterKind::Type => {
                    let mut selected = types
                        .iter()
                        .filter(|(symbol, _)| *symbol == parameter.symbol);
                    let (_, actual) = selected.next()?;
                    if !actual.is_valid() || selected.next().is_some() {
                        return None;
                    }
                    result.types.push((parameter.symbol, *actual));
                }
                TypeParameterKind::Const { type_reference } => {
                    let mut selected = constants
                        .iter()
                        .filter(|binding| binding.symbol == parameter.symbol);
                    let actual = selected.next()?;
                    if selected.next().is_some() {
                        return None;
                    }
                    crate::value_custody::type_references::validate_exact_const_identity(
                        program,
                        typed_trees::static_applications::project_type_operand(
                            program,
                            type_reference,
                            types,
                        ),
                        &actual.value,
                    )
                    .ok()?;
                    result.constants.push(AppliedConstBinding {
                        symbol: actual.symbol,
                        value: AppliedConstValue::Closed(actual.value.clone()),
                    });
                }
                // These categories are not integer constants or type operands.
                TypeParameterKind::Value { .. }
                | TypeParameterKind::Machine { .. }
                | TypeParameterKind::Implementation
                | TypeParameterKind::Proposition { .. } => return None,
            }
        }
        Some(result)
    }

    pub fn for_call(
        program: &dyn crate::CheckingInput,
        caller: SymbolHandle,
        owner: &SelectedCallContract<'_>,
        call: &TableCallExpression,
    ) -> Option<Self> {
        Self::for_call_site(
            program,
            caller,
            owner,
            crate::CallSite::Expression {
                expression: Default::default(),
                call,
            },
        )
    }

    pub fn for_call_site(
        program: &dyn crate::CheckingInput,
        caller: SymbolHandle,
        owner: &SelectedCallContract<'_>,
        site: crate::CallSite<'_>,
    ) -> Option<Self> {
        let (handle, arguments) = match site {
            crate::CallSite::Expression { call, .. } => (
                call.authored_machine_application,
                call.machine_arguments.as_ref(),
            ),
            crate::CallSite::Statement(call) => (
                call.authored_machine_application,
                call.machine_arguments.as_ref(),
            ),
            crate::CallSite::TransitionNamed { .. } => return None,
        };
        if handle.is_valid() {
            let application = program.expression_table.static_machine_application(handle);
            if application.lexical_owner != caller {
                return None;
            }
            if !matches!(
                owner,
                SelectedCallContract::Operator(_) | SelectedCallContract::DynamicSlot { .. }
            ) && let Some(declaration) =
                program.machine_by_symbol(application.binding_declaration)
            {
                let correspondence = crate::machine_calls::calls::call_binding_correspondence(
                    program, caller, &site,
                )
                .ok()?;
                if correspondence.public_owner != owner_symbol(owner)
                    || correspondence.declaration != declaration.symbol
                {
                    return None;
                }
                // Decode the retained source tuple under its own declaration.
                // Only binder identities are then transported to the applied
                // contract; a private body's guarantees never enter this intake.
                let entry = program.machine_states(declaration).first()?;
                let intake = SelectedCallContract::Concrete(declaration, entry);
                let mut bindings = Self::for_application(
                    program,
                    &intake,
                    handle,
                    arguments,
                    &correspondence.runtime_parameters,
                    &[],
                    declaration.symbol,
                )?;
                bindings.transport_parameters(&correspondence.parameters)?;
                return Some(bindings);
            }
            if let SelectedCallContract::Signature(signature) = owner
                && (program
                    .trait_machine_signature(application.binding_declaration)
                    .is_some()
                    || program
                        .machine_by_symbol(caller)
                        .and_then(|machine| {
                            program.machine_parameter_signature_in(
                                machine,
                                application.binding_declaration,
                            )
                        })
                        .is_some_and(|declared| std::ptr::eq(declared, *signature)))
            {
                let correspondence = crate::machine_calls::calls::call_binding_correspondence(
                    program, caller, &site,
                )
                .ok()?;
                if correspondence.public_owner != owner_symbol(owner)
                    || correspondence.declaration != application.binding_declaration
                {
                    return None;
                }
                let runtime_parameters = correspondence
                    .runtime_parameters
                    .iter()
                    .map(|source| {
                        let mut rows = correspondence
                            .parameters
                            .iter()
                            .filter(|(parameter, _)| parameter == source);
                        let (_, logical) = rows.next()?;
                        rows.next().is_none().then_some(*logical)
                    })
                    .collect::<Option<Vec<_>>>()?;
                // Applied requirements and lexical callable parameters both
                // retain their exact receiving telescope. Decode the authored
                // tuple only through its authenticated source-to-public map.
                return Self::for_application(
                    program,
                    owner,
                    handle,
                    arguments,
                    &runtime_parameters,
                    &correspondence.parameters,
                    correspondence.declaration,
                );
            }
        }
        let declaration = match owner {
            SelectedCallContract::Concrete(machine, _) => machine.symbol,
            _ => owner_symbol(owner),
        };
        Self::for_application(program, owner, handle, arguments, &[], &[], declaration)
    }

    fn transport_parameters(&mut self, parameters: &[(SymbolHandle, SymbolHandle)]) -> Option<()> {
        let target = |source| {
            let mut matches = parameters
                .iter()
                .filter(|(parameter, _)| *parameter == source);
            let (_, logical) = matches.next()?;
            matches.next().is_none().then_some(*logical)
        };
        // The complete source tuple was authenticated before this projection.
        // A statically erased binder has no receiving name and creates no fact
        // in the realized contract. Its already substituted payload stays there.
        for (parameter, _) in &mut self.types {
            *parameter = target(*parameter)?;
        }
        self.types.retain(|(parameter, _)| parameter.is_valid());
        for binding in &mut self.constants {
            binding.symbol = target(binding.symbol)?;
        }
        self.constants.retain(|binding| binding.symbol.is_valid());
        for parameter in &mut self.runtime_parameters {
            parameter.symbol = target(parameter.symbol)?;
            if !parameter.symbol.is_valid() {
                return None;
            }
        }
        for (parameter, _, _) in &mut self.machines {
            let logical = target(*parameter)?;
            // Machine operands still govern guarantee independence even when
            // realization erased the header. Do not drop their source custody.
            if logical.is_valid() {
                *parameter = logical;
            }
        }
        for (parameter, _) in &mut self.implementations {
            // Concrete conformance bounds keep their independent source owner;
            // they are not value bindings in the public member telescope.
            if let Some(logical) = target(*parameter).filter(|symbol| symbol.is_valid()) {
                *parameter = logical;
            }
        }
        Some(())
    }

    fn for_application(
        program: &dyn crate::CheckingInput,
        owner: &SelectedCallContract<'_>,
        handle: typed_trees::static_applications::StaticMachineApplicationHandle,
        machine_arguments: &[typed_trees::expression::StaticMachineArgument],
        runtime_parameters: &[SymbolHandle],
        binding_parameters: &[(SymbolHandle, SymbolHandle)],
        declaration: SymbolHandle,
    ) -> Option<Self> {
        let parameters = match owner {
            SelectedCallContract::Operator(operator) => program.operator_type_parameters(operator),
            SelectedCallContract::Concrete(machine, _) => program.machine_type_parameters(machine),
            SelectedCallContract::Signature(signature)
            | SelectedCallContract::DynamicSlot { signature, .. } => {
                program.state_signature_type_parameters(signature)
            }
        };
        let conformance_bounds = match owner {
            SelectedCallContract::Concrete(machine, _) => machine.conformance_bounds.as_slice(),
            _ => &[],
        };
        // Registration can precede closed-binding publication. For a pure
        // Machine telescope its live operand lane is checked independently;
        // an empty registration shell does not erase those exact selections.
        // Once any binding roster/declaration is published it is authoritative.
        let registration_only = !handle.is_valid() || {
            let application = program.expression_table.static_machine_application(handle);
            !application.binding_declaration.is_valid()
                && program
                    .expression_table
                    .static_machine_application_bindings(handle)
                    .is_empty()
        };
        if registration_only
            && conformance_bounds.is_empty()
            && parameters
                .iter()
                .all(|parameter| matches!(parameter.kind, TypeParameterKind::Machine { .. }))
        {
            return Some(Self {
                machines: call_machine_substitution(program, owner, machine_arguments)?,
                ..Self::default()
            });
        }
        if !handle.is_valid() {
            return None;
        }
        let application = program.expression_table.static_machine_application(handle);
        if application.binding_declaration != declaration {
            return None;
        }
        let authored = program
            .expression_table
            .static_machine_application_bindings(handle);
        let retained: std::borrow::Cow<
            '_,
            [typed_trees::static_applications::StaticMachineBinding],
        > = if binding_parameters.is_empty() {
            std::borrow::Cow::Borrowed(authored)
        } else {
            std::borrow::Cow::Owned(
                authored
                    .iter()
                    .map(|binding| {
                        let mut mappings = binding_parameters
                            .iter()
                            .filter(|(source, _)| *source == binding.parameter);
                        let (_, parameter) = mappings.next()?;
                        if mappings.next().is_some() || !parameter.is_valid() {
                            return None;
                        }
                        let mut binding = binding.clone();
                        binding.parameter = *parameter;
                        Some(binding)
                    })
                    .collect::<Option<Vec<_>>>()?,
            )
        };
        // Logical applications retain open type and const identities under the
        // same exact telescope as named calls. Backend realization separately
        // requires closure; it cannot decide whether a public contract exists.
        // Preserve the operator categories this intake already supports.
        if let SelectedCallContract::Operator(operator) = owner
            && (!operator.lifetime_parameters.is_empty()
                || parameters.iter().any(|parameter| {
                    !matches!(
                        parameter.kind,
                        TypeParameterKind::Type | TypeParameterKind::Const { .. }
                    )
                }))
        {
            return None;
        }
        // Machine implementation binders have their own declared bound roster;
        // they are not ordinary Type/Const/Machine parameters. Both rosters
        // belong to this exact application and must be covered exactly once.
        if retained.len() != parameters.len() + conformance_bounds.len()
            || retained.iter().any(|binding| {
                parameters
                    .iter()
                    .filter(|parameter| parameter.symbol == binding.parameter)
                    .count()
                    + conformance_bounds
                        .iter()
                        .filter(|bound| bound.binder == binding.parameter)
                        .count()
                    != 1
            })
        {
            return None;
        }
        let types = parameters
            .iter()
            .filter(|parameter| matches!(parameter.kind, TypeParameterKind::Type))
            .map(|parameter| {
                let mut selected = retained
                    .iter()
                    .filter(|binding| binding.parameter == parameter.symbol);
                let binding = selected.next()?;
                if selected.next().is_some() {
                    return None;
                }
                let typed_trees::static_applications::StaticMachineBindingValue::Type(reference) =
                    binding.value
                else {
                    return None;
                };
                reference
                    .is_valid()
                    .then_some((parameter.symbol, reference))
            })
            .collect::<Option<Vec<_>>>()?;
        let mut result = Self {
            types,
            ..Self::default()
        };
        // As in ordinary static-argument admission, dependent receiving
        // carriers see the complete simultaneous tuple. Actual leaves retain
        // their supplying declaration rather than seeing receiving bindings.
        let applied_arguments = parameters
            .iter()
            .map(|parameter| {
                let mut selected = retained
                    .iter()
                    .filter(|binding| binding.parameter == parameter.symbol);
                let binding = selected.next()?;
                if selected.next().is_some() {
                    return None;
                }
                Some(match (&parameter.kind, &binding.value) {
                    (TypeParameterKind::Type, StaticMachineBindingValue::Type(reference))
                    | (
                        TypeParameterKind::Const { .. } | TypeParameterKind::Value { .. },
                        StaticMachineBindingValue::Const(reference),
                    ) => *reference,
                    _ => TypeReferenceHandle::invalid(),
                })
            })
            .collect::<Option<Vec<_>>>()?;
        for parameter in parameters {
            let mut selected = retained
                .iter()
                .filter(|binding| binding.parameter == parameter.symbol);
            let actual = selected.next()?;
            if selected.next().is_some() {
                return None;
            }
            match (&parameter.kind, &actual.value) {
                (TypeParameterKind::Type, StaticMachineBindingValue::Type(reference))
                    if reference.is_valid() =>
                {
                    // Complete type operands were retained simultaneously above.
                }
                (
                    TypeParameterKind::Const { type_reference }
                    | TypeParameterKind::Value { type_reference },
                    StaticMachineBindingValue::Const(reference),
                ) => {
                    if !reference.is_valid() {
                        return None;
                    }
                    // Canonical named constants can retain a resolved symbol;
                    // decode their admitted closed value before looking for an
                    // open supplying binder. Open values never acquire that
                    // closed identity merely because their row is Const.
                    let value = if let Some(value) =
                        typed_trees::operator::closed_const_identity_for_type_reference(
                            program,
                            *reference,
                            result.project_type(program, *type_reference),
                        ) {
                        crate::value_custody::type_references::validate_exact_const_identity_in_carrier(
                            program,
                            crate::value_custody::type_references::AppliedValueCarrier::applied(
                                *type_reference,
                                parameters,
                                &applied_arguments,
                            ),
                            &value,
                        )
                        .ok()?;
                        AppliedConstValue::Closed(value)
                    } else {
                        let symbol = match program.type_reference_table.type_reference(*reference) {
                            TypeReferenceNode::Named { symbol, .. } => *symbol,
                            TypeReferenceNode::ConstExpression(expression) => {
                                let typed_trees::expression::ExpressionNode::Name(path) =
                                    program.expression_table.expression(*expression)
                                else {
                                    return None;
                                };
                                // Only a direct resolved binder leaf is
                                // admitted here, never an unevaluated expression
                                // or the head of a projected/qualified path.
                                if program
                                    .expression_table
                                    .name_path_members(path.members)
                                    .len()
                                    != 1
                                {
                                    return None;
                                }
                                path.resolved_symbol()?
                            }
                            _ => return None,
                        };
                        if !symbol.is_valid() {
                            return None;
                        }
                        let caller = program.machine_by_symbol(application.lexical_owner)?;
                        let mut supplying = program
                            .machine_type_parameters(caller)
                            .iter()
                            .filter(|binder| binder.symbol == symbol);
                        let binder = supplying.next()?;
                        if supplying.next().is_some() {
                            return None;
                        }
                        let (TypeParameterKind::Const {
                            type_reference: actual,
                        }
                        | TypeParameterKind::Value {
                            type_reference: actual,
                        }) = binder.kind
                        else {
                            return None;
                        };
                        if !crate::value_custody::type_references::AppliedValueCarrier::applied(
                            *type_reference,
                            parameters,
                            &applied_arguments,
                        )
                        .matches(program, actual)
                        {
                            return None;
                        }
                        AppliedConstValue::Parameter {
                            symbol,
                            type_reference: actual,
                        }
                    };
                    result.constants.push(AppliedConstBinding {
                        symbol: parameter.symbol,
                        value,
                    });
                }
                (
                    TypeParameterKind::Value { type_reference },
                    StaticMachineBindingValue::RuntimeValue {
                        type_reference: actual_type,
                        ..
                    },
                ) => {
                    if runtime_parameters
                        .iter()
                        .filter(|symbol| **symbol == parameter.symbol)
                        .count()
                        != 1
                        || !crate::value_custody::type_references::AppliedValueCarrier::applied(
                            *type_reference,
                            parameters,
                            &applied_arguments,
                        )
                        .matches(program, *actual_type)
                    {
                        return None;
                    }
                    // The common call correspondence authenticated the actual
                    // expression, carrier, ordinal and clone/public binder.
                    // Its value belongs to the normal observer's captured
                    // actual roster and is deliberately absent from constants.
                    result.runtime_parameters.push(AppliedRuntimeBinding {
                        symbol: parameter.symbol,
                        actual: actual.value.clone(),
                    });
                }
                (TypeParameterKind::Machine { .. }, StaticMachineBindingValue::Machine(actual)) => {
                    result.machines.push(super::selected_machine_binding(
                        program,
                        parameter.symbol,
                        actual,
                    )?);
                }
                (
                    TypeParameterKind::Implementation,
                    StaticMachineBindingValue::Conformance(actual),
                ) => {
                    result.implementations.push((
                        parameter.symbol,
                        implementation_operand(program, application.lexical_owner, actual)?,
                    ));
                }
                _ => return None,
            }
        }
        if result.runtime_parameters.len() != runtime_parameters.len() {
            return None;
        }
        for bound in conformance_bounds {
            let mut selected = retained
                .iter()
                .filter(|binding| binding.parameter == bound.binder);
            let actual = selected.next()?;
            if selected.next().is_some() {
                return None;
            }
            let StaticMachineBindingValue::Conformance(actual) = &actual.value else {
                return None;
            };
            result.implementations.push((
                bound.binder,
                implementation_operand(program, application.lexical_owner, actual)?,
            ));
        }
        Some(result)
    }

    /// Admit the result and preserve the exact telescope of its dependent
    /// recipe. A dynamic slot freshens ordinary formals; its public requirement
    /// keeps the source symbols. Alpha correspondence changes neither actual
    /// values nor the full qualified/indexed result contract.
    pub fn runtime_result_presentation<'scope>(
        &self,
        program: &dyn crate::CheckingInput,
        owner: &SelectedCallContract<'scope>,
        call: &TableCallExpression,
        observed: TypeReferenceHandle,
        applied_result_parameters: Option<&[typed_trees::signature::StateParameter]>,
        parameter_owner: SymbolHandle,
        parameters: &'scope [typed_trees::signature::StateParameter],
        bindings: &'scope [(SymbolHandle, StructuralTerm)],
    ) -> Option<AppliedResultPresentation<'scope>> {
        use typed_trees::type_identity::{
            AppliedTypeIdentityScope, IndexIdentityArgument, try_normalized_index_identity,
        };
        let SelectedCallContract::DynamicSlot { slot, signature } = owner else {
            return Some(AppliedResultPresentation {
                type_reference: self.runtime_result_type(
                    program,
                    owner,
                    call,
                    observed,
                    applied_result_parameters,
                )?,
                parameter_owner,
                parameters,
                bindings: std::borrow::Cow::Borrowed(bindings),
            });
        };
        let slot = *slot;
        let signature = *signature;
        let declared = program.state_signature_parameters(signature);
        let selected = slot.data_parameters();
        if !slot.symbol.is_valid()
            || !signature.symbol.is_valid()
            || parameter_owner != signature.symbol
            || call.target_symbol != slot.symbol
            || slot.requirement != signature.symbol
            || program.dynamic_slot_requirement(slot)?.symbol != signature.symbol
            || program.symbols.get(slot.symbol).generated_from != signature.symbol
            || slot.parameters.len() != declared.len() + 1
            || selected.len() != parameters.len()
            || parameters.len() != declared.len()
        {
            return None;
        }
        let selector = slot.parameters.first()?;
        if !selector.symbol.is_valid()
            || program.symbols.get(selector.symbol).parent != slot.symbol
            || program.symbols.get(selector.symbol).generated_from != signature.symbol
        {
            return None;
        }
        let mut public_binders = Vec::with_capacity(declared.len());
        let mut slot_binders = Vec::with_capacity(selected.len());
        for (position, ((source, selected), prepared)) in
            declared.iter().zip(selected).zip(parameters).enumerate()
        {
            if !source.symbol.is_valid()
                || !selected.symbol.is_valid()
                || prepared.symbol != source.symbol
                || declared
                    .iter()
                    .any(|parameter| parameter.symbol == selected.symbol)
                || program.symbols.get(selected.symbol).parent != slot.symbol
                || program.symbols.get(selected.symbol).generated_from != source.symbol
                || selected.is_const != source.is_const
                || selected.is_mutable != source.is_mutable
                || prepared.is_const != source.is_const
                || prepared.is_mutable != source.is_mutable
                || public_binders
                    .iter()
                    .any(|(symbol, _)| *symbol == source.symbol)
                || slot_binders
                    .iter()
                    .any(|(symbol, _)| *symbol == selected.symbol)
            {
                return None;
            }
            // Ordinal alpha tokens are private to this authenticated complete
            // signature map. Exact handles remain the keys on both sides;
            // source names, Debug payloads and actual values are not identities.
            let identity = format!("\0public-result-formal:{position}");
            public_binders.push((source.symbol, identity.clone()));
            slot_binders.push((selected.symbol, identity));
        }
        let requires_header = !program
            .state_signature_type_parameters(signature)
            .is_empty()
            || !self.types.is_empty()
            || !self.constants.is_empty()
            || !self.runtime_parameters.is_empty()
            || !self.machines.is_empty()
            || !self.implementations.is_empty();
        let mut header = Vec::new();
        let mut header_owner = SymbolHandle::invalid();
        if call.authored_machine_application.is_valid() {
            let application = program
                .expression_table
                .static_machine_application(call.authored_machine_application);
            let retained = program
                .expression_table
                .static_machine_application_bindings(call.authored_machine_application);
            if retained.is_empty() {
                if requires_header {
                    return None;
                }
            } else {
                if !application.binding_declaration.is_valid() {
                    return None;
                }
                header_owner = application.binding_declaration;
                header.extend(retained.iter().map(|binding| {
                    let argument = match &binding.value {
                        StaticMachineBindingValue::Type(reference) => {
                            IndexIdentityArgument::TypeArgument(*reference)
                        }
                        StaticMachineBindingValue::Const(reference) => {
                            IndexIdentityArgument::TypeReference(*reference)
                        }
                        StaticMachineBindingValue::RuntimeValue {
                            subject,
                            expression,
                            ..
                        } => {
                            if expression.is_valid() {
                                IndexIdentityArgument::Expression(*expression)
                            } else {
                                IndexIdentityArgument::Subject(*subject)
                            }
                        }
                        StaticMachineBindingValue::Machine(argument)
                        | StaticMachineBindingValue::Conformance(argument) => {
                            IndexIdentityArgument::StaticArgument(argument)
                        }
                    };
                    (binding.parameter, argument)
                }));
            }
        } else if requires_header {
            return None;
        }
        // Nongeneric calls may be admitted before an application is retained.
        // Their genuinely empty scope indexes neither a zero handle nor an
        // invented declaration. Required header evidence never takes this path.
        let prepared_scopes = [AppliedTypeIdentityScope {
            declaration: header_owner,
            arguments: &header,
            lifetimes: &[],
        }];
        let scopes = if header_owner.is_valid() {
            &prepared_scopes[..]
        } else {
            &[]
        };
        let identity = |reference, binders: &[(SymbolHandle, String)]| {
            try_normalized_index_identity(
                program.typed_program(),
                IndexIdentityArgument::TypeArgument(reference),
                binders,
                scopes,
            )
        };
        // The prepared public telescope already takes its instantiated types
        // from this slot while retaining public symbols. Compare every full
        // type after the complete simultaneous ordinary alpha map exists.
        for (selected, prepared) in selected.iter().zip(parameters) {
            if identity(selected.type_reference, &slot_binders)?
                != identity(prepared.type_reference, &slot_binders)?
            {
                return None;
            }
        }
        if program.arithmetic_domain_for_type_reference(observed)
            != program
                .arithmetic_domain_for_type_reference(self.project_type(program, slot.return_type))
        {
            return None;
        }
        let result = identity(slot.return_type, &slot_binders)?;
        if identity(observed, &public_binders).as_ref() == Some(&result) {
            return Some(AppliedResultPresentation {
                type_reference: observed,
                parameter_owner,
                parameters,
                bindings: std::borrow::Cow::Borrowed(bindings),
            });
        }
        if identity(observed, &slot_binders)? != result {
            return None;
        }
        let mut selected_bindings = Vec::with_capacity(bindings.len());
        for (symbol, value) in bindings {
            let symbol = parameters
                .iter()
                .position(|parameter| parameter.symbol == *symbol)
                .map_or(*symbol, |position| selected[position].symbol);
            if selected_bindings
                .iter()
                .any(|(previous, _)| *previous == symbol)
            {
                return None;
            }
            selected_bindings.push((symbol, value.clone()));
        }
        Some(AppliedResultPresentation {
            type_reference: observed,
            parameter_owner: slot.symbol,
            parameters: selected,
            bindings: std::borrow::Cow::Owned(selected_bindings),
        })
    }

    /// Authenticate a completed result under the same simultaneous static and
    /// ordinary application that produced its occurrence type. The declaration
    /// recipe remains in its formal telescope: downstream proof substitutes
    /// the already completed argument values, never current caller storage.
    pub fn runtime_result_type(
        &self,
        program: &dyn crate::CheckingInput,
        owner: &SelectedCallContract<'_>,
        call: &TableCallExpression,
        observed: TypeReferenceHandle,
        applied_result_parameters: Option<&[typed_trees::signature::StateParameter]>,
    ) -> Option<TypeReferenceHandle> {
        use typed_trees::type_identity::{
            AppliedTypeIdentityScope, IndexIdentityArgument, applied_declared_carrier_identity,
            try_normalized_index_identity,
        };
        let declared = super::owner_return_type(owner);
        let static_bindings = if call.authored_machine_application.is_valid() {
            program
                .expression_table
                .static_machine_application_bindings(call.authored_machine_application)
        } else if self.types.is_empty()
            && self.constants.is_empty()
            && self.runtime_parameters.is_empty()
            && self.machines.is_empty()
            && self.implementations.is_empty()
        {
            // Ordinary monomorphic calls need no retained static header. A
            // missing required application never becomes an empty substitution.
            &[]
        } else {
            return None;
        };
        let mut application = static_bindings
            .iter()
            .filter_map(|binding| match binding.value {
                StaticMachineBindingValue::Type(reference) => Some((
                    binding.parameter,
                    IndexIdentityArgument::TypeArgument(reference),
                )),
                StaticMachineBindingValue::Const(reference) => Some((
                    binding.parameter,
                    IndexIdentityArgument::TypeReference(reference),
                )),
                _ => None,
            })
            .collect::<Vec<_>>();
        let static_count = application.len();
        let arguments = program.expression_table.expression_handles(call.arguments);
        // The producer applies ordinary operands only under this exact
        // CallSite public/applied telescope and complete explicit arity.
        // Missing or nonmatching rosters retain declaration vocabulary there,
        // so they cannot become an invented positional map here.
        let parameters =
            applied_result_parameters.filter(|parameters| parameters.len() == arguments.len());
        for (parameter, argument) in parameters.into_iter().flatten().zip(arguments) {
            if !parameter.symbol.is_valid()
                || application
                    .iter()
                    .any(|(symbol, _)| *symbol == parameter.symbol)
            {
                return None;
            }
            application.push((
                parameter.symbol,
                IndexIdentityArgument::Expression(*argument),
            ));
        }
        // Actual expressions identify the supplying vocabulary only. The
        // identity owner substitutes them simultaneously without executing or
        // resolving their current values. Recursive calls cannot reapply the
        // receiving map to leaves of an already selected actual.
        let scope = AppliedTypeIdentityScope {
            declaration: super::owner_symbol(owner),
            arguments: &application,
            lifetimes: &[],
        };
        if applied_declared_carrier_identity(program, observed, &[])?
            != applied_declared_carrier_identity(program, declared, &[scope])?
            || program.arithmetic_domain_for_type_reference(observed)
                != program
                    .arithmetic_domain_for_type_reference(self.project_type(program, declared))
        {
            return None;
        }
        // The published type is the declaration's complete qualified recipe
        // after static substitution, before replacing ordinary formals. Its
        // index equations are interpreted under captured normal-call bindings.
        // A receiving annotation may match its carrier but donate no domain.
        let static_scope = AppliedTypeIdentityScope {
            declaration: super::owner_symbol(owner),
            arguments: &application[..static_count],
            lifetimes: &[],
        };
        let identity = try_normalized_index_identity(
            program,
            IndexIdentityArgument::TypeArgument(declared),
            &[],
            &[static_scope],
        )?;
        // Full qualified identities must also be complete. An unavailable
        // index marker is diagnostic payload, never a matching public recipe.
        let public = self.project_type(program, declared);
        if try_normalized_index_identity(
            program,
            IndexIdentityArgument::TypeArgument(public),
            &[],
            &[],
        )
        .as_ref()
            == Some(&identity)
        {
            return Some(public);
        }
        program
            .type_reference_table
            .references()
            .find_map(|(reference, _)| {
                (try_normalized_index_identity(
                    program,
                    IndexIdentityArgument::TypeArgument(reference),
                    &[],
                    &[],
                )
                .as_ref()
                    == Some(&identity))
                .then_some(reference)
            })
    }

    /// A direct formal type is read under this simultaneous application.
    /// The actual reference already retains its supplying owner and policy;
    /// it is not recursively substituted under the receiving roster.
    pub fn project_type(
        &self,
        program: &dyn crate::CheckingInput,
        reference: TypeReferenceHandle,
    ) -> TypeReferenceHandle {
        typed_trees::static_applications::project_type_operand(program, reference, &self.types)
    }

    pub fn retains_parameter(&self, parameter: &TypeParameter) -> bool {
        match parameter.kind {
            TypeParameterKind::Type => self
                .types
                .iter()
                .any(|(symbol, _)| *symbol == parameter.symbol),
            TypeParameterKind::Const { .. } => self
                .constants
                .iter()
                .any(|binding| binding.symbol == parameter.symbol),
            TypeParameterKind::Value { .. } => {
                self.constants
                    .iter()
                    .any(|binding| binding.symbol == parameter.symbol)
                    || self
                        .runtime_parameters
                        .iter()
                        .any(|binding| binding.symbol == parameter.symbol)
            }
            TypeParameterKind::Machine { .. } => self
                .machines
                .iter()
                .any(|(symbol, _, _)| *symbol == parameter.symbol),
            TypeParameterKind::Implementation => self
                .implementations
                .iter()
                .any(|(symbol, _)| *symbol == parameter.symbol),
            TypeParameterKind::Proposition { .. } => false,
        }
    }

    pub fn constant_integer(&self, symbol: SymbolHandle) -> Option<numerics::bignum::BigInt> {
        let AppliedConstValue::Closed(value) = &self
            .constants
            .iter()
            .find(|binding| binding.symbol == symbol)?
            .value
        else {
            return None;
        };
        integer(value)
    }
}

/// Read an erased selection without deriving a fact from its implementation.
/// Ordinary static admission still owns its complete trait tuple and bounds.
fn implementation_operand(
    program: &dyn crate::CheckingInput,
    lexical_owner: SymbolHandle,
    actual: &typed_trees::expression::StaticMachineArgument,
) -> Option<typed_trees::expression::StaticMachineArgument> {
    if !actual.symbol.is_valid()
        || actual.type_reference.is_valid()
        || actual.const_literal.is_some()
        || actual.evidence_projection.is_some()
    {
        return None;
    }
    let named = program.conformance_by_symbol(actual.symbol).is_some();
    let forwarded = program
        .machine_by_symbol(lexical_owner)
        .is_some_and(|caller| {
            caller
                .conformance_bounds
                .iter()
                .any(|bound| bound.binder == actual.symbol)
        });
    (named || forwarded).then(|| actual.clone())
}

pub(super) fn integer(value: &CanonicalConstIdentity) -> Option<numerics::bignum::BigInt> {
    match value.decode_encoding()? {
        DecodedCanonicalConstValue::Integer { value, .. } => {
            Some(numerics::bignum::BigInt::from_i128(value))
        }
        _ => None,
    }
}
