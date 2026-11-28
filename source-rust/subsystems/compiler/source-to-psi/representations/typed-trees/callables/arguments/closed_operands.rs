use crate::source_state::{TypedTrees, types};
use symbols::SymbolHandle;

#[derive(Default)]
struct ClosedOperands {
    symbols: Vec<SymbolHandle>,
    types: Vec<types::TypeReferenceHandle>,
}

struct OperandClosure<'program, 'scope> {
    program: &'program TypedTrees,
    operands: ClosedOperands,
    scope: Option<crate::source_state::trait_definition::AppliedTypeReference<'scope>>,
    visited_types: Vec<(
        types::TypeReferenceHandle,
        Option<crate::source_state::trait_definition::AppliedTypeReference<'scope>>,
    )>,
    visited_expressions: Vec<(
        crate::source_state::expression::ExpressionHandle,
        Option<crate::source_state::trait_definition::AppliedTypeReference<'scope>>,
    )>,
}

impl Extend<SymbolHandle> for OperandClosure<'_, '_> {
    fn extend<Symbols: IntoIterator<Item = SymbolHandle>>(&mut self, symbols: Symbols) {
        for symbol in symbols {
            if let Some(actual) = self
                .scope
                .and_then(|scope| scope.parameter_argument(self.program, symbol))
            {
                let previous = self.scope.replace(actual);
                crate::source_state::references::visit_type_reference(
                    self.program,
                    self,
                    actual.reference,
                );
                self.scope = previous;
            } else {
                self.operands.symbols.push(symbol);
            }
        }
    }
}

impl crate::source_state::references::TypedReferences for OperandClosure<'_, '_> {
    fn type_reference(&mut self, reference: types::TypeReferenceHandle) -> bool {
        let key = (reference, self.scope);
        if self.visited_types.contains(&key) {
            return false;
        }
        self.visited_types.push(key);
        if let Some(applied) = self.scope.map(|scope| scope.child(self.program, reference))
            && applied.reference != reference
        {
            let previous = self.scope.replace(applied);
            crate::source_state::references::visit_type_reference(
                self.program,
                self,
                applied.reference,
            );
            self.scope = previous;
            return false;
        }
        self.operands.types.push(reference);
        let symbol = match self.program.type_reference_table.type_reference(reference) {
            types::TypeReferenceNode::Named { symbol, .. } => *symbol,
            types::TypeReferenceNode::Generic { base_symbol, .. } => *base_symbol,
            _ => SymbolHandle::invalid(),
        };
        if let Some(origin) = self
            .program
            .data_definition_by_symbol(symbol)
            .and_then(|definition| definition.generic_instance)
        {
            crate::source_state::references::visit_type_reference(self.program, self, origin);
        }
        true
    }
    fn expression(
        &mut self,
        expression: crate::source_state::expression::ExpressionHandle,
    ) -> bool {
        let key = (expression, self.scope);
        if self.visited_expressions.contains(&key) {
            return false;
        }
        self.visited_expressions.push(key);
        true
    }
}

impl TypedTrees {
    pub fn require_declared_application_operands(
        &self,
        arguments: &[types::TypeReferenceHandle],
        lexical_owner: SymbolHandle,
    ) -> Result<(), &'static str> {
        self.require_application_operand_bindings(arguments, lexical_owner, true, None)
    }

    pub fn require_owned_application_operands(
        &self,
        arguments: &[types::TypeReferenceHandle],
        lexical_owner: SymbolHandle,
    ) -> Result<(), &'static str> {
        self.require_application_operand_bindings(arguments, lexical_owner, false, None)
    }

    pub fn require_receiving_trait_operands(
        &self,
        scope: &crate::source_state::trait_definition::AppliedTraitScope<'_>,
    ) -> Result<(), &'static str> {
        self.require_trait_operand_bindings(scope, false)
    }

    fn require_trait_operand_bindings(
        &self,
        scope: &crate::source_state::trait_definition::AppliedTraitScope<'_>,
        allow_declared_binders: bool,
    ) -> Result<(), &'static str> {
        for parameter in self.trait_type_parameters(scope.declaration) {
            let (actual, preceding_frames) = scope
                .parameter_application(parameter.symbol)
                .ok_or("closed requirement omits a declaring trait operand")?;
            self.require_application_operand_bindings(
                &[actual],
                scope.argument_owner,
                allow_declared_binders,
                Some((scope, preceding_frames)),
            )?;
        }
        Ok(())
    }

    fn require_application_operand_bindings(
        &self,
        arguments: &[types::TypeReferenceHandle],
        lexical_owner: SymbolHandle,
        allow_declared_binders: bool,
        scope: Option<(
            &crate::source_state::trait_definition::AppliedTraitScope<'_>,
            usize,
        )>,
    ) -> Result<(), &'static str> {
        let mut closure = OperandClosure {
            program: self,
            operands: ClosedOperands::default(),
            scope: None,
            visited_types: Vec::new(),
            visited_expressions: Vec::new(),
        };
        for argument in arguments.iter() {
            closure.scope = scope
                .map(|(scope, preceding_frames)| {
                    scope
                        .supplied_type_reference(self, *argument, preceding_frames)
                        .ok_or("closed requirement lost its supplying operand prefix")
                })
                .transpose()?;
            let supplied = closure.scope.map_or(*argument, |view| view.reference);
            crate::source_state::references::visit_type_reference(self, &mut closure, supplied);
        }
        let operands = closure.operands;
        if operands
            .symbols
            .iter()
            .filter(|symbol| symbol.is_valid())
            .any(|symbol| {
                if allow_declared_binders
                    && self
                        .data_type_parameter_by_symbol(*symbol)
                        .is_some_and(|parameter| {
                            matches!(
                                parameter.kind,
                                crate::source_state::data::TypeParameterKind::Type
                                    | crate::source_state::data::TypeParameterKind::Const { .. }
                            )
                        })
                {
                    let parent = self.symbols.get(*symbol).parent;
                    let mut owner = lexical_owner;
                    while owner.is_valid() {
                        if parent == owner
                            || self
                                .owned_conformance_realization(owner)
                                .is_some_and(|(map, _)| map.symbol == parent)
                        {
                            return false;
                        }
                        owner = self.symbols.get(owner).parent;
                    }
                }
                matches!(
                    self.symbols.get(*symbol).kind,
                    symbols::SymbolKind::TypeParameter
                        | symbols::SymbolKind::MachineParameter
                        | symbols::SymbolKind::ConformanceParameter
                        | symbols::SymbolKind::PropositionParameter
                        | symbols::SymbolKind::PropositionMachineParameter
                        | symbols::SymbolKind::Local
                        | symbols::SymbolKind::Parameter
                )
            })
        {
            return Err("open operands do not close a requirement instance");
        }
        for reference in &operands.types {
            if let types::TypeReferenceNode::Named { symbol, .. } =
                self.type_reference_table.type_reference(*reference)
                && let Some(definition) = self.data_definition_by_symbol(*symbol)
                && (!definition.type_parameters.is_empty()
                    || !definition.lifetime_parameters.is_empty())
            {
                return Err("closed requirement operand omits a nominal telescope");
            }
        }
        for argument in arguments.iter().copied() {
            let exact_regions = if let Some((scope, preceding_frames)) = scope {
                scope
                    .with_identity_scopes(self, |frames| {
                        crate::source_state::type_identity::applied_type_identity(
                            self,
                            argument,
                            scope.argument_owner,
                            &frames[..preceding_frames],
                        )
                    })
                    .is_some()
            } else if lexical_owner.is_valid() {
                crate::source_state::type_identity::applied_type_identity(
                    self,
                    argument,
                    lexical_owner,
                    &[],
                )
                .is_some()
            } else {
                false
            };
            if exact_regions {
                continue;
            }
            if self
                .region_free_scoped_type_identity(
                    crate::source_state::type_identity::TypeIdentityRequest::ordinary(argument),
                    &[],
                )
                .is_none()
            {
                return Err("closed requirement needs a retained exact region application");
            }
        }
        Ok(())
    }
}
