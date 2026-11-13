//! Form static schemas in their own binder scope and retain each exact
//! required/supplied question. A named map grants no evidence by itself.

use language_semantics::{ServiceReachRowId, ServiceReachRowTable};
use semantic_vocabulary::MachineId;
use symbols::SymbolHandle;
use terminal_check_tables::Checked;
use typed_trees::signature::StateSignature;
use typed_trees::trait_definition::{AppliedTraitScope, TraitDefinition};

use super::{Program, services};
use crate::body::ValueType;
use crate::static_contracts::{
    SourceConformance, SourceImplementation, SourceSchema, SourceSchemaRequirement,
    SourceServiceCeiling, StaticProviderCandidate, StaticSourceEnvelope,
};

fn key(symbol: SymbolHandle) -> (u32, u32) {
    (symbol.arena_index(), symbol.generation())
}

impl Program<'_> {
    fn static_envelope(
        &self,
        context: MachineId,
        signature: &StateSignature,
        boundary: bool,
    ) -> Checked<StaticSourceEnvelope> {
        let written = self
            .typed
            .authored_service_reach_rows_for(signature.symbol)
            .next()
            .is_some()
            || !matches!(
                signature.service_reach_row,
                ServiceReachRowId::NULL | ServiceReachRowTable::EMPTY_ROW
            );
        let service_reach = if written {
            if signature.service_reach_is_installation_bound {
                return Err(services::unavailable(
                    context,
                    "static service ceiling is bound to an unresolved installation",
                ));
            }
            SourceServiceCeiling::Closed(services::finite_reach(
                self.typed,
                context,
                signature.service_reach_row,
            )?)
        } else if boundary {
            return Err(services::unavailable(
                context,
                "boundary requirement has no authored finite service ceiling",
            ));
        } else {
            SourceServiceCeiling::Open
        };
        let authored = self
            .typed
            .signature_invokes
            .span(signature.invokes)
            .ok_or_else(|| {
                services::unavailable(context, "static invocation ceiling has a stale span")
            })?;
        let parameters = self
            .typed
            .state_parameters
            .span(signature.parameters)
            .ok_or_else(|| {
                services::unavailable(context, "static signature has a stale formal span")
            })?;
        Ok(StaticSourceEnvelope {
            service_reach,
            invocations: services::invocations(self.typed, context, authored, parameters)?,
            suspends: signature.suspends,
            blocks: signature.blocks,
            termination: services::termination(context, &signature.termination_guarantee)?,
        })
    }

    pub(super) fn trait_declaration(
        &mut self,
        context: MachineId,
        symbol: SymbolHandle,
    ) -> Checked<bool> {
        let typed = self.typed;
        let Some(declaration) = typed.trait_definition_by_symbol(symbol) else {
            return Ok(false);
        };
        let position = match self
            .static_contracts
            .schemas
            .binary_search_by_key(&key(symbol), |schema| key(schema.declaration))
        {
            Ok(_) => return Ok(true),
            Err(position) => position,
        };
        schema_shape(typed, context, declaration)?;
        let parameters = typed
            .data_type_parameters
            .span(declaration.type_parameters)
            .ok_or_else(|| services::unavailable(context, "trait has a stale generic telescope"))?;
        let scope = parameters
            .iter()
            .enumerate()
            .map(|(ordinal, parameter)| {
                Ok((
                    parameter.symbol,
                    ValueType::Rigid(terminal_check_tables::values::RigidType(
                        u32::try_from(ordinal).map_err(|_| {
                            services::unavailable(
                                context,
                                "trait type telescope exceeds its ordinal width",
                            )
                        })?,
                    )),
                ))
            })
            .collect::<Checked<Vec<_>>>()?;
        let signatures = typed
            .trait_machine_signatures
            .span(declaration.machines)
            .ok_or_else(|| services::unavailable(context, "trait has a stale requirement span"))?;
        let mut requirements = Vec::with_capacity(signatures.len());
        let mut symbols = Vec::with_capacity(signatures.len());
        for signature in signatures {
            if !typed.symbols.symbols().nodes().is_valid(signature.symbol)
                || symbols.contains(&signature.symbol)
            {
                return Err(services::unavailable(
                    context,
                    "trait repeats or loses an exact requirement declaration",
                ));
            }
            symbols.push(signature.symbol);
            let formals = typed
                .state_parameters
                .span(signature.parameters)
                .ok_or_else(|| {
                    services::unavailable(
                        context,
                        "schema requirement has a stale formal telescope",
                    )
                })?;
            if formals.iter().any(|formal| {
                !typed.symbols.symbols().nodes().is_valid(formal.symbol)
                    || typed.symbols.get(formal.symbol).parent != signature.symbol
            }) {
                return Err(services::unavailable(
                    context,
                    "schema requirement borrows another declaration's formal scope",
                ));
            }
            validate_references(
                typed,
                context,
                signature,
                parameters
                    .iter()
                    .map(|parameter| parameter.symbol)
                    .collect(),
            )?;
            let lowered = self.static_requirement_signature(
                context,
                signature,
                scope.clone(),
                declaration.is_boundary,
                true,
            )?;
            let abstract_types = lowered
                .parameters
                .iter()
                .filter_map(|value| match value {
                    ValueType::Rigid(rigid) => Some(rigid.0),
                    _ => None,
                })
                .max()
                .map_or(Ok(0), |last| {
                    last.checked_add(1).ok_or_else(|| {
                        services::unavailable(
                            context,
                            "abstract type namespace exceeds its ordinal width",
                        )
                    })
                })?;
            requirements.push(SourceSchemaRequirement {
                declaration: signature.symbol,
                abstract_types,
                signature: lowered,
                envelope: self.static_envelope(context, signature, declaration.is_boundary)?,
            });
        }
        self.static_contracts.schemas.insert(
            position,
            SourceSchema {
                declaration: symbol,
                requirements,
            },
        );
        Ok(true)
    }

    pub(super) fn conformance_declaration(
        &mut self,
        context: MachineId,
        symbol: SymbolHandle,
    ) -> Checked<bool> {
        let typed = self.typed;
        let Some(declaration) = typed.conformance_by_symbol(symbol) else {
            return Ok(false);
        };
        let position = match self
            .static_contracts
            .formed
            .binary_search_by_key(&key(symbol), |symbol| key(*symbol))
        {
            Ok(_) => return Ok(true),
            Err(position) => position,
        };
        if !declaration.type_parameters.is_empty()
            || !declaration.lifetime_parameters.is_empty()
            || !declaration.trait_lifetime_arguments.is_empty()
        {
            return Err(services::unavailable(
                context,
                "static implementation family needs its general binder or region proof",
            ));
        }
        if !self.trait_declaration(context, declaration.trait_symbol)? {
            return Err(services::unavailable(
                context,
                "implementation has no exact schema declaration",
            ));
        }
        let application =
            AppliedTraitScope::from_conformance(typed, declaration).ok_or_else(|| {
                services::unavailable(
                    context,
                    "implementation has no complete exact trait application",
                )
            })?;
        let application_identity = application
            .portable_application_identity(typed)
            .and_then(|identity| typed.type_identity_text(identity))
            .filter(|identity| !identity.is_empty())
            .ok_or_else(|| {
                services::unavailable(
                    context,
                    "implementation trait application has no portable identity",
                )
            })?;
        let requirements = typed.trait_machine_signatures(application.declaration);
        if declaration.rows.len() != requirements.len() {
            return Err(services::unavailable(
                context,
                "implementation rows do not cover the complete schema",
            ));
        }
        let mut entered = Vec::with_capacity(declaration.rows.len());
        let mut questions = Vec::with_capacity(declaration.rows.len());
        for row in &declaration.rows {
            if row.declaring_trait != application.declaration.symbol
                || entered.contains(&row.requirement)
            {
                return Err(services::unavailable(
                    context,
                    "implementation repeats or changes an exact schema requirement",
                ));
            }
            let required = requirements
                .iter()
                .find(|signature| signature.symbol == row.requirement)
                .ok_or_else(|| {
                    services::unavailable(
                        context,
                        "implementation row names no exact schema requirement",
                    )
                })?;
            entered.push(row.requirement);
            let supplied = typed
                .machine_by_symbol(row.realization_machine)
                .ok_or_else(|| {
                    services::unavailable(
                        context,
                        "implementation row has no exact selected declaration",
                    )
                })?;
            let [state] = typed.machine_states(supplied) else {
                return Err(services::unavailable(
                    context,
                    "implementation row has no unique selected entry",
                ));
            };
            if state.symbol != row.realization_state {
                return Err(services::unavailable(
                    context,
                    "implementation row changes the selected entry state",
                ));
            }
            let implementation = if self.select_native_provider(context, supplied.symbol)? {
                SourceImplementation::Native(supplied.symbol)
            } else {
                let machine = self.machine(supplied.symbol)?;
                self.fixed_inputs.push(machine);
                SourceImplementation::Machine(machine)
            };
            let scope = self.applied_scope(context, &application, required)?;
            let signature = self.static_requirement_signature(
                context,
                required,
                scope,
                application.declaration.is_boundary,
                false,
            )?;
            if signature
                .parameters
                .iter()
                .any(|value| matches!(value, ValueType::Rigid(_)))
                || matches!(signature.result, Some(ValueType::Rigid(_)))
            {
                return Err(services::unavailable(
                    context,
                    "conformance requirement retains an unapplied abstract carrier",
                ));
            }
            let provider = if application.declaration.is_boundary
                && matches!(implementation, SourceImplementation::Machine(_))
            {
                let requirement = if application.keeps_declared_signatures(typed) {
                    required.symbol
                } else {
                    let mut closed = typed.closed_requirement_applications().filter(|closed| {
                        closed.template == required.symbol
                            && typed
                                .type_identity_text(closed.portable_trait_application_identity)
                                .as_deref()
                                == Some(application_identity.as_str())
                            && closed.local_arguments.is_empty()
                    });
                    let selected = closed.next().ok_or_else(|| {
                        services::unavailable(
                            context,
                            "boundary implementation lost its prepared exact receiving application",
                        )
                    })?;
                    if closed.next().is_some() {
                        return Err(services::unavailable(
                            context,
                            "boundary implementation has ambiguous receiving applications",
                        ));
                    }
                    selected.signature.symbol
                };
                let (boundary, _) = self.boundary_call(context, requirement)?.ok_or_else(|| {
                    services::unavailable(
                        context,
                        "boundary implementation lost its genuine required header",
                    )
                })?;
                let sources = source::source_identity::toolchain_source_identities(
                    typed.symbols.source_files(),
                )
                .map_err(|_| {
                    services::unavailable(
                        context,
                        "provider has no exact toolchain source identity",
                    )
                })?;
                let mut templates = typed
                    .machine_specializations
                    .iter()
                    .filter(|receipt| receipt.instance == supplied.symbol);
                let template = templates
                    .next()
                    .map_or(supplied.symbol, |receipt| receipt.template);
                if templates.next().is_some() {
                    return Err(services::unavailable(
                        context,
                        "provider body has ambiguous specialization provenance",
                    ));
                }
                let original = typed.machine_by_symbol(template).ok_or_else(|| {
                    services::unavailable(context, "provider body lost its original declaration")
                })?;
                Some(StaticProviderCandidate {
                    boundary,
                    declaration: crate::declare::nominal(typed, &sources, symbol)
                        .map_err(|reason| services::unavailable(context, reason))?,
                    source_requirement: typed.trait_requirement_callable_identity_text(
                        application.declaration,
                        required,
                    ),
                    source_candidate: typed.machine_callable_identity_text(original).ok_or_else(
                        || {
                            services::unavailable(
                                context,
                                "provider body has no original callable identity",
                            )
                        },
                    )?,
                    package: typed.symbols.symbol_package_identity(template),
                })
            } else {
                None
            };
            questions.push(SourceConformance {
                declaration: symbol,
                requirement: row.requirement,
                application: application_identity.clone(),
                implementation,
                signature,
                envelope: self.static_envelope(
                    context,
                    required,
                    application.declaration.is_boundary,
                )?,
                provider,
            });
        }
        self.static_contracts.conformances.extend(questions);
        self.static_contracts.formed.insert(position, symbol);
        Ok(true)
    }

    fn applied_scope(
        &mut self,
        context: MachineId,
        application: &AppliedTraitScope<'_>,
        signature: &StateSignature,
    ) -> Checked<Vec<(SymbolHandle, ValueType)>> {
        let typed = self.typed;
        let outer = std::mem::take(&mut self.scope);
        let view = application.type_reference(typed, signature.return_type);
        let result = (|| {
            for (parameter, _) in &application.substitutions {
                let actual = view.parameter_argument(typed, *parameter).ok_or_else(|| {
                    services::unavailable(
                        context,
                        "trait substitution lost its exact supplying frame",
                    )
                })?;
                let value = self.value_type(context, actual.reference)?;
                if let Some((_, previous)) = self
                    .scope
                    .iter_mut()
                    .find(|(symbol, _)| symbol == parameter)
                {
                    *previous = value;
                } else {
                    self.scope.push((*parameter, value));
                }
            }
            Ok(self.scope.clone())
        })();
        self.scope = outer;
        result
    }
}

fn schema_shape(
    typed: &typed_trees::TypedTrees,
    context: MachineId,
    schema: &TraitDefinition,
) -> Checked<()> {
    if !typed.symbols.symbols().nodes().is_valid(schema.symbol)
        || !schema.lifetime_parameters.is_empty()
        || !schema.conformance_bounds.is_empty()
        || !schema.requires.is_empty()
        || schema.refines.is_some()
        || !schema.refinement_clauses.is_empty()
    {
        return Err(services::unavailable(
            context,
            "trait has unresolved identity or unrepresented laws, inheritance, refinements or regions",
        ));
    }
    let parameters = typed
        .data_type_parameters
        .span(schema.type_parameters)
        .ok_or_else(|| services::unavailable(context, "trait type telescope is stale"))?;
    let mut seen = Vec::with_capacity(parameters.len());
    for parameter in parameters {
        if !typed.symbols.symbols().nodes().is_valid(parameter.symbol)
            || seen.contains(&parameter.symbol)
            || !matches!(parameter.kind, typed_trees::data::TypeParameterKind::Type)
            || parameter.bounds.carry.is_some()
            || parameter.bounds.multiplicity != language_core::Multiplicity::Affine
            || typed.symbols.get(parameter.symbol).parent != schema.symbol
        {
            return Err(services::unavailable(
                context,
                "trait type telescope has an unsupported family, carry bound or duplicate binder",
            ));
        }
        seen.push(parameter.symbol);
    }
    Ok(())
}

struct SignatureReferences<'a> {
    typed: &'a typed_trees::TypedTrees,
    binders: Vec<SymbolHandle>,
    types: Vec<typed_trees::types::TypeReferenceHandle>,
    expressions: Vec<typed_trees::expression::ExpressionHandle>,
    contracts: Vec<typed_trees::callable_values::RuntimeCallableContractHandle>,
    error: Option<&'static str>,
}
fn enter<Node>(visited: &mut Vec<arena::Handle<Node>>, handle: arena::Handle<Node>) -> bool {
    match visited.binary_search_by_key(&(handle.arena_index(), handle.generation()), |handle| {
        (handle.arena_index(), handle.generation())
    }) {
        Ok(_) => false,
        Err(position) => {
            visited.insert(position, handle);
            true
        }
    }
}
impl Extend<SymbolHandle> for SignatureReferences<'_> {
    fn extend<I: IntoIterator<Item = SymbolHandle>>(&mut self, symbols: I) {
        if symbols
            .into_iter()
            .any(|symbol| !self.typed.symbols.symbols().nodes().is_valid(symbol))
        {
            self.error = Some("static signature references a stale declaration");
        }
    }
}
impl typed_trees::references::TypedReferences for SignatureReferences<'_> {
    fn derived_declaration_references(&self) -> bool {
        false
    }
    fn type_reference(&mut self, reference: typed_trees::types::TypeReferenceHandle) -> bool {
        if !self
            .typed
            .type_reference_table
            .contains_type_reference(reference)
        {
            self.error = Some("static signature has a stale type reference");
            return false;
        }
        if let typed_trees::types::TypeReferenceNode::Named { symbol, .. } =
            self.typed.type_reference_table.type_reference(reference)
        {
            if !self.typed.symbols.symbols().nodes().is_valid(*symbol)
                || (self.typed.symbols.get(*symbol).kind == symbols::SymbolKind::TypeParameter
                    && !self.binders.contains(symbol))
            {
                self.error = Some("static signature has an unresolved or out-of-scope type binder");
                return false;
            }
        }
        enter(&mut self.types, reference)
    }
    fn expression(&mut self, expression: typed_trees::expression::ExpressionHandle) -> bool {
        if !self.typed.expression_table.expression_is_valid(expression) {
            self.error = Some("static signature has a stale contract operand");
            return false;
        }
        enter(&mut self.expressions, expression)
    }
    fn callable_contract(
        &mut self,
        contract: typed_trees::callable_values::RuntimeCallableContractHandle,
    ) -> bool {
        if !self.typed.runtime_callable_contracts.is_valid(contract) {
            self.error = Some("static signature has a stale callable contract");
            return false;
        }
        enter(&mut self.contracts, contract)
    }
}
fn validate_references(
    typed: &typed_trees::TypedTrees,
    context: MachineId,
    signature: &StateSignature,
    binders: Vec<SymbolHandle>,
) -> Checked<()> {
    let mut references = SignatureReferences {
        typed,
        binders,
        types: Vec::new(),
        expressions: Vec::new(),
        contracts: Vec::new(),
        error: None,
    };
    typed_trees::references::visit_signature(typed, &mut references, signature);
    references
        .error
        .map_or(Ok(()), |reason| Err(services::unavailable(context, reason)))
}
