//! Trace a copied machine, its entry and its static arguments to their source declarations.

use crate::source_state::{
    CapturedMachineApplication, MachineSpecialization, TypedTrees,
    static_applications::{StaticMachineBinding, StaticMachineLifetimeBinding},
};
use symbols::SymbolHandle;

mod map_captures;
pub use map_captures::{NamedCaptureError, NamedConformanceCaptureIndex};

const MISSING_EXECUTABLE_TEMPLATE: &str = "executable application lost its template machine";

#[derive(Clone, Copy)]
pub enum MachineInstantiation<'program> {
    Specialization(&'program MachineSpecialization),
    Capture(
        &'program CapturedMachineApplication,
        &'program crate::source_state::CapturedMachineDeclaration,
    ),
}

pub struct EnclosingInvocationFrame<'program> {
    pub declaration: SymbolHandle,
    pub argument_owner: SymbolHandle,
    pub bindings: &'program [StaticMachineBinding],
    pub lifetimes: Vec<StaticMachineLifetimeBinding>,
}
impl MachineInstantiation<'_> {
    pub fn template(self) -> SymbolHandle {
        match self {
            Self::Specialization(receipt) => receipt.template,
            Self::Capture(_, declaration) => declaration.template,
        }
    }
    pub fn instance(self) -> SymbolHandle {
        match self {
            Self::Specialization(receipt) => receipt.instance,
            Self::Capture(_, declaration) => declaration.instance,
        }
    }
    pub fn binding_application(
        self,
    ) -> crate::source_state::static_applications::StaticMachineApplicationHandle {
        match self {
            Self::Specialization(receipt) => receipt.binding_application,
            Self::Capture(receipt, _) => receipt.binding_application(),
        }
    }

    /// Rejoins an executable entry to the exact declaration retained when
    /// its application was produced. State order and symbol provenance do
    /// not supply missing correspondence.
    pub fn entry_template(
        self,
        program: &TypedTrees,
        entry: SymbolHandle,
    ) -> Result<SymbolHandle, &'static str> {
        let span = match self {
            Self::Specialization(receipt) => receipt.states,
            Self::Capture(_, declaration) => declaration.states,
        };
        let template = self.template();
        program.validate_executable_template_symbol(template)?;
        let source = program
            .machine(template)
            .ok_or(MISSING_EXECUTABLE_TEMPLATE)?;
        let instance = program
            .machine(self.instance())
            .ok_or("executable application lost its instance machine")?;
        let source_states = program.machine_states(source);
        let instance_states = program.machine_states(instance);
        let states = program
            .captured_machine_states
            .span(span)
            .ok_or("executable application lost its entry correspondence")?;
        if states.len() != source_states.len() || states.len() != instance_states.len() {
            return Err("executable application has incomplete entry correspondence");
        }
        let mut selected = states.iter().filter(|state| state.instance == entry);
        let (Some(mapping), None) = (selected.next(), selected.next()) else {
            return Err("executable application has no unique entry correspondence");
        };
        if !instance_states.iter().any(|state| state.symbol == entry)
            || !source_states
                .iter()
                .any(|state| state.symbol == mapping.template)
        {
            return Err("executable application entry belongs to another template");
        }
        Ok(mapping.template)
    }
}
impl TypedTrees {
    /// Ownership discovery can use a template before reading its entry.
    /// Both reads require the exact retained symbol generation.
    pub fn validate_executable_template_symbol(
        &self,
        template: SymbolHandle,
    ) -> Result<(), &'static str> {
        self.symbols
            .symbols()
            .nodes()
            .is_valid(template)
            .then_some(())
            .ok_or(MISSING_EXECUTABLE_TEMPLATE)
    }

    /// A copied child keeps its own callable template, but the enclosing
    /// application binds the declaration that introduced the retained prefix.
    pub fn specialization_binding_declaration(
        &self,
        receipt: &MachineSpecialization,
    ) -> Result<SymbolHandle, &'static str> {
        use crate::source_state::data::TypeParameterKind;
        use crate::source_state::static_applications::StaticMachineBindingValue;
        let live = |symbol| self.symbols.symbols().nodes().is_valid(symbol);
        if !live(receipt.template)
            || !live(receipt.instance)
            || !self
                .expression_table
                .static_machine_application_is_valid(receipt.binding_application)
        {
            return Err(
                "specialization lost its current template, instance or binding application",
            );
        }
        let source = self.static_machine_application(receipt.binding_application);
        let declaration = source.binding_declaration;
        if !live(declaration) || !live(source.lexical_owner) {
            return Err("specialization binding declaration or supplier is missing or stale");
        }
        let template = self
            .machine_by_symbol(receipt.template)
            .ok_or("specialization template is not an actual machine declaration")?;
        let bound = self
            .machine_by_symbol(declaration)
            .ok_or("specialization binding declaration is not an actual machine")?;
        let instance = self
            .machine_by_symbol(receipt.instance)
            .ok_or("specialization instance is not an actual machine")?;
        let mut ancestor = receipt.template;
        let mut visited = Vec::new();
        while ancestor != declaration {
            if !live(ancestor) || visited.contains(&ancestor) {
                return Err(
                    "specialization binding declaration is outside its actual lexical ancestry",
                );
            }
            visited.push(ancestor);
            ancestor = self.symbols.get(ancestor).parent;
        }
        let prefix = self
            .data_type_parameters
            .span(receipt.template_parameters)
            .ok_or("specialization has a stale retained parameter prefix")?;
        let source_parameters = self
            .data_type_parameters
            .span(template.type_parameters)
            .ok_or("specialization template has a stale parameter list")?;
        let remaining = self
            .data_type_parameters
            .span(instance.type_parameters)
            .ok_or("specialization instance has a stale remaining parameter list")?;
        if source_parameters.len() != prefix.len() + remaining.len()
            || !source_parameters
                .iter()
                .take(prefix.len())
                .eq(prefix.iter())
            || !source_parameters
                .iter()
                .skip(prefix.len())
                .zip(remaining)
                .all(|(original, copied)| {
                    live(copied.symbol)
                        && self.symbols.get(copied.symbol).generated_from == original.symbol
                        && original.kind == copied.kind
                        && original.bounds == copied.bounds
                })
        {
            return Err("specialization changed its retained prefix or remaining binders");
        }
        let parameters = self
            .data_type_parameters
            .span(bound.type_parameters)
            .ok_or("specialization binding declaration has a stale parameter list")?;
        if parameters.len() != prefix.len() {
            return Err("specialization prefix does not cover its complete binding declaration");
        }
        for (original, retained) in parameters.iter().zip(prefix.iter()) {
            if !live(original.symbol)
                || original.symbol != retained.symbol
                || original.bounds != retained.bounds
            {
                return Err("specialization prefix changed its exact binder identity or bounds");
            }
            let same = self.copied_parameter_kinds_match(
                &original.kind, declaration, &retained.kind, receipt.template
            )?;
            if !same {
                return Err(
                    "specialization prefix changed a binder kind, carrier or complete contract",
                );
            }
        }
        let parameters = self.machine_callable_parameters(bound);
        let bindings = self.static_machine_application_bindings(receipt.binding_application);
        let arguments = self.static_machine_application_arguments(receipt.binding_application);
        if parameters.len() != bindings.len() || parameters.len() != arguments.len() {
            return Err("specialization binding declaration has an incomplete argument tuple");
        }
        for parameter in &parameters {
            let mut selected = bindings
                .iter()
                .filter(|binding| binding.parameter == parameter.symbol);
            let (Some(binding), None) = (selected.next(), selected.next()) else {
                return Err(
                    "specialization binding declaration has no unique retained parameter binding",
                );
            };
            if !matches!(
                (&parameter.kind, &binding.value),
                (TypeParameterKind::Type, StaticMachineBindingValue::Type(_))
                    | (
                        TypeParameterKind::Const { .. } | TypeParameterKind::Value { .. },
                        StaticMachineBindingValue::Const(_)
                    )
                    | (
                        TypeParameterKind::Machine { .. },
                        StaticMachineBindingValue::Machine(_)
                    )
                    | (
                        TypeParameterKind::Implementation,
                        StaticMachineBindingValue::Conformance(_)
                    )
            ) {
                return Err(
                    "specialization retained binding differs from its declared parameter kind",
                );
            }
        }
        Ok(declaration)
    }

    /// A copied binder keeps its carrier or its complete callback contract
    /// even when preparation allocates new handles for those source types.
    pub fn copied_parameter_kinds_match(
        &self,
        left: &crate::source_state::data::TypeParameterKind,
        left_owner: SymbolHandle,
        right: &crate::source_state::data::TypeParameterKind,
        right_owner: SymbolHandle,
    ) -> Result<bool, &'static str> {
        use crate::source_state::data::TypeParameterKind;
        Ok(match (left, right) {
            (
                TypeParameterKind::Const {
                    type_reference: left,
                },
                TypeParameterKind::Const {
                    type_reference: right,
                },
            )
            | (
                TypeParameterKind::Value {
                    type_reference: left,
                },
                TypeParameterKind::Value {
                    type_reference: right,
                },
            ) => self.specialization_carriers_match(*left, left_owner, *right, right_owner)?,
            // Copying a callback binder re-inserts its trait arguments and
            // signature, so `ElementStep<Element>::apply` keeps its meaning
            // under new handles. Compare the normalized contracts, like the
            // carriers above.
            (
                TypeParameterKind::Machine { contract: left },
                TypeParameterKind::Machine { contract: right },
            ) => {
                left == right
                    || self.specialization_contracts_match(left, left_owner, right, right_owner)?
            }
            (left, right) => left == right,
        })
    }

    fn specialization_contracts_match(
        &self,
        left: &crate::source_state::data::MachineParameterContract,
        left_owner: SymbolHandle,
        right: &crate::source_state::data::MachineParameterContract,
        right_owner: SymbolHandle,
    ) -> Result<bool, &'static str> {
        use crate::source_state::type_identity::identity_context::{
            TypeIdentityContext, TypeIdentityQualification,
        };
        use std::cell::Cell;
        let identity = |contract, owner| {
            if !self.symbols.symbols().nodes().is_valid(owner) {
                return Err("callable parameter contract lost its supplying declaration");
            }
            let invalid = Cell::new(false);
            let identity = crate::source_state::runtime_callable_identity::machine_contract(
                self,
                contract,
                &TypeIdentityContext {
                    application_owner: owner,
                    application_argument_owner: owner,
                    invalid_application: Some(&invalid),
                    encountered_elided_regions: Some(&invalid),
                    qualification: TypeIdentityQualification::PackageQualified,
                    ..Default::default()
                },
            )
            .map_err(|_| "callable parameter contract has no normalized identity")?;
            if invalid.get() {
                return Err("callable parameter contract has an incomplete scoped identity");
            }
            Ok(identity)
        };
        Ok(identity(left, left_owner)? == identity(right, right_owner)?)
    }

    fn specialization_carriers_match(
        &self,
        left: crate::source_state::types::TypeReferenceHandle,
        left_owner: SymbolHandle,
        right: crate::source_state::types::TypeReferenceHandle,
        right_owner: SymbolHandle,
    ) -> Result<bool, &'static str> {
        use crate::source_state::type_identity::identity_context::{
            TypeIdentityContext, TypeIdentityQualification, normalize_type_identity,
        };
        use std::cell::Cell;
        let identity = |reference, owner| {
            if !self.type_reference_table.contains_type_reference(reference) {
                return Err("specialization prefix has a stale carrier reference");
            }
            let retained_owner = self.type_reference_table.operand_lifetime_owner(reference);
            let owner = if retained_owner.is_valid() {
                retained_owner
            } else {
                owner
            };
            if !self.symbols.symbols().nodes().is_valid(owner) {
                return Err("specialization prefix carrier lost its supplying declaration");
            }
            let invalid = Cell::new(false);
            let identity = normalize_type_identity(
                self,
                reference,
                &TypeIdentityContext {
                    application_owner: owner,
                    application_argument_owner: owner,
                    invalid_application: Some(&invalid),
                    encountered_elided_regions: Some(&invalid),
                    qualification: TypeIdentityQualification::PackageQualified,
                    ..Default::default()
                },
            );
            if invalid.get() {
                return Err("specialization prefix carrier has an incomplete scoped identity");
            }
            Ok(identity)
        };
        Ok(identity(left, left_owner)? == identity(right, right_owner)?)
    }

    /// Recover the caller's input when an applied callable contains a type
    /// supplied at a different lexical scope. A retained region owner by
    /// itself cannot authorize a declaration read. The actual invocation
    /// must carry the same type from that owner.
    pub fn supplied_invocation_type(
        &self,
        reader: SymbolHandle,
        reference: crate::source_state::types::TypeReferenceHandle,
    ) -> Result<
        Option<(
            SymbolHandle,
            crate::source_state::types::TypeReferenceHandle,
        )>,
        &'static str,
    > {
        use crate::source_state::static_applications::StaticMachineBindingValue;

        let source = self.type_reference_table.operand_lifetime_owner(reference);
        if !source.is_valid() || source == reader {
            return Ok(None);
        }
        for frame in self.enclosing_invocation_frames(reader)? {
            if frame.argument_owner != source {
                continue;
            }
            for binding in frame.bindings {
                let StaticMachineBindingValue::Type(actual) = binding.value else {
                    continue;
                };
                let table = &self.type_reference_table;
                if actual == reference
                    || (table.operand_lifetime_owner(actual) == source
                        && table.type_symbol(actual) == table.type_symbol(reference)
                        && table.operand_origin(actual) == table.operand_origin(reference)
                        && table
                            .type_alias_applications(actual)
                            .eq(table.type_alias_applications(reference))
                        && self.specialization_carriers_match(actual, source, reference, source)?)
                {
                    return Ok(Some((source, actual)));
                }
            }
        }
        Ok(None)
    }

    /// Read enclosing applications from their exact instance receipts, from
    /// the immediate caller out to the original supplying declaration.
    pub fn enclosing_invocation_frames(
        &self,
        mut owner: SymbolHandle,
    ) -> Result<Vec<EnclosingInvocationFrame<'_>>, &'static str> {
        let mut frames = Vec::new();
        let mut visited = Vec::new();
        while owner.is_valid() {
            if visited.contains(&owner) {
                return Err("enclosing invocation applications contain a cycle");
            }
            visited.push(owner);
            let Some(receipt) = self.machine_instantiation(owner)? else {
                break;
            };
            let (application, declaration, argument_owner, lifetimes) = match receipt {
                MachineInstantiation::Specialization(receipt) => {
                    let application = receipt.binding_application;
                    if !application.is_valid() {
                        return Err("enclosing specialization lost its binding application");
                    }
                    let declaration = self.specialization_binding_declaration(receipt)?;
                    let source = self.static_machine_application(application);
                    (
                        application,
                        declaration,
                        source.lexical_owner,
                        self.static_application_lifetimes_for_declaration(
                            application,
                            declaration,
                        )?
                        .into_owned(),
                    )
                }
                MachineInstantiation::Capture(receipt, _) => {
                    let application = receipt.binding_application();
                    let selected = &receipt.conformance_application;
                    let scope = selected
                        .trait_scope
                        .scope(self)
                        .ok_or("enclosing capture lost its supplying scope")?;
                    let declaration = self
                        .conformance_by_symbol(selected.declaration)
                        .ok_or("enclosing capture lost its implementation declaration")?;
                    if !application.is_valid()
                        || scope.declaration.symbol != selected.trait_definition
                        || declaration.lifetime_parameters.len()
                            != selected.lifetime_arguments.len()
                    {
                        return Err("enclosing capture changed its complete application");
                    }
                    let lifetimes = declaration
                        .lifetime_parameters
                        .iter()
                        .zip(&selected.lifetime_arguments)
                        .map(|(parameter, argument)| StaticMachineLifetimeBinding {
                            parameter: parameter.clone(),
                            argument: crate::source_state::name::Identifier::generated(argument),
                        })
                        .collect();
                    (
                        application,
                        selected.declaration,
                        scope.argument_owner,
                        lifetimes,
                    )
                }
            };
            let source = self.static_machine_application(application);
            if source.binding_declaration != declaration
                || !self.symbols.symbols().nodes().is_valid(argument_owner)
            {
                return Err("enclosing invocation changed its declaration or supplier");
            }
            frames.push(EnclosingInvocationFrame {
                declaration,
                argument_owner,
                bindings: self.static_machine_application_bindings(application),
                lifetimes,
            });
            // Inferred instances without an authored outer application
            // retain themselves as the terminal supplying owner.
            if source.lexical_owner == owner {
                break;
            }
            owner = source.lexical_owner;
        }
        Ok(frames)
    }

    /// Trace a copied declaration through its actual parents to an owned member
    /// of this map. Every copied parent and entry must match its source.
    pub fn captured_conformance_owner(
        &self,
        selection: &CapturedMachineApplication,
        declaration: &crate::source_state::CapturedMachineDeclaration,
    ) -> Option<&crate::source_state::trait_definition::Conformance> {
        let owner = self.conformance_by_symbol(selection.conformance_application.declaration)?;
        if !std::ptr::eq(
            selection.declaration(declaration.template).ok()??,
            declaration,
        ) {
            return None;
        }
        let source = self.machine_by_symbol(declaration.template)?;
        let instance = self.machine_by_symbol(declaration.instance)?;
        if self.machine_callable_identity_text(source).as_deref()
            != Some(declaration.normalized_template_identity.as_str())
            || !self
                .machine_type_parameters(source)
                .into_iter()
                .map(|parameter| (&parameter.kind, &parameter.bounds))
                .eq(self
                    .machine_type_parameters(instance)
                    .into_iter()
                    .map(|parameter| (&parameter.kind, &parameter.bounds)))
        {
            return None;
        }
        let binders = |machine| {
            self.machine_type_parameters(machine)
                .into_iter()
                .map(|parameter| parameter.symbol)
                .chain(machine.conformance_bounds.iter().map(|bound| bound.binder))
        };
        if !binders(source).eq(declaration.parameters.iter().map(|(original, _)| *original))
            || !binders(instance).eq(declaration.parameters.iter().map(|(_, copied)| *copied))
            || declaration.parameters.iter().any(|(original, copied)| {
                !self.symbols.symbols().nodes().is_valid(*copied)
                    || self.symbols.get(*copied).generated_from != *original
            })
        {
            return None;
        }
        let mut body = declaration;
        for _ in 0..selection.declarations.len() {
            let symbols = self.symbols.symbols().nodes();
            if body.instance == body.template
                || !symbols.is_valid(body.instance)
                || !symbols.is_valid(body.template)
                || self.symbols.get(body.instance).generated_from != body.template
            {
                return None;
            }
            if let Some((implementation, _)) = self.owned_conformance_realization(body.template) {
                return (implementation.symbol == owner.symbol).then_some(owner);
            }
            let parent = self.symbols.get(body.instance).parent;
            let source_parent = self.symbols.get(body.template).parent;
            if !symbols.is_valid(parent) || self.symbols.get(parent).generated_from != source_parent
            {
                return None;
            }
            let (parent_machine, source_machine) = match self.symbols.get(parent).kind {
                symbols::SymbolKind::Machine => (parent, source_parent),
                symbols::SymbolKind::State => {
                    let enclosing = self.symbols.get(parent).parent;
                    let receipt = self.machine_instantiation(enclosing).ok()??;
                    if receipt.entry_template(self, parent).ok()? != source_parent {
                        return None;
                    }
                    (enclosing, receipt.template())
                }
                _ => return None,
            };
            let MachineInstantiation::Capture(actual, ancestor) =
                self.machine_instantiation(parent_machine).ok()??
            else {
                return None;
            };
            if !std::ptr::eq(actual, selection) || ancestor.template != source_machine {
                return None;
            }
            body = ancestor;
        }
        None
    }

    pub fn captured_declarations(
        &self,
    ) -> impl Iterator<
        Item = (
            &CapturedMachineApplication,
            &crate::source_state::CapturedMachineDeclaration,
        ),
    > {
        self.captured_machine_applications
            .iter()
            .flat_map(|(_, application)| {
                application
                    .declarations
                    .iter()
                    .map(move |declaration| (application, declaration))
            })
    }

    pub fn machine_instantiation(
        &self,
        instance: SymbolHandle,
    ) -> Result<Option<MachineInstantiation<'_>>, &'static str> {
        let mut receipts = self
            .machine_specializations
            .for_instance(instance)
            .map(MachineInstantiation::Specialization)
            .chain(
                self.captured_declarations()
                    .filter(|(_, declaration)| declaration.instance == instance)
                    .map(|(receipt, declaration)| {
                        MachineInstantiation::Capture(receipt, declaration)
                    }),
            );
        let selected = receipts.next();
        if receipts.next().is_some() {
            return Err("callable has multiple capture or specialization receipts");
        }
        Ok(selected)
    }

    pub fn machine_instantiation_path(
        &self,
        instance: SymbolHandle,
        template: SymbolHandle,
    ) -> Option<Vec<MachineInstantiation<'_>>> {
        let mut current = instance;
        let mut visited = Vec::new();
        let mut path = Vec::new();
        while current != template {
            if !current.is_valid() || visited.contains(&current) {
                return None;
            }
            visited.push(current);
            let receipt = self.machine_instantiation(current).ok()??;
            current = receipt.template();
            path.push(receipt);
        }
        Some(path)
    }

    /// Follow the actual copy, including a helper specialized again at its
    /// own arguments, to the map application that owns its declaration tree.
    pub fn enclosing_map_capture(
        &self,
        machine: SymbolHandle,
    ) -> Result<Option<&CapturedMachineApplication>, &'static str> {
        let authored = self.lifetime_declaration_owner(machine);
        let path = self
            .machine_instantiation_path(machine, authored)
            .ok_or("lexical caller lost its declaration correspondence")?;
        let mut captures = path.into_iter().filter_map(|receipt| match receipt {
            MachineInstantiation::Capture(selection, body) => Some((selection, body)),
            _ => None,
        });
        let Some((selection, body)) = captures.next() else {
            return Ok(None);
        };
        if captures.next().is_some() || self.captured_conformance_owner(selection, body).is_none() {
            return Err("lexical caller lost its unique owning map application");
        }
        Ok(Some(selection))
    }

    pub fn instantiated_machine_state_matches(
        &self,
        instance: SymbolHandle,
        template: SymbolHandle,
        source_state: SymbolHandle,
        instance_state: SymbolHandle,
    ) -> bool {
        let Some(path) = self.machine_instantiation_path(instance, template) else {
            return false;
        };
        let mut current = instance_state;
        for receipt in path {
            let Ok(template) = receipt.entry_template(self, current) else {
                return false;
            };
            current = template;
        }
        current == source_state
    }
}

impl TypedTrees {
    pub fn static_machine_binding_position(
        &self,
        owner: SymbolHandle,
        binder: SymbolHandle,
        target: SymbolHandle,
    ) -> Result<Option<u32>, &'static str> {
        self.static_machine_binding(owner, binder, target)
            .map(|binding| binding.map(|(position, _)| position))
    }

    /// Return the selected operand with its binder position. Its authored
    /// selection supplies visibility even after a library forwards it.
    pub fn static_machine_binding(
        &self,
        mut owner: SymbolHandle,
        binder: SymbolHandle,
        target: SymbolHandle,
    ) -> Result<Option<(u32, &crate::source_state::expression::StaticMachineArgument)>, &'static str>
    {
        let valid = |symbol| self.symbols.symbols().nodes().is_valid(symbol);
        if !valid(binder)
            || !valid(target)
            || self.retained_static_machine_contract(binder).is_none()
        {
            return Ok(None);
        }
        let entry = |symbol| {
            if !valid(symbol) {
                return None;
            }
            if self.symbols.get(symbol).kind == symbols::SymbolKind::Machine {
                self.machine_by_symbol(symbol)
                    .and_then(|machine| self.machine_states(machine).first())
                    .map(|state| state.symbol)
            } else {
                (self.symbols.get(symbol).kind == symbols::SymbolKind::State).then_some(symbol)
            }
        };
        let mut visited = Vec::new();
        loop {
            if visited.contains(&owner) {
                return Err("static call has cyclic invocation ownership");
            }
            visited.push(owner);
            let mut instances = self.machine_specializations.for_instance(owner);
            let Some(instance) = instances.next() else {
                return Ok(None);
            };
            if instances.next().is_some() {
                return Err("static call has ambiguous invocation ownership");
            }
            if let Some(
                crate::source_state::static_applications::StaticMachineBindingValue::Machine(
                    argument,
                ),
            ) = self.specialization_binding(instance, binder)
            {
                if argument.application.is_some()
                    || entry(argument.symbol).is_none_or(|selected| entry(target) != Some(selected))
                {
                    return Ok(None);
                }
                return self
                    .data_type_parameters
                    .span_or_empty(instance.template_parameters)
                    .iter()
                    .position(|parameter| {
                        parameter.symbol == binder
                            && matches!(
                                parameter.kind,
                                crate::source_state::data::TypeParameterKind::Machine { .. }
                            )
                    })
                    .map(u32::try_from)
                    .transpose()
                    .map_err(|_| "static call has too many application binders")
                    .map(|position| position.map(|position| (position, argument)));
            }
            owner = instance.template;
        }
    }
}
