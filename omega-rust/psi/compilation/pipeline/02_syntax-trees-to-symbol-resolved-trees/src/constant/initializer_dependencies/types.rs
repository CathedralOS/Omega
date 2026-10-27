//! Type work belongs to the demanded computation too. A selected record's
//! extent may depend on a constant even when its initializer is never read as
//! an expression. Discover that dependency before probe placeholders exist.

use super::Collector;
use arena::{Handle, HandleSpan};
use source::SourceSpan;
use symbol_resolved_trees::{
    data::{DataMember, MachineParameterContract, TypeParameter, TypeParameterKind},
    expression::StaticMachineArgument,
    signature::StateParameter,
    types::{FixedArrayLength, TypeConstraint, TypeReference},
};
use symbols::{SymbolHandle, SymbolKind};

impl<'program> Collector<'program> {
    pub(super) fn constant(
        &mut self,
        symbol: SymbolHandle,
        reference: SourceSpan,
    ) -> Result<(), String> {
        let mut declarations = self
            .program
            .const_declarations
            .iter()
            .filter(|declaration| declaration.symbol == symbol);
        let declaration = declarations
            .next()
            .ok_or("constant dependency lost its retained declaration")?;
        if declarations.next().is_some() {
            return Err("constant dependency has ambiguous declaration custody".to_owned());
        }
        let source = self
            .program
            .symbols
            .symbol_source_span(symbol)
            .ok_or("constant dependency lost its declaration source")?;
        let dependency = (reference, source);
        if !self.dependencies.constants.contains(&dependency) {
            self.dependencies.constants.push(dependency);
        }
        if self.follow_dependencies && !self.expanded_constants.contains(&symbol) {
            self.expanded_constants.push(symbol);
            self.types.push(&declaration.declared_type);
            let initializer = if declaration.authored_initializer.is_valid() {
                declaration.authored_initializer
            } else {
                declaration.initializer
            };
            self.expressions.push(initializer);
            if !self.initializer_expressions.contains(&initializer) {
                self.initializer_expressions.push(initializer);
            }
        }
        Ok(())
    }

    pub(super) fn type_child(&mut self, handle: Handle<TypeReference>) -> Result<(), String> {
        if !self.follow_dependencies || self.visited_type_children.contains(&handle) {
            return Ok(());
        }
        let children = &self.program.tables.declarations.child_type_references;
        if !children.is_valid(handle) {
            return Err("constant dependency has a stale child type reference".to_owned());
        }
        self.visited_type_children.push(handle);
        self.types.push(children.get(handle));
        Ok(())
    }

    pub(super) fn type_arguments(
        &mut self,
        arguments: HandleSpan<TypeReference>,
    ) -> Result<(), String> {
        if !self.follow_dependencies {
            return Ok(());
        }
        let children = self.program.child_type_references(arguments);
        if children.len() != arguments.len() {
            return Err("constant dependency has stale type arguments".to_owned());
        }
        for argument in arguments.handles() {
            self.type_child(argument)?;
        }
        Ok(())
    }

    pub(super) fn type_reference(
        &mut self,
        reference: &'program TypeReference,
    ) -> Result<(), String> {
        match reference {
            TypeReference::Reference(reference) => self.type_child(reference.referee)?,
            TypeReference::Slice(reference) => self.type_child(reference.element_type)?,
            TypeReference::FixedArray(array) => {
                self.type_child(array.element_type)?;
                match &array.length {
                    FixedArrayLength::ConstParameter { symbol, name }
                        if self.program.symbols.get(*symbol).kind == SymbolKind::Const =>
                    {
                        self.constant(*symbol, name.source_span())?;
                    }
                    FixedArrayLength::Expression(value) => self.type_child(*value)?,
                    FixedArrayLength::ConstCall { machine, name } => {
                        self.call(*machine, name.source_span())?;
                    }
                    // A lexical const binder is not an authored declaration.
                    // Its argument is traversed at the selected application;
                    // never recover a same-spelled outer constant here.
                    FixedArrayLength::ConstParameter { .. } | FixedArrayLength::Literal(_) => {}
                }
            }
            TypeReference::Constrained(reference) => {
                self.type_child(reference.base_type)?;
                let constraints = self
                    .program
                    .tables
                    .types
                    .constraints
                    .span_or_empty(reference.constraints);
                if constraints.len() != reference.constraints.len() {
                    return Err("constant dependency has stale type constraints".to_owned());
                }
                for constraint in constraints {
                    match constraint {
                        TypeConstraint::Range {
                            minimum, maximum, ..
                        } => {
                            self.expressions.extend(
                                [*minimum, *maximum]
                                    .into_iter()
                                    .filter(|expression| expression.is_valid()),
                            );
                        }
                        TypeConstraint::Domain(domain) => self.type_arguments(domain.arguments)?,
                        TypeConstraint::Named(_) | TypeConstraint::ArithmeticDomain(_) => {}
                    }
                }
            }
            TypeReference::Generic(application) => {
                self.type_declaration(application.base_symbol)?;
                self.type_arguments(application.arguments)?;
            }
            TypeReference::Named { symbol, name, .. }
                if self.program.symbols.get(*symbol).kind == SymbolKind::Const =>
            {
                self.constant(*symbol, name.source_span())?;
            }
            TypeReference::Named { symbol, .. } => {
                self.type_declaration(*symbol)?;
            }
            TypeReference::ConstExpression(expression) => self.expressions.push(*expression),
            TypeReference::DynamicTrait { .. } | TypeReference::Unit => {}
        }
        Ok(())
    }

    pub(super) fn type_declaration(&mut self, symbol: SymbolHandle) -> Result<(), String> {
        if self.program.symbols.get(symbol).kind == SymbolKind::Conformance {
            return self.conformance_declaration(symbol);
        }
        if !self.follow_dependencies
            || self.program.symbols.get(symbol).kind != SymbolKind::Data
            || self.visited_type_declarations.contains(&symbol)
        {
            return Ok(());
        }
        self.visited_type_declarations.push(symbol);
        let mut definitions = self
            .program
            .data_definitions
            .iter()
            .filter(|definition| definition.symbol == symbol);
        let definition = definitions
            .next()
            .ok_or("constant dependency lost its selected data declaration")?;
        if definitions.next().is_some() {
            return Err("constant dependency has ambiguous data declaration custody".to_owned());
        }
        self.type_parameters(definition.type_parameters)?;
        self.predicate_facts(definition.where_facts)?;
        if let Some(quotient) = &definition.quotient {
            self.types.push(&quotient.carrier);
        }
        let members = self.program.data_members(definition.members);
        if members.len() != definition.members.len() {
            return Err("constant dependency has stale data members".to_owned());
        }
        for member in members {
            match member {
                DataMember::Field(field) => self.types.push(&field.type_reference),
                DataMember::Variant(variant) => {
                    self.predicate_facts(variant.where_facts)?;
                    let fields = self.program.data_payload_fields(variant.payload);
                    if fields.len() != variant.payload.len() {
                        return Err("constant dependency has stale variant fields".to_owned());
                    }
                    self.types
                        .extend(fields.iter().map(|field| &field.type_reference));
                }
            }
        }
        Ok(())
    }

    fn conformance_declaration(&mut self, symbol: SymbolHandle) -> Result<(), String> {
        if !self.follow_dependencies
            || self.purpose != super::DependencyPurpose::DeclarationReadiness
            || self.visited_type_declarations.contains(&symbol)
        {
            return Ok(());
        }
        let definitions = self
            .program
            .conformances
            .iter()
            .filter(|definition| definition.symbol == symbol)
            .collect::<Vec<_>>();
        let [definition] = definitions.as_slice() else {
            return Err(
                "constant dependency lost its exact selected implementation declaration".into(),
            );
        };
        self.visited_type_declarations.push(symbol);
        self.type_parameters(definition.type_parameters)?;
        self.type_arguments(definition.arguments)?;
        // A selected named implementation is a complete source member map.
        // Complete its owned declarations for readiness and checking; this
        // discovers constant dependencies but grants no invocation authority.
        for row in &definition.rows {
            let machine = crate::symbols::owned_conformance_realization(self.program, row)?;
            if crate::symbols::owned_conformance_namespace(self.program, machine)? != symbol {
                return Err(
                    "constant dependency implementation member lost its exact owner".into(),
                );
            }
            self.machines.push(machine);
        }
        Ok(())
    }

    pub(super) fn type_parameters(
        &mut self,
        parameters: HandleSpan<TypeParameter>,
    ) -> Result<(), String> {
        let parameters_slice = self
            .program
            .tables
            .declarations
            .data_type_parameters
            .span_or_empty(parameters);
        if parameters_slice.len() != parameters.len() {
            return Err("constant dependency has stale type parameters".to_owned());
        }
        for parameter in parameters_slice {
            match &parameter.kind {
                TypeParameterKind::Const { type_reference }
                | TypeParameterKind::Value { type_reference } => self.types.push(type_reference),
                TypeParameterKind::Machine {
                    contract: MachineParameterContract::Structural(signature),
                } => {
                    self.type_parameters(signature.type_parameters)?;
                    self.state_parameters(signature.parameters)?;
                    if let Some(reference) = &signature.return_type {
                        self.types.push(reference);
                    }
                }
                TypeParameterKind::Proposition { contract } => {
                    self.state_parameters(contract.parameters)?;
                }
                TypeParameterKind::Type
                | TypeParameterKind::Implementation
                | TypeParameterKind::Machine { .. } => {}
            }
        }
        Ok(())
    }

    pub(super) fn state_parameters(
        &mut self,
        parameters: HandleSpan<StateParameter>,
    ) -> Result<(), String> {
        let parameters_slice = self
            .program
            .tables
            .declarations
            .state_parameters
            .span_or_empty(parameters);
        if parameters_slice.len() != parameters.len() {
            return Err("constant dependency has stale state parameters".to_owned());
        }
        self.types.extend(
            parameters_slice
                .iter()
                .map(|parameter| &parameter.type_reference),
        );
        Ok(())
    }

    pub(super) fn static_arguments(
        &mut self,
        arguments: &'program [StaticMachineArgument],
    ) -> Result<(), String> {
        if !self.follow_dependencies {
            return Ok(());
        }
        for argument in arguments {
            let selected = self.program.symbols.get(argument.symbol);
            match selected.kind {
                SymbolKind::Conformance => self.conformance_declaration(argument.symbol)?,
                SymbolKind::Machine => self.machines.push(argument.symbol),
                SymbolKind::State
                    if self.program.symbols.get(selected.parent).kind == SymbolKind::Machine =>
                {
                    self.machines.push(selected.parent);
                }
                _ => {}
            }
            if argument.type_reference.is_valid() {
                self.type_child(argument.type_reference)?;
            }
            if let Some(application) = &argument.application {
                self.static_arguments(&application.arguments)?;
            }
        }
        Ok(())
    }
}
