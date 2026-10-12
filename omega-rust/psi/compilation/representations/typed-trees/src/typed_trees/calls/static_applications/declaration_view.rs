//! Borrowed original declaration and complete application correspondence.
//!
//! Declaration-order projection must survive grouped binding scratch and
//! generated public headers. This owner rejoins exact original binder symbols
//! and original lifetime formal subjects; it does not evaluate operands, admit
//! bounds, select providers, serialize local application equality, or publish
//! execution authority. Missing producer custody is an error, never Empty.

use super::{
    StaticMachineApplication, StaticMachineApplicationHandle, StaticMachineBinding,
    StaticMachineBindingValue,
};
use crate::{TypedTrees, data::TypeParameter, data::TypeParameterKind, name::Identifier};
use symbols::SymbolHandle;

#[derive(Clone, Copy)]
pub struct DeclaredStaticLifetime<'declaration> {
    pub declaration: SymbolHandle,
    pub ordinal: usize,
    pub name: &'declaration Identifier,
    pub invocation: bool,
}

pub struct StaticApplicationDeclaration<'declaration> {
    symbol: SymbolHandle,
    parameters: Vec<&'declaration TypeParameter>,
    conformance_bounds: &'declaration [crate::machine::GenericConformanceBound],
    lifetimes: Vec<DeclaredStaticLifetime<'declaration>>,
}

impl<'declaration> StaticApplicationDeclaration<'declaration> {
    pub fn for_symbol(
        program: &'declaration TypedTrees,
        symbol: SymbolHandle,
    ) -> Result<Self, &'static str> {
        let mut lifetimes = Vec::new();
        let mut append_lifetimes =
            |declaration, parameters: &'declaration [Identifier], invocation| {
                lifetimes.extend(parameters.iter().enumerate().map(|(ordinal, name)| {
                    DeclaredStaticLifetime {
                        declaration,
                        ordinal,
                        name,
                        invocation,
                    }
                }));
            };
        let (symbol, parameters, conformance_bounds) = if let Some(machine) = program
            .machine(symbol)
            .or_else(|| program.machine_holding_state(symbol))
        {
            append_lifetimes(machine.symbol, &machine.lifetime_parameters, true);
            (
                machine.symbol,
                original_machine_parameters(program, machine)?
                    .iter()
                    .collect(),
                machine.conformance_bounds.as_slice(),
            )
        } else if let Some((owner, signature)) = program.trait_machine_signature(symbol) {
            // A produced public signature is not an original declaration
            // telescope. Its retained application names the exact template.
            if program
                .public_trait_requirement_signature(symbol)
                .is_some_and(|selected| selected.application.is_valid())
            {
                return Err("static application must name its original requirement declaration");
            }
            append_lifetimes(owner.symbol, &owner.lifetime_parameters, false);
            append_lifetimes(signature.symbol, &signature.lifetime_parameters, true);
            (
                signature.symbol,
                program
                    .trait_type_parameters(owner)
                    .iter()
                    .chain(program.state_signature_type_parameters(signature))
                    .collect(),
                owner.conformance_bounds.as_slice(),
            )
        } else if let Some(conformance) = program.conformance_by_symbol(symbol) {
            append_lifetimes(conformance.symbol, &conformance.lifetime_parameters, false);
            (
                conformance.symbol,
                program
                    .conformance_type_parameters(conformance)
                    .iter()
                    .collect(),
                &[][..],
            )
        } else if let Some(operator) = crate::operator::declaration_by_symbol(program, symbol) {
            append_lifetimes(operator.symbol, &operator.lifetime_parameters, true);
            (
                operator.symbol,
                program.operator_type_parameters(operator).iter().collect(),
                &[][..],
            )
        } else {
            return Err("static application has no retained declaration telescope");
        };
        Ok(Self {
            symbol,
            parameters,
            conformance_bounds,
            lifetimes,
        })
    }

    /// Original outer interface plus declaring member telescope. Inheritance
    /// keeps the member identity while its selected interface owns the outer
    /// tuple and erased-region correspondence. The plain symbol query remains
    /// the direct declaration entrance for callers without an application.
    pub fn for_application(
        program: &'declaration TypedTrees,
        application: &StaticMachineApplication,
    ) -> Result<Self, &'static str> {
        Self::for_selected_interface(
            program,
            application.binding_declaration,
            application.selected_interface,
        )
    }

    pub fn for_selected_interface(
        program: &'declaration TypedTrees,
        symbol: SymbolHandle,
        selected_interface: SymbolHandle,
    ) -> Result<Self, &'static str> {
        let mut declaration = Self::for_symbol(program, symbol)?;
        if !selected_interface.is_valid() {
            return Ok(declaration);
        }
        let (owner, signature) = program
            .trait_machine_signature(symbol)
            .ok_or("static application selected an interface for a non-trait requirement")?;
        let selected = program
            .trait_definition_by_symbol(selected_interface)
            .ok_or("static application lost its original selected interface")?;
        if selected.symbol == owner.symbol {
            return Ok(declaration);
        }
        let scope = crate::trait_definition::AppliedTraitScope::abstract_scope(selected);
        let mut reachable = false;
        scope.visit(program, &mut |scope| {
            reachable |= scope.declaration.symbol == owner.symbol;
        });
        if !reachable {
            return Err("static application interface does not contain its declaring requirement");
        }
        declaration.parameters = program
            .trait_type_parameters(selected)
            .iter()
            .chain(program.state_signature_type_parameters(signature))
            .collect();
        declaration.conformance_bounds = &selected.conformance_bounds;
        declaration.lifetimes = selected
            .lifetime_parameters
            .iter()
            .enumerate()
            .map(|(ordinal, name)| DeclaredStaticLifetime {
                declaration: selected.symbol,
                ordinal,
                name,
                invocation: false,
            })
            .chain(
                signature
                    .lifetime_parameters
                    .iter()
                    .enumerate()
                    .map(|(ordinal, name)| DeclaredStaticLifetime {
                        declaration: signature.symbol,
                        ordinal,
                        name,
                        invocation: true,
                    }),
            )
            .collect();
        Ok(declaration)
    }

    pub fn symbol(&self) -> SymbolHandle {
        self.symbol
    }
    pub fn parameters(&self) -> &[&'declaration TypeParameter] {
        &self.parameters
    }
    pub fn conformance_bounds(&self) -> &'declaration [crate::machine::GenericConformanceBound] {
        self.conformance_bounds
    }
    pub fn lifetimes(&self) -> &[DeclaredStaticLifetime<'declaration>] {
        &self.lifetimes
    }

    /// An actual occurrence supplies every original lifetime formal. Shared
    /// declaration-family normalization binds static arguments independently;
    /// it cannot discharge this invocation's supplied correspondence.
    pub fn bind_lifetimes<'application>(
        &self,
        program: &'application TypedTrees,
        handle: StaticMachineApplicationHandle,
    ) -> Result<
        Vec<(
            DeclaredStaticLifetime<'declaration>,
            &'application super::StaticMachineLifetimeBinding,
        )>,
        &'static str,
    > {
        if !program
            .expression_table
            .static_machine_application_is_valid(handle)
            || program
                .expression_table
                .static_machine_application(handle)
                .binding_declaration
                != self.symbol
        {
            return Err("static application lost its original lifetime declaration");
        }
        let source_lifetimes = program
            .expression_table
            .static_machine_application_lifetimes(handle);
        if source_lifetimes.len() != self.lifetimes.len() {
            return Err(
                "static application does not retain its complete original lifetime correspondence",
            );
        }
        let mut lifetimes = Vec::with_capacity(source_lifetimes.len());
        for formal in &self.lifetimes {
            let binding = self.lifetime_binding(source_lifetimes, *formal)?;
            if !formal.invocation && binding.argument.named().is_none() {
                return Err(
                    "captured lifetime operand has no original named declaration correspondence",
                );
            }
            lifetimes.push((*formal, binding));
        }
        Ok(lifetimes)
    }

    fn lifetime_binding<'binding>(
        &self,
        source: &'binding [super::StaticMachineLifetimeBinding],
        formal: DeclaredStaticLifetime<'_>,
    ) -> Result<&'binding super::StaticMachineLifetimeBinding, &'static str> {
        if self
            .lifetimes
            .iter()
            .filter(|candidate| candidate.name == formal.name)
            .count()
            != 1
        {
            return Err(
                "static application lifetime producer lost distinct original formal owners",
            );
        }
        let mut matches = source
            .iter()
            .filter(|binding| &binding.parameter == formal.name);
        let binding = matches
            .next()
            .ok_or("static application lost an original lifetime formal")?;
        if matches.next().is_some() {
            return Err("static application repeats an original lifetime formal");
        }
        Ok(binding)
    }

    /// The complete original operand telescope is independent of invocation
    /// region instantiation. Source satisfaction projection reads these rows
    /// while its quantified region family remains a declaration-owned axis.
    pub fn bind_arguments<'owner>(
        &self,
        program: &'owner TypedTrees,
        handle: StaticMachineApplicationHandle,
    ) -> Result<Vec<&'owner StaticMachineBinding>, &'static str> {
        if !program
            .expression_table
            .static_machine_application_is_valid(handle)
            || program
                .expression_table
                .static_machine_application(handle)
                .binding_declaration
                != self.symbol
        {
            return Err("static application lost its original declaration owner");
        }
        let source_bindings = program
            .expression_table
            .static_machine_application_bindings(handle);
        if source_bindings.len() != self.binders().count() {
            return Err(
                "static application does not retain its complete original declaration telescope",
            );
        }
        let mut bindings = Vec::with_capacity(source_bindings.len());
        for parameter in self.binders() {
            let mut matches = source_bindings
                .iter()
                .filter(|binding| binding.parameter == parameter);
            let binding = matches
                .next()
                .ok_or("static application lost an original declaration binder")?;
            if matches.next().is_some() {
                return Err("static application repeats an original declaration binder");
            }
            let parameter = self
                .parameters
                .iter()
                .find(|candidate| candidate.symbol == parameter);
            let matches_category = matches!(
                (parameter.map(|parameter| &parameter.kind), &binding.value),
                (
                    Some(TypeParameterKind::Type),
                    StaticMachineBindingValue::Type(_)
                ) | (
                    Some(TypeParameterKind::Const { .. } | TypeParameterKind::Value { .. }),
                    StaticMachineBindingValue::Const(_),
                ) | (
                    Some(TypeParameterKind::Value { .. }),
                    StaticMachineBindingValue::RuntimeValue { .. },
                ) | (
                    Some(TypeParameterKind::Machine { .. }),
                    StaticMachineBindingValue::Machine(_),
                ) | (
                    Some(TypeParameterKind::Implementation) | None,
                    StaticMachineBindingValue::Conformance(_),
                )
            );
            if !matches_category {
                return Err(
                    "static application operand does not retain its original binder category",
                );
            }
            match &binding.value {
                StaticMachineBindingValue::Type(reference)
                | StaticMachineBindingValue::Const(reference)
                | StaticMachineBindingValue::RuntimeValue {
                    type_reference: reference,
                    ..
                } if !program
                    .type_reference_table
                    .contains_type_reference(*reference) =>
                {
                    return Err("static application lost its original typed operand payload");
                }
                _ => {}
            }
            bindings.push(binding);
        }
        Ok(bindings)
    }

    pub fn binders(&self) -> impl Iterator<Item = SymbolHandle> + '_ {
        self.parameters
            .iter()
            .map(|parameter| parameter.symbol)
            .chain(
                self.conformance_bounds
                    .iter()
                    .filter(|bound| {
                        !self
                            .parameters
                            .iter()
                            .any(|parameter| parameter.symbol == bound.binder)
                    })
                    .map(|bound| bound.binder),
            )
    }
}

pub struct StaticApplicationView<'owner> {
    application: &'owner StaticMachineApplication,
    declaration: StaticApplicationDeclaration<'owner>,
    bindings: Vec<&'owner StaticMachineBinding>,
    lifetimes: Vec<(
        DeclaredStaticLifetime<'owner>,
        &'owner super::StaticMachineLifetimeBinding,
    )>,
}

impl<'owner> StaticApplicationView<'owner> {
    pub fn new(
        program: &'owner TypedTrees,
        handle: StaticMachineApplicationHandle,
    ) -> Result<Self, &'static str> {
        if !handle.is_valid() {
            return Err("static application has no original retained payload");
        }
        let application = program.expression_table.static_machine_application(handle);
        application
            .type_identity_owner(program)
            .ok_or("static application lost its original lexical capture")?;
        let declaration = StaticApplicationDeclaration::for_application(program, application)?;
        let bindings = declaration.bind_arguments(program, handle)?;
        let lifetimes = declaration.bind_lifetimes(program, handle)?;
        Ok(Self {
            application,
            declaration,
            bindings,
            lifetimes,
        })
    }

    pub fn application(&self) -> &'owner StaticMachineApplication {
        self.application
    }
    pub fn declaration(&self) -> &StaticApplicationDeclaration<'owner> {
        &self.declaration
    }
    pub fn bindings(&self) -> &[&'owner StaticMachineBinding] {
        &self.bindings
    }
    pub fn lifetimes(
        &self,
    ) -> &[(
        DeclaredStaticLifetime<'owner>,
        &'owner super::StaticMachineLifetimeBinding,
    )] {
        &self.lifetimes
    }
}

impl TypedTrees {
    /// Project one original lifetime subject without requiring unrelated member
    /// operands. Receiver regions are available independently of invocation
    /// regions, but their source row still owes unique declaration custody.
    /// This read neither closes the application nor admits its other bindings.
    pub fn static_application_lifetime_binding(
        &self,
        application: StaticMachineApplicationHandle,
        declaration: SymbolHandle,
        ordinal: usize,
    ) -> Option<&super::StaticMachineLifetimeBinding> {
        if !self
            .expression_table
            .static_machine_application_is_valid(application)
        {
            return None;
        }
        let retained = self
            .expression_table
            .static_machine_application(application);
        retained.type_identity_owner(self)?;
        let original = StaticApplicationDeclaration::for_application(self, retained).ok()?;
        let mut formals = original
            .lifetimes()
            .iter()
            .filter(|formal| formal.declaration == declaration && formal.ordinal == ordinal);
        let formal = *formals.next()?;
        if formals.next().is_some() {
            return None;
        }
        original
            .lifetime_binding(
                self.expression_table
                    .static_machine_application_lifetimes(application),
                formal,
            )
            .ok()
    }

    /// Resolve a lexical lifetime only within its original declaration scope.
    /// Duplicate formal spellings across distinct owners remain unavailable
    /// until the originating producer retains their exact correspondence.
    pub fn declaration_lifetime_parameter(
        &self,
        symbol: SymbolHandle,
        name: &str,
    ) -> Option<(SymbolHandle, usize)> {
        let mut owner = self.lifetime_declaration_owner(symbol);
        let mut visited = Vec::new();
        while owner.is_valid() {
            if visited.contains(&owner) {
                return None;
            }
            visited.push(owner);
            if let Some(parameter) = self.own_declaration_lifetime_parameter(owner, name).ok()? {
                return Some(parameter);
            }
            owner = self.lifetime_declaration_owner(self.symbols.get(owner).parent);
        }
        None
    }

    fn own_declaration_lifetime_parameter(
        &self,
        symbol: SymbolHandle,
        name: &str,
    ) -> Result<Option<(SymbolHandle, usize)>, ()> {
        if let Some(data) = self.data_definition_by_symbol(symbol) {
            return Ok(data
                .lifetime_parameters
                .iter()
                .position(|parameter| parameter.as_str() == name)
                .map(|ordinal| (data.symbol, ordinal)));
        }
        if let Some(owner) = self.trait_definition_by_symbol(symbol) {
            return Ok(owner
                .lifetime_parameters
                .iter()
                .position(|parameter| parameter.as_str() == name)
                .map(|ordinal| (owner.symbol, ordinal)));
        }
        if let Some(contract) = self.retained_static_machine_contract(symbol) {
            // A selected callable binder owns an authored invocation signature
            // without acquiring an executable Machine declaration.
            let mut matches = contract
                .signature()
                .lifetime_parameters
                .iter()
                .enumerate()
                .filter(|(_, parameter)| parameter.as_str() == name);
            let Some((ordinal, _)) = matches.next() else {
                return Ok(None);
            };
            if matches.next().is_some() {
                return Err(());
            }
            return Ok(Some((symbol, ordinal)));
        }
        let Ok(declaration) = StaticApplicationDeclaration::for_symbol(self, symbol) else {
            if self
                .public_trait_requirement_signature(symbol)
                .is_some_and(|selected| selected.application.is_valid())
            {
                return Err(());
            }
            return Ok(None);
        };
        let mut matches = declaration
            .lifetimes
            .iter()
            .filter(|parameter| parameter.name.as_str() == name);
        let Some(parameter) = matches.next() else {
            return Ok(None);
        };
        if matches.next().is_some() {
            return Err(());
        }
        Ok(Some((parameter.declaration, parameter.ordinal)))
    }
}

impl TypedTrees {
    /// Original source lifetime coordinate for portable publication. An erased
    /// machine instance rejoins its retained template family and same ordinal;
    /// disagreement or a missing cross-owner instance correspondence declines.
    pub fn original_declaration_lifetime_parameter(
        &self,
        symbol: SymbolHandle,
        name: &str,
    ) -> Option<(SymbolHandle, usize)> {
        let (mut declaration, ordinal) = self.declaration_lifetime_parameter(symbol, name)?;
        let mut visited = Vec::new();
        loop {
            if visited.contains(&declaration) {
                return None;
            }
            visited.push(declaration);
            let mut origins = self
                .machine_specializations
                .iter()
                .filter(|specialization| specialization.instance == declaration);
            let Some(first) = origins.next() else {
                if self
                    .instantiated_static_applications()
                    .any(|(_, instance)| {
                        instance.instance == declaration
                            && instance.demand.declaration != declaration
                    })
                {
                    return None;
                }
                return Some((declaration, ordinal));
            };
            if origins.any(|origin| origin.template != first.template) {
                return None;
            }
            if first.template == declaration {
                return Some((declaration, ordinal));
            }
            let source = self.machine(first.template)?;
            let instance = self.machine(declaration)?;
            if source.lifetime_parameters.len() != instance.lifetime_parameters.len()
                || ordinal >= source.lifetime_parameters.len()
            {
                return None;
            }
            declaration = source.symbol;
        }
    }
}

fn original_machine_parameters<'owner>(
    program: &'owner TypedTrees,
    machine: &'owner crate::machine::Machine,
) -> Result<&'owner [TypeParameter], &'static str> {
    let mut retained = program
        .machine_specializations
        .iter()
        .filter(|specialization| specialization.template == machine.symbol);
    let Some(first) = retained.next() else {
        return Ok(program.machine_type_parameters(machine));
    };
    if retained
        .any(|specialization| specialization.template_parameters != first.template_parameters)
    {
        return Err("static declaration has conflicting retained original telescopes");
    }
    Ok(program
        .data_type_parameters
        .span_or_empty(first.template_parameters))
}
