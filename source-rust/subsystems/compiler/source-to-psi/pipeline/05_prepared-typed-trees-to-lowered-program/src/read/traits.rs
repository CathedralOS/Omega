//! Form static schemas in their own binder scope and retain each exact
//! required/supplied question. A named map grants no evidence by itself.

use crate::source_state::signature::StateSignature;
use crate::source_state::trait_definition::{
    AppliedTraitScope, Conformance, ConformanceRow, TraitDefinition,
};
use semantic_vocabulary::MachineId;
use symbols::SymbolHandle;
use terminal_check_tables::Checked;

use super::{Program, services};
use crate::body::ValueType;
use crate::callables::static_contracts::{
    SourceConformance, SourceImplementation, SourceSchema, SourceSchemaRequirement,
    SourceServiceCeiling, StaticProviderCandidate, StaticSourceEnvelope,
};

fn key(symbol: SymbolHandle) -> (u32, u32) {
    (symbol.arena_index(), symbol.generation())
}

/// The same requirement arguments as lowered values and as written source
/// types. A record application needs the latter to retain its exact identity.
#[derive(Clone)]
pub(super) struct RequirementTypeScope {
    pub(super) values: Vec<(SymbolHandle, ValueType)>,
    pub(super) source: std::sync::Arc<crate::source_state::SourceTypeScope>,
}

/// Pair every selected row with the declaration and arguments that own its public contract.
struct ImplementationMember<'program, 'row> {
    receiver: AppliedTraitScope<'program>,
    requirement: &'program StateSignature,
    row: &'row ConformanceRow,
}

fn implementation_members<'program, 'row>(
    typed: &'program crate::source_state::TypedTrees,
    context: MachineId,
    declaration: &'row Conformance,
    application: AppliedTraitScope<'program>,
) -> Checked<Vec<ImplementationMember<'program, 'row>>> {
    let receivers =
        crate::source_state::service_declarations::projection::inherited_scopes(typed, application)
            .map_err(|diagnostic| crate::read::unavailable(context, diagnostic.message))?;
    let mut requirements = std::collections::HashMap::new();
    for receiver in &receivers {
        for signature in typed.trait_machine_signatures(receiver.declaration) {
            if requirements
                .insert(signature.symbol, (receiver, signature))
                .is_some()
            {
                return Err(crate::read::unavailable(
                    context,
                    "two declarations claim the same implementation requirement",
                ));
            }
        }
    }
    if declaration.rows.len() != requirements.len() {
        return Err(crate::read::unavailable(
            context,
            "implementation rows do not cover the complete schema",
        ));
    }
    declaration
        .rows
        .iter()
        .map(|row| {
            let (receiver, requirement) = requirements
                .remove(&row.requirement)
                .filter(|(owner, _)| owner.declaration.symbol == row.declaring_trait)
                .ok_or_else(|| {
                    crate::read::unavailable(
                        context,
                        "implementation row repeats or changes an exact declaring requirement",
                    )
                })?;
            if row.source
                == crate::source_state::trait_definition::ConformanceRowSource::TraitDefault
                && !requirement.is_default
            {
                return Err(crate::read::unavailable(
                    context,
                    "selected requirement has no default body",
                ));
            }
            Ok(ImplementationMember {
                receiver: receiver.clone(),
                requirement,
                row,
            })
        })
        .collect()
}

impl<'program> Program<'program> {
    /// A requirement's own type and lifetime parameters have meaning inside
    /// its signature. So does an array extent that depends on its own const
    /// parameter. Check those requirements separately with their binders in scope.
    pub(super) fn checked_as_template(&self, signature: &StateSignature) -> bool {
        use crate::source_state::data::TypeParameterKind;
        use crate::source_state::types::{FixedArrayLength, TypeReferenceNode};

        let typed = self.typed;
        let parameters = typed.state_signature_type_parameters(signature);
        if !signature.lifetime_parameters.is_empty()
            || parameters
                .iter()
                .any(|parameter| parameter.kind == TypeParameterKind::Type)
        {
            return true;
        }
        let binders = parameters
            .iter()
            .filter(|parameter| {
                matches!(
                    parameter.kind,
                    TypeParameterKind::Const { .. } | TypeParameterKind::Value { .. }
                )
            })
            .map(|parameter| parameter.symbol)
            .collect::<Vec<_>>();
        if binders.is_empty() {
            return false;
        }
        let types = &typed.type_reference_table;
        let mut pending = typed
            .state_parameters
            .span_or_empty(signature.parameters)
            .iter()
            .map(|formal| formal.type_reference)
            .chain(std::iter::once(signature.return_type))
            .collect::<Vec<_>>();
        // Type references form a tree; the bound only guards a malformed table.
        let mut budget = types.type_reference_count();
        while let Some(reference) = pending.pop() {
            if budget == 0 {
                return false;
            }
            budget -= 1;
            if !types.contains_type_reference(reference) {
                continue;
            }
            match types.type_reference(reference) {
                TypeReferenceNode::FixedArray {
                    element_type,
                    length,
                } => {
                    if let FixedArrayLength::ConstParameter { symbol, .. } = length
                        && binders.contains(symbol)
                    {
                        return true;
                    }
                    pending.push(*element_type);
                }
                TypeReferenceNode::Slice { element_type } => pending.push(*element_type),
                TypeReferenceNode::Reference { referee, .. } => pending.push(*referee),
                TypeReferenceNode::Constrained { base_type, .. } => pending.push(*base_type),
                _ => {}
            }
        }
        false
    }

    /// An applied data type with a rigid argument needs a storage row for
    /// that argument. Only the trait's own check form owns its binder.
    fn schema_needs_parameter_storage(
        &self,
        reference: crate::source_state::types::TypeReferenceHandle,
        binders: &[SymbolHandle],
    ) -> bool {
        use crate::source_state::types::TypeReferenceNode;
        let typed = self.typed;
        let mut current = reference;
        for _ in 0..typed.type_reference_table.type_reference_count() {
            if !typed.contains_type_reference(current) {
                return false;
            }
            match typed.type_reference_table.type_reference(current) {
                TypeReferenceNode::Generic { .. } => {
                    return typed.type_reference_mentions_any(current, binders);
                }
                TypeReferenceNode::Reference { referee, .. } => current = *referee,
                TypeReferenceNode::Constrained { base_type, .. } => current = *base_type,
                TypeReferenceNode::FixedArray { element_type, .. }
                | TypeReferenceNode::Slice { element_type } => current = *element_type,
                _ => return false,
            }
        }
        false
    }

    pub(super) fn static_envelope(
        &self,
        context: MachineId,
        signature: &StateSignature,
        boundary: bool,
    ) -> Checked<StaticSourceEnvelope> {
        let written =
            crate::publication::reach::has_authored_ceiling(self.typed, context, signature.symbol)?;
        let service_reach = if written {
            if signature.service_reach_is_installation_bound && !boundary {
                return Err(crate::read::unavailable(
                    context,
                    "static service ceiling is bound to an unresolved installation",
                ));
            }
            // A boundary schema checks the finite upper bound here. Its
            // boundary declaration retains the installation-bound row until
            // selection substitutes the exact provider reach.
            SourceServiceCeiling::Closed(services::finite_reach(
                self.typed,
                context,
                signature.service_reach_row,
            )?)
        } else if boundary {
            return Err(crate::read::unavailable(
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
                crate::read::unavailable(context, "static invocation ceiling has a stale span")
            })?;
        let parameters = self
            .typed
            .state_parameters
            .span(signature.parameters)
            .ok_or_else(|| {
                crate::read::unavailable(context, "static signature has a stale formal span")
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
        if declaration.is_boundary {
            return Ok(false);
        }
        self.read_trait_schema(context, declaration)
    }

    pub(super) fn boundary_trait_declaration(
        &mut self,
        context: MachineId,
        declaration: &TraitDefinition,
    ) -> Checked<()> {
        if !declaration.is_boundary {
            return Err(crate::read::unavailable(
                context,
                "boundary schema names an ordinary trait",
            ));
        }
        self.read_trait_schema(context, declaration).map(|_| ())
    }

    /// A generic schema is checked as authored, but has no bare runtime
    /// service identity. Its closed requirements publish the applied services.
    pub(super) fn claim_boundary_trait(
        &mut self,
        context: MachineId,
        declaration: &'program TraitDefinition,
    ) -> Checked<()> {
        self.boundary_trait_declaration(context, declaration)?;
        if declaration.type_parameters.is_empty() && declaration.lifetime_parameters.is_empty() {
            self.declared_service(context, declaration)?;
        }
        Ok(())
    }

    pub(super) fn read_trait_schema(
        &mut self,
        context: MachineId,
        declaration: &TraitDefinition,
    ) -> Checked<bool> {
        let typed = self.typed;
        let symbol = declaration.symbol;
        self.admitted_declarations.include(symbol);
        let position = match self
            .static_contracts
            .schemas
            .binary_search_by_key(&key(symbol), |schema| key(schema.declaration))
        {
            Ok(_) => return Ok(true),
            Err(position) => position,
        };
        if declaration.is_boundary {
            schema_local_shape(typed, context, declaration)?;
            if !declaration.requires.is_empty() {
                // A schema owns an open telescope. Closed calls independently
                // project the same parent edges under their actual arguments.
                let scope = AppliedTraitScope::abstract_scope(declaration);
                if declaration.type_parameters.is_empty()
                    && declaration.lifetime_parameters.is_empty()
                {
                    self.require_boundary_parent_contract(context, &scope)?;
                } else {
                    Self::check_boundary_parents(typed, context, &scope)?;
                }
            }
        } else {
            schema_shape(typed, context, declaration)?;
        }
        let parameters = typed
            .data_type_parameters
            .span(declaration.type_parameters)
            .ok_or_else(|| {
                crate::read::unavailable(context, "trait has a stale generic telescope")
            })?;
        let values = parameters
            .iter()
            .enumerate()
            // An implementation binder is its declared record value, not a type.
            .filter(|(_, parameter)| {
                parameter.kind == crate::source_state::data::TypeParameterKind::Type
            })
            .map(|(ordinal, parameter)| {
                Ok((
                    parameter.symbol,
                    ValueType::Rigid(terminal_check_tables::values::RigidType(
                        u32::try_from(ordinal).map_err(|_| {
                            crate::read::unavailable(
                                context,
                                "trait type telescope exceeds its ordinal width",
                            )
                        })?,
                    )),
                ))
            })
            .collect::<Checked<Vec<_>>>()?;
        let owns_binders = values.iter().all(|(symbol, value)| {
            matches!(value, ValueType::Rigid(rigid)
                if self.generic_binders.get(rigid.0 as usize) == Some(symbol))
        });
        let rigid_parameters = values.iter().map(|(binder, _)| *binder).collect();
        let scope = RequirementTypeScope {
            values,
            source: std::sync::Arc::new(crate::source_state::SourceTypeScope {
                rigid_parameters,
                ..Default::default()
            }),
        };
        let signatures = typed
            .trait_machine_signatures
            .span(declaration.machines)
            .ok_or_else(|| {
                crate::read::unavailable(context, "trait has a stale requirement span")
            })?;
        let mut requirements = Vec::with_capacity(signatures.len());
        let mut symbols = Vec::with_capacity(signatures.len());
        for signature in signatures {
            if !typed.symbols.symbols().nodes().is_valid(signature.symbol)
                || symbols.contains(&signature.symbol)
            {
                return Err(crate::read::unavailable(
                    context,
                    "trait repeats or loses an exact requirement declaration",
                ));
            }
            symbols.push(signature.symbol);
            let formals = typed
                .state_parameters
                .span(signature.parameters)
                .ok_or_else(|| {
                    crate::read::unavailable(
                        context,
                        "schema requirement has a stale formal telescope",
                    )
                })?;
            if formals.iter().any(|formal| {
                !typed.symbols.symbols().nodes().is_valid(formal.symbol)
                    || typed.symbols.get(formal.symbol).parent != signature.symbol
            }) {
                return Err(crate::read::unavailable(
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
                    .chain(typed.state_signature_type_parameters(signature))
                    .map(|parameter| parameter.symbol)
                    .collect(),
            )?;
            // generics.md, "Publication and assumptions": a requirement with
            // binders of its own is a template. Its open signature is checked
            // once in its own check form, never in this module, whose uses are
            // requirement instances at their actual arguments and regions.
            let template = if self.checked_as_template(signature) {
                Some(signature.symbol)
            } else if !owns_binders
                && formals
                    .iter()
                    .map(|formal| formal.type_reference)
                    .chain([signature.return_type])
                    .any(|reference| {
                        self.schema_needs_parameter_storage(
                            reference,
                            &scope.source.rigid_parameters,
                        )
                    })
            {
                Some(declaration.symbol)
            } else {
                None
            };
            if let Some(template) = template {
                if !self.generic_templates.contains(&template) {
                    self.generic_templates.push(template);
                }
                requirements.push(SourceSchemaRequirement {
                    instance: None,
                    declaration: signature.symbol,
                    abstract_types: 0,
                    signature: None,
                    envelope: self.static_envelope(context, signature, declaration.is_boundary)?,
                });
                continue;
            }
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
                .chain(lowered.result.iter())
                .filter_map(|value| match value {
                    ValueType::Rigid(rigid) => Some(rigid.0),
                    _ => None,
                })
                .max()
                .map_or(Ok(0), |last| {
                    last.checked_add(1).ok_or_else(|| {
                        crate::read::unavailable(
                            context,
                            "abstract type namespace exceeds its ordinal width",
                        )
                    })
                })?;
            let envelope = self.static_envelope(context, signature, declaration.is_boundary)?;
            // Abstract schemas retain their logical carriers. A closed
            // declaration also publishes its ordinary Terminal contract,
            // without manufacturing a body or an executable entry.
            if declaration.type_parameters.is_empty()
                && declaration.lifetime_parameters.is_empty()
                && signature.type_parameters.is_empty()
                && abstract_types == 0
                && matches!(&envelope.service_reach, SourceServiceCeiling::Closed(_))
            {
                let complete = self.signature_envelope(context, signature)?;
                let id = self.declaration_signatures.push(lowered.clone());
                if self.declaration_sources.push(signature) != id
                    || self.declaration_selections.push(None) != id
                    || self.declaration_envelopes.push(complete) != id
                {
                    return Err(crate::read::unavailable(
                        context,
                        "requirement lost its declaration contract coordinate",
                    ));
                }
            } else if !lowered.content_ensures.is_empty() {
                return Err(crate::read::unavailable(
                    context,
                    "an abstract content guarantee needs its applied declaration contract",
                ));
            }
            requirements.push(SourceSchemaRequirement {
                instance: None,
                declaration: signature.symbol,
                abstract_types,
                signature: Some(lowered),
                envelope,
            });
        }
        self.static_contracts.schemas.insert(
            position,
            SourceSchema {
                declaration: symbol,
                requirements,
                bindings: Vec::new(),
            },
        );
        let parents = AppliedTraitScope::abstract_scope(declaration)
            .parent_scopes(typed)
            .map_err(|reason| crate::read::unavailable(context, reason))?;
        for parent in parents {
            self.read_trait_schema(context, parent.declaration)?;
        }
        Ok(true)
    }

    pub(super) fn require_boundary_parent_contract(
        &mut self,
        context: MachineId,
        scope: &AppliedTraitScope<'_>,
    ) -> Checked<()> {
        if Self::check_boundary_parents(self.typed, context, scope)? {
            let environment = scope.environment();
            if !self.boundary_parent_contracts.contains(&environment) {
                self.boundary_parent_contracts.push(environment);
            }
        }
        Ok(())
    }

    /// A schema can check its inheritance with open parameters. Calling
    /// policies are issued for the receiving application when it is supplied.
    fn check_boundary_parents(
        typed: &crate::source_state::TypedTrees,
        context: MachineId,
        scope: &AppliedTraitScope<'_>,
    ) -> Checked<bool> {
        if !scope.declaration.is_boundary {
            return Err(crate::read::unavailable(
                context,
                "boundary parent contract has no boundary owner",
            ));
        }
        // Boundary parents keep their callable declarations in their own
        // schemas. Validate each actual application here; the calling-policy
        // owner projects the interface and checks completed ordinary parents.
        let scopes = crate::source_state::service_declarations::projection::inherited_scopes(
            typed,
            scope.clone(),
        )
        .map_err(|diagnostic| crate::read::unavailable(context, diagnostic.message))?;
        let mut needs_contract_owner = false;
        for view in &scopes {
            if !view.declaration.is_boundary {
                needs_contract_owner = true;
                if view
                    .parent_scopes(typed)
                    .map_err(|reason| crate::read::unavailable(context, reason))?
                    .iter()
                    .any(|parent| parent.declaration.is_boundary)
                {
                    return Err(crate::read::unavailable(
                        context,
                        "ordinary contract trait inherits a boundary service",
                    ));
                }
            } else if view.declaration.symbol != scope.declaration.symbol {
                // Each parent keeps its own schema. Authenticate its supplied
                // types in that parent's application before the existing
                // service and Calling owners retain the inherited questions.
                let signatures = typed.trait_machine_signatures(view.declaration);
                if !signatures.is_empty() {
                    // The complete inherited callable surface still owes the
                    // exact selected contract owner before publication.
                    needs_contract_owner = true;
                }
                for signature in signatures {
                    typed
                        .requirement_definition_types(view, signature)
                        .ok_or_else(|| crate::read::unavailable(
                            context,
                            "inherited boundary call lost its complete declared parameter and result types",
                        ))?;
                }
            }
        }
        Ok(needs_contract_owner)
    }

    pub(super) fn prepare_inherited_boundary_contracts(
        &mut self,
        context: MachineId,
        root: &AppliedTraitScope<'_>,
        selected_calling: Option<SymbolHandle>,
        calling_vocabulary: &[(&TraitDefinition, &TraitDefinition)],
    ) -> Checked<()> {
        use crate::source_state::data::TypeParameterKind;
        let typed = self.typed;
        let unavailable = |reason| crate::read::unavailable(context, reason);
        let scopes = crate::source_state::service_declarations::projection::inherited_scopes(
            typed,
            root.clone(),
        )
        .map_err(|diagnostic| crate::read::unavailable(context, diagnostic.message))?;
        for parent in scopes.iter().skip(1) {
            let declaration = parent.declaration;
            if declaration.is_boundary {
                // Calling checks its selected headers separately. In a plain
                // service, the declaring boundary owns each inherited contract.
                if selected_calling.is_none() && !declaration.machines.is_empty() {
                    self.boundary_trait_declaration(context, declaration)?;
                    let schema = self
                        .static_contracts
                        .schemas
                        .iter()
                        .find(|schema| schema.declaration == declaration.symbol)
                        .ok_or_else(|| {
                            unavailable("inherited service lost its admitted declaration")
                        })?;
                    for requirement in typed.trait_machine_signatures(declaration) {
                        let header = typed
                            .applied_callable_signature(parent, requirement.symbol)
                            .or_else(|| {
                                parent
                                    .keeps_declared_signatures(typed)
                                    .then_some(requirement)
                            })
                            .ok_or_else(|| {
                                unavailable(
                                    "inherited boundary call has no complete applied header",
                                )
                            })?;
                        if !schema
                            .requirements
                            .iter()
                            .any(|row| row.declaration == header.symbol)
                        {
                            return Err(unavailable(
                                "inherited call is absent from the declaring service's contract",
                            ));
                        }
                    }
                }
                continue;
            }
            if !declaration.machines.is_empty() {
                return Err(unavailable(
                    "inherited boundary calls need an admitted contract owner",
                ));
            }
            if Some(declaration.symbol) == selected_calling {
                continue;
            }
            if calling_vocabulary
                .iter()
                .any(|(calling, _)| calling.symbol == declaration.symbol)
            {
                return Err(unavailable("inherited Calling has no prepared policy"));
            }
            self.read_trait_schema(context, declaration)?;
            // The service schema retains this exact parent application. Its
            // selected implementations still owe their own conformance proofs.
            for parameter in typed.trait_type_parameters(declaration) {
                if parameter.kind == TypeParameterKind::Implementation {
                    let selected = parent
                        .named_implementation_argument(typed, parameter.symbol)
                        .map_err(unavailable)?;
                    if !self.conformance_declaration(context, selected.symbol)? {
                        return Err(unavailable(
                            "boundary parent argument lost its implementation declaration",
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    /// Whether a subject implementation is generic: it declares its own
    /// parameters, or one of its rows selects a machine that is checked as a
    /// generic template (`is_generic_template`). Such an implementation has no
    /// closed static conformance question to form.
    pub(super) fn is_generic_conformance(
        &self,
        declaration: &crate::source_state::trait_definition::Conformance,
    ) -> bool {
        let typed = self.typed;
        !declaration.type_parameters.is_empty()
            || !declaration.lifetime_parameters.is_empty()
            || !declaration.trait_lifetime_arguments.is_empty()
            || declaration.rows.iter().any(|row| {
                typed
                    .machine_by_symbol(row.realization_machine)
                    .is_some_and(|machine| self.is_generic_template(machine))
            })
    }

    /// A generic subject implementation is checked the way a generic machine
    /// is: once, as written. Each row owes one refinement witness, whose
    /// implementation is the row's machine read as its own generic template
    /// (`subject_declarations` sends that machine to `generic_templates`).
    /// The witness checks that the machine refines the requirement for every
    /// application (generics.md: "Bodies are checked against their declared
    /// requirements; a concrete selection cannot silently strengthen the
    /// generic API").
    ///
    /// A type parameter stays abstract and a supported const parameter keeps
    /// its declared carrier, just as it does in an ordinary generic machine.
    pub(super) fn generic_conformance_witnesses(
        &mut self,
        context: MachineId,
        declaration: &'_ crate::source_state::trait_definition::Conformance,
    ) -> Checked<()> {
        let typed = self.typed;
        let unserved = |what| crate::read::unavailable(context, what);
        let parameters = typed.conformance_type_parameters(declaration);
        let plain = declaration.lifetime_parameters.is_empty()
            && declaration.trait_lifetime_arguments.is_empty()
            && parameters.len() == declaration.type_parameters.len()
            && parameters.iter().all(|parameter| {
                super::type_parameters::can_check_implementation_parameter(typed, parameter)
            });
        let Some(application) = plain
            .then(|| AppliedTraitScope::from_conformance(typed, declaration))
            .flatten()
            .filter(|application| {
                super::subjects::served_trait_header(typed, application.declaration).is_ok()
                    && typed
                        .trait_type_parameters(application.declaration)
                        .iter()
                        .all(|parameter| {
                            super::type_parameters::can_check_implementation_parameter(
                                typed, parameter,
                            )
                        })
            })
        else {
            return Err(unserved(
                "static implementation family needs its general binder or region proof",
            ));
        };
        for member in implementation_members(typed, context, declaration, application)? {
            let row = member.row;
            let implemented = member.receiver.declaration;
            if super::subjects::served_trait_header(typed, implemented).is_err() {
                return Err(unserved(
                    "generic implementation needs its exact declaring trait header",
                ));
            }
            if row.source == crate::source_state::trait_definition::ConformanceRowSource::Reference
            {
                if !parameters.is_empty() || !implemented.type_parameters.is_empty() {
                    return Err(unserved("generic reference-row refinement"));
                }
                let realization = typed
                    .machine_by_symbol(row.realization_machine)
                    .ok_or_else(|| {
                        unserved("implementation row has no exact selected declaration")
                    })?;
                // A row may select an established conformer's entry only for
                // the requirement it already implements.
                if let Some((owner, supplied)) = self.conformer_requirement(context, realization)?
                    && (owner != row.declaring_trait || supplied.symbol != row.requirement)
                {
                    return Err(unserved("selection row cannot rebind a declared conformer"));
                }
            }
            if !self.generic_templates.contains(&row.realization_machine)
                && typed
                    .machine_by_symbol(row.realization_machine)
                    .is_some_and(|machine| self.is_generic_template(machine))
            {
                self.generic_templates.push(row.realization_machine);
            }
            let witness = crate::ConformanceWitness {
                declaring_trait: row.declaring_trait,
                requirement: row.requirement,
                implementation: row.realization_machine,
                value_arguments: Vec::new(),
            };
            if !self.conformance_witnesses.contains(&witness) {
                self.conformance_witnesses.push(witness);
            }
        }
        Ok(())
    }

    pub(super) fn conformance_declaration(
        &mut self,
        context: MachineId,
        symbol: SymbolHandle,
    ) -> Checked<bool> {
        self.applied_conformance_declaration(context, symbol, None)
    }

    pub(super) fn applied_conformance_declaration(
        &mut self,
        context: MachineId,
        symbol: SymbolHandle,
        selected: Option<(
            &'program crate::source_state::ClosedConformanceApplication,
            &str,
        )>,
    ) -> Checked<bool> {
        let typed = self.typed;
        let Some(declaration) = typed.conformance_by_symbol(symbol) else {
            return Ok(false);
        };
        self.admitted_declarations.include(symbol);
        let selected_identity = selected.map(|(_, identity)| identity);
        let position = match self
            .static_contracts
            .conformance_position(symbol, selected_identity)
        {
            Ok(_) => return Ok(true),
            Err(position) => position,
        };
        if (!declaration.type_parameters.is_empty() && selected.is_none())
            || !declaration.lifetime_parameters.is_empty()
            || !declaration.trait_lifetime_arguments.is_empty()
        {
            return Err(crate::read::unavailable(
                context,
                "static implementation family needs its general binder or region proof",
            ));
        }
        let schema = typed
            .trait_definition_by_symbol(declaration.trait_symbol)
            .ok_or_else(|| {
                crate::read::unavailable(context, "implementation has no exact schema declaration")
            })?;
        if typed.core_vocabulary(schema.symbol)
            == Some(language_core::core_vocabulary::CoreVocabulary::AccessCustody)
        {
            return Err(crate::read::unavailable(
                context,
                "AccessCustody conformance agreement is not checked yet",
            ));
        }
        if typed.core_vocabulary(schema.symbol)
            == Some(language_core::core_vocabulary::CoreVocabulary::PrivateCallbackSlot)
        {
            // The slot has no callable rows. Its contract is the closed
            // relation between the semantic type, layout policy and callback.
            // A declaration must satisfy it even when no policy uses the slot.
            let selected = crate::source_state::expression::StaticMachineArgument {
                symbol,
                ..Default::default()
            };
            let sources = self
                .exact_toolchain_sources()
                .map_err(|reason| crate::read::unavailable(context, reason))?;
            typed
                .check_private_callback_slot_declaration(&selected, symbol, sources)
                .map_err(|reason| crate::read::unavailable(context, reason))?;
            if !schema.machines.is_empty()
                || !schema.requires.is_empty()
                || schema.refines.is_some()
                || !schema.refinement_clauses.is_empty()
            {
                return Err(crate::read::unavailable(
                    context,
                    "private callback slot has additional requirements",
                ));
            }
            let start = self.static_contracts.conformances.len();
            self.static_contracts.formed.insert(
                position,
                crate::callables::static_contracts::FormedConformance {
                    declaration: symbol,
                    application: selected_identity.map(str::to_owned),
                    questions: start..start,
                },
            );
            return Ok(true);
        }
        let has_proposition_parameter =
            typed.trait_type_parameters(schema).iter().any(|parameter| {
                matches!(
                    parameter.kind,
                    crate::source_state::data::TypeParameterKind::Proposition { .. }
                )
            });
        if !schema.is_boundary && has_proposition_parameter {
            schema_shape(typed, context, schema)?;
        } else {
            self.read_trait_schema(context, schema)?;
        }
        let application = match selected {
            Some((record, _)) => record.trait_scope.scope(typed),
            None => AppliedTraitScope::from_conformance(typed, declaration),
        }
        .ok_or_else(|| {
            crate::read::unavailable(
                context,
                "implementation has no complete exact trait application",
            )
        })?;
        let capture = selected
            .map(|(record, _)| {
                let mut groups =
                    typed
                        .captured_machine_applications
                        .iter()
                        .filter(|(_, capture)| {
                            capture.binding_application() == record.callable_application
                                && capture.conformance_application == *record
                        });
                match (groups.next(), groups.next()) {
                    (Some((_, group)), None) => Ok(group),
                    _ => Err(crate::read::unavailable(
                        context,
                        "selected provider has no unique set of prepared member bodies",
                    )),
                }
            })
            .transpose()?;
        // Check each generic body against the public contract even when no
        // call in this compilation needs a concrete implementation.
        if schema.is_boundary && self.is_generic_conformance(declaration) {
            self.generic_conformance_witnesses(context, declaration)?;
        }
        let mut questions = Vec::with_capacity(declaration.rows.len());
        for member in implementation_members(typed, context, declaration, application)? {
            let application = &member.receiver;
            let required = member.requirement;
            let row = member.row;
            let application_identity = application
                .portable_application_identity(typed)
                .and_then(|identity| typed.type_identity_text(identity))
                .filter(|identity| !identity.is_empty())
                .ok_or_else(|| {
                    crate::read::unavailable(
                        context,
                        "implementation requirement has no portable receiver identity",
                    )
                })?;
            let supplied = typed
                .machine_by_symbol(row.realization_machine)
                .ok_or_else(|| {
                    crate::read::unavailable(
                        context,
                        "implementation row has no exact selected declaration",
                    )
                })?;
            if row.source == crate::source_state::trait_definition::ConformanceRowSource::Reference
                && let Some((owner, required)) = self.conformer_requirement(context, supplied)?
            {
                let same_requirement =
                    owner == row.declaring_trait && required.symbol == row.requirement;
                let same_application = same_requirement
                    && (application.declaration.type_parameters.is_empty()
                        || typed
                            .machine_conformance_requirement_scope(
                                supplied.symbol,
                                owner,
                                required.symbol,
                            )
                            .map_err(|reason| crate::read::unavailable(context, reason))?
                            .is_some_and(|own| own.same_application(typed, application)));
                if !same_application {
                    return Err(crate::read::unavailable(
                        context,
                        "selection row cannot rebind a declared conformer",
                    ));
                }
            }
            let Some([state]) = typed.machine_states(supplied).as_array() else {
                return Err(crate::read::unavailable(
                    context,
                    "implementation row has no unique selected entry",
                ));
            };
            if state.symbol != row.realization_state {
                return Err(crate::read::unavailable(
                    context,
                    "implementation row changes the selected entry state",
                ));
            }
            if row.source.owns_realization() {
                let (owner, owned) = typed
                    .owned_conformance_realization(supplied.symbol)
                    .ok_or_else(|| {
                        crate::read::unavailable(
                            context,
                            "implementation body lost its exact owning conformance row",
                        )
                    })?;
                if owner.symbol != declaration.symbol || owned != row {
                    return Err(crate::read::unavailable(
                        context,
                        "implementation body changes its exact owning conformance row",
                    ));
                }
            }
            let original = supplied;
            let supplied = if let Some(capture) = capture.filter(|_| row.source.owns_realization())
            {
                capture
                    .declaration(original.symbol)
                    .map_err(|reason| crate::read::unavailable(context, reason))?
                    .filter(|body| {
                        typed
                            .captured_conformance_owner(capture, body)
                            .is_some_and(|owner| owner.symbol == symbol)
                    })
                    .and_then(|body| typed.machine_by_symbol(body.instance))
                    .ok_or_else(|| {
                        crate::read::unavailable(
                            context,
                            "selected provider lost its checked member body",
                        )
                    })?
            } else {
                supplied
            };
            let members =
                self.conformance_method_bodies(context, application, required, supplied)?;
            for (applied_requirement, supplied) in members {
                let implementation = if self.select_native_provider(context, supplied.symbol)? {
                    SourceImplementation::Native(supplied.symbol)
                } else {
                    let machine = self.subject_machine(supplied.symbol)?;
                    self.fixed_contracts
                        .push(super::FixedContract::Interface(machine));
                    SourceImplementation::Machine(machine)
                };
                let scope = self.applied_scope(context, application)?;
                let signature = self.static_requirement_signature(
                    context,
                    applied_requirement,
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
                    return Err(crate::read::unavailable(
                        context,
                        "conformance requirement retains an unapplied abstract carrier",
                    ));
                }
                let provider = if application.declaration.is_boundary
                    && matches!(implementation, SourceImplementation::Machine(_))
                {
                    let requirement = if typed
                        .closed_requirement_application(applied_requirement.symbol)
                        .is_some()
                        || application.keeps_declared_signatures(typed)
                    {
                        applied_requirement.symbol
                    } else {
                        let mut closed = typed.closed_requirement_applications().filter(|closed| {
                            closed.template == applied_requirement.symbol
                                && typed.requirement_application_matches_contract_scope(
                                    closed.signature.symbol,
                                    application,
                                )
                                && closed.local_arguments.is_empty()
                        });
                        let selected = closed.next().ok_or_else(|| {
                            crate::read::unavailable(
                                context,
                                "boundary implementation lost its prepared exact receiving application",
                            )
                        })?;
                        if closed.next().is_some() {
                            return Err(crate::read::unavailable(
                                context,
                                "boundary implementation has ambiguous receiving applications",
                            ));
                        }
                        selected.signature.symbol
                    };
                    let (boundary, _) = self
                        .read_boundary_declaration(context, requirement)?
                        .ok_or_else(|| {
                            crate::read::unavailable(
                                context,
                                "boundary implementation lost its genuine required header",
                            )
                        })?;
                    let mut templates = typed
                        .machine_specializations
                        .iter()
                        .filter(|receipt| receipt.instance == supplied.symbol);
                    let template = templates
                        .next()
                        .map_or(original.symbol, |receipt| receipt.template);
                    if templates.next().is_some() {
                        return Err(crate::read::unavailable(
                            context,
                            "provider body has ambiguous specialization provenance",
                        ));
                    }
                    let original = typed.machine_by_symbol(template).ok_or_else(|| {
                        crate::read::unavailable(
                            context,
                            "provider body lost its original declaration",
                        )
                    })?;
                    Some(StaticProviderCandidate {
                        boundary,
                        declaration: symbol,
                        provider_identity: match selected_identity {
                            Some(identity) => identity.to_owned(),
                            None => typed
                                .normalized_hermetic_symbol_identity(
                                    typed.implementation_value_head(symbol),
                                )
                                .map_err(|reason| crate::read::unavailable(context, reason))?,
                        },
                        // The selection names the public method in the service schema.
                        // Its boundary above identifies this particular checked call.
                        source_requirement: typed.trait_requirement_callable_identity_text(
                            application.declaration,
                            required,
                        ),
                        source_candidate: typed
                            .machine_callable_identity_text(original)
                            .ok_or_else(|| {
                                crate::read::unavailable(
                                    context,
                                    "provider body has no original callable identity",
                                )
                            })?,
                        package: typed.symbols.symbol_package_identity(template),
                    })
                } else {
                    None
                };
                questions.push(SourceConformance {
                    declaration: symbol,
                    requirement: applied_requirement.symbol,
                    application: application_identity.clone(),
                    implementation,
                    signature,
                    envelope: self.static_envelope(
                        context,
                        applied_requirement,
                        application.declaration.is_boundary,
                    )?,
                    provider,
                });
            }
        }
        let start = self.static_contracts.conformances.len();
        self.static_contracts.conformances.extend(questions);
        self.static_contracts.formed.insert(
            position,
            crate::callables::static_contracts::FormedConformance {
                declaration: symbol,
                application: selected_identity.map(str::to_owned),
                questions: start..self.static_contracts.conformances.len(),
            },
        );
        Ok(true)
    }

    pub(super) fn applied_scope(
        &mut self,
        context: MachineId,
        application: &AppliedTraitScope<'_>,
    ) -> Checked<RequirementTypeScope> {
        let typed = self.typed;
        let outer = std::mem::take(&mut self.scope);
        // A non-generic conformance supplies its receiver arguments together,
        // independently of which parameters its chosen method uses.
        let read = (|| -> Checked<_> {
            let mut substitutions = Vec::new();
            let mut values = Vec::new();
            for binder in typed.trait_type_parameters(application.declaration) {
                match binder.kind {
                    crate::source_state::data::TypeParameterKind::Type => {
                        let actual = application.parameter_argument(typed, binder.symbol);
                        substitutions.push((binder.symbol, actual));
                        let value = self.value_type(context, actual)?;
                        values.push((binder.symbol, value));
                    }
                    crate::source_state::data::TypeParameterKind::Proposition { .. } => {
                        let actual = application.parameter_argument(typed, binder.symbol);
                        substitutions.push((binder.symbol, actual));
                    }
                    _ => {}
                }
            }
            Ok((values, substitutions))
        })();
        self.scope = outer;
        let (values, substitutions) = read?;
        Ok(RequirementTypeScope {
            values,
            source: self.extended_source_scope(substitutions),
        })
    }
}

fn schema_shape(
    typed: &crate::source_state::TypedTrees,
    context: MachineId,
    schema: &TraitDefinition,
) -> Checked<()> {
    schema_local_shape(typed, context, schema)
}

fn schema_local_shape(
    typed: &crate::source_state::TypedTrees,
    context: MachineId,
    schema: &TraitDefinition,
) -> Checked<()> {
    super::subjects::served_trait_header(typed, schema)
        .map_err(|reason| crate::read::unavailable(context, reason))?;
    if !typed.symbols.symbols().nodes().is_valid(schema.symbol)
        || !schema.lifetime_parameters.is_empty()
        || schema.refines.is_some()
        || !schema.refinement_clauses.is_empty()
    {
        let resolved = typed.symbols.symbols().nodes().is_valid(schema.symbol);
        let declaration = if resolved {
            typed.symbols.display_path(schema.symbol, "::")
        } else {
            schema.name.as_str().to_owned()
        };
        let source = resolved
            .then(|| typed.symbols.symbol_provenance_source_span(schema.symbol))
            .flatten()
            .and_then(|span| {
                typed
                    .symbols
                    .source_file(span)
                    .map(|file| (&file.path, span))
            });
        return Err(crate::read::unavailable(
            context,
            format!(
                "trait `{declaration}` cannot be lowered: resolved declaration={resolved}, lifetime parameters={}, conformance bounds={}, required traits={}, inheritance={}, refinements={}; source {source:?}",
                schema.lifetime_parameters.len(),
                schema.conformance_bounds.len(),
                schema.requires.len(),
                schema.refines.is_some(),
                schema.refinement_clauses.len(),
            ),
        ));
    }
    let parameters = typed
        .data_type_parameters
        .span(schema.type_parameters)
        .ok_or_else(|| crate::read::unavailable(context, "trait type telescope is stale"))?;
    let mut seen = Vec::with_capacity(parameters.len());
    for parameter in parameters {
        if !typed.symbols.symbols().nodes().is_valid(parameter.symbol)
            || seen.contains(&parameter.symbol)
            || parameter.bounds.multiplicity != language_core::Multiplicity::Affine
            || typed.symbols.get(parameter.symbol).parent != schema.symbol
        {
            return Err(crate::read::unavailable(
                context,
                "trait type telescope has an unsupported family or duplicate binder",
            ));
        }
        seen.push(parameter.symbol);
    }
    Ok(())
}

struct SignatureReferences<'a> {
    typed: &'a crate::source_state::TypedTrees,
    binders: Vec<SymbolHandle>,
    types: Vec<crate::source_state::types::TypeReferenceHandle>,
    expressions: Vec<crate::source_state::expression::ExpressionHandle>,
    contracts: Vec<crate::source_state::callable_values::RuntimeCallableContractHandle>,
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
impl crate::source_state::references::TypedReferences for SignatureReferences<'_> {
    fn derived_declaration_references(&self) -> bool {
        false
    }
    fn type_reference(
        &mut self,
        reference: crate::source_state::types::TypeReferenceHandle,
    ) -> bool {
        if !self
            .typed
            .type_reference_table
            .contains_type_reference(reference)
        {
            self.error = Some("static signature has a stale type reference");
            return false;
        }
        if let crate::source_state::types::TypeReferenceNode::Named { symbol, .. } =
            self.typed.type_reference_table.type_reference(reference)
            && self.typed.symbols.symbols().nodes().is_valid(*symbol)
            && self.typed.symbols.get(*symbol).kind == symbols::SymbolKind::TypeParameter
            && !self.binders.contains(symbol)
        {
            self.error = Some("static signature has an out-of-scope type binder");
            return false;
        }
        enter(&mut self.types, reference)
    }
    fn expression(
        &mut self,
        expression: crate::source_state::expression::ExpressionHandle,
    ) -> bool {
        if !self.typed.expression_table.expression_is_valid(expression) {
            self.error = Some("static signature has a stale contract operand");
            return false;
        }
        enter(&mut self.expressions, expression)
    }
    fn callable_contract(
        &mut self,
        contract: crate::source_state::callable_values::RuntimeCallableContractHandle,
    ) -> bool {
        if !self.typed.runtime_callable_contracts.is_valid(contract) {
            self.error = Some("static signature has a stale callable contract");
            return false;
        }
        enter(&mut self.contracts, contract)
    }
}
fn validate_references(
    typed: &crate::source_state::TypedTrees,
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
    crate::source_state::references::visit_signature(typed, &mut references, signature);
    references.error.map_or(Ok(()), |reason| {
        Err(crate::read::unavailable(context, reason))
    })
}

#[cfg(test)]
mod parent_contract_tests {
    use super::*;
    use crate::source_inputs::SourceMap;
    use crate::source_state::trait_definition::TraitRequirement;
    use crate::source_symbols::SymbolTableBuilder;
    use source_admission::SourceOrigin;
    use source_text::{SourceSpan, Span};
    use std::sync::Arc;
    use symbols::{SymbolKind, SymbolNameRef};

    #[test]
    fn an_ordinary_parent_with_a_method_requires_a_contract_owner() {
        let mut sources = SourceMap::default();
        let source = sources
            .add_with_metadata(
                "parents/main.omg".into(),
                "Wire Marker".into(),
                "parents".into(),
                semantic_vocabulary::PackageKeyIdentity::from_digest([7; 32]),
                SourceOrigin::User,
            )
            .source_id;
        let mut symbols = SymbolTableBuilder::with_sources(Some(Arc::new(sources)));
        let root = symbols.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let declarations = symbols.insert_children(
            root,
            [
                (
                    SymbolKind::Trait,
                    SymbolNameRef::Source(SourceSpan::new(source, Span::new(0, 4))),
                ),
                (
                    SymbolKind::Trait,
                    SymbolNameRef::Source(SourceSpan::new(source, Span::new(5, 11))),
                ),
            ],
        );
        let boundary = declarations.handle_at(0);
        let ordinary = declarations.handle_at(1);
        let method = symbols
            .insert_children(
                ordinary,
                [(SymbolKind::State, SymbolNameRef::Static("read"))],
            )
            .handle_at(0);
        let mut typed = crate::source_state::TypedTrees {
            symbols: symbols.finish(),
            ..Default::default()
        };
        let parents = typed.trait_requirements.insert_many([TraitRequirement {
            symbol: ordinary,
            name: crate::source_state::name::Identifier::generated("Marker"),
            lifetime_arguments: Vec::new(),
            arguments: Default::default(),
            source_span: SourceSpan::new(source, Span::new(5, 11)),
        }]);
        let unit = typed
            .type_reference_table
            .insert(crate::source_state::types::TypeReferenceNode::Unit);
        let machines = typed.trait_machine_signatures.insert_many([StateSignature {
            symbol: method,
            name: crate::source_state::name::Identifier::generated("read"),
            return_type: unit,
            ..Default::default()
        }]);
        typed.push_trait_definition(TraitDefinition {
            symbol: ordinary,
            name: crate::source_state::name::Identifier::generated("Marker"),
            machines,
            ..Default::default()
        });
        typed.push_trait_definition(TraitDefinition {
            symbol: boundary,
            name: crate::source_state::name::Identifier::generated("Wire"),
            is_boundary: true,
            requires: parents,
            ..Default::default()
        });
        let context = MachineId::new(1).unwrap();
        let mut program = Program::new(&typed, false);
        program.selected_declarations(context, &[boundary]).unwrap();
        assert_eq!(program.services.len(), 1);
        assert!(program.services.iter().all(|(_, (scope, parents))| {
            scope.declaration.symbol == boundary && parents.is_empty()
        }));
        assert_eq!(program.boundary_parent_contracts.len(), 1);
        let mut policies = super::super::calling::SourceCallingPolicies::default();
        let refusal = program
            .prepare_calling_policies(context, None, &mut policies)
            .expect_err("an ordinary parent cannot disappear without its contract owner");
        assert_eq!(
            refusal.reason,
            terminal_check_tables::RefusalReason::Unsupported
        );
        assert!(
            program
                .require_complete_boundary_parent_contracts(context, &policies)
                .is_err()
        );
    }
}
