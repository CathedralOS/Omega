//! The declaration tables of the typed trees: constants, data, domains,
//! proof facts, propositions, measures and operators.

use crate::typed_trees::{TypedTrees, proof_fact_source_span_index};
use crate::{data, domain, measure, proposition, signature};
use arena::{Handle, HandleSpan};
use symbols::JoinedPath;

impl TypedTrees {
    /// Semantic containment is not necessarily symbol allocation containment.
    /// Owned conformance rows are allocated as root machines, but capture the
    /// selecting declaration's telescope. Referenced machines capture nothing
    /// from a map that merely selects them. Executable copies use the same
    /// authored relationship; diagnostic qualified names establish no scope.
    pub fn declaration_scope_parent(&self, owner: symbols::SymbolHandle) -> symbols::SymbolHandle {
        let owner = self.lifetime_declaration_owner(owner);
        if !owner.is_valid() {
            return owner;
        }
        let mut selecting = self.conformances().iter().filter(|conformance| {
            conformance
                .rows
                .iter()
                .any(|row| row.source.owns_realization() && row.realization_machine == owner)
        });
        if let Some(conformance) = selecting.next() {
            return if selecting.next().is_none() {
                conformance.symbol
            } else {
                symbols::SymbolHandle::invalid()
            };
        }
        self.symbols.get(owner).parent
    }

    /// Authored applications still joined to retained source calls. Arena rows
    /// survive projection and therefore cannot define checking obligations.
    /// Uncalled authored bodies remain in the full Source machine roster.
    pub fn live_conformance_occurrence_applications(
        &self,
    ) -> Vec<crate::static_applications::StaticMachineApplicationHandle> {
        crate::references::collect_program_conformance_occurrences(self)
    }

    /// Census of occurrence/evidence selections, preserving each source anchor.
    pub fn closed_conformance_application_sites(
        &self,
    ) -> Vec<(
        super::ClosedConformanceApplicationSite,
        symbols::SymbolHandle,
        crate::static_applications::StaticMachineApplicationHandle,
        &super::ClosedConformanceApplication,
    )> {
        let mut sites = Vec::new();
        for (s, receipt) in self.machine_specializations.iter().enumerate() {
            for (a, application) in receipt.conformance_applications.iter().enumerate() {
                sites.push((
                    super::ClosedConformanceApplicationSite::Invocation {
                        specialization: s,
                        application: a,
                    },
                    receipt.instance,
                    receipt.binding_application,
                    application,
                ));
            }
        }
        for handle in self.live_conformance_occurrence_applications() {
            let source = self.expression_table.static_machine_application(handle);
            if let Some(application) = source
                .conformance_selection
                .as_ref()
                .and_then(|s| s.application.as_deref())
            {
                sites.push((
                    super::ClosedConformanceApplicationSite::Occurrence(handle),
                    source.lexical_owner,
                    handle,
                    application,
                ));
            }
        }
        sites
    }

    pub fn closed_conformance_application_at_mut(
        &mut self,
        site: super::ClosedConformanceApplicationSite,
    ) -> Option<&mut super::ClosedConformanceApplication> {
        match site {
            super::ClosedConformanceApplicationSite::Invocation {
                specialization,
                application,
            } => self
                .machine_specializations
                .get_mut(specialization)?
                .conformance_applications
                .get_mut(application),
            super::ClosedConformanceApplicationSite::Occurrence(handle) => self
                .expression_table
                .static_machine_application_mut(handle)
                .conformance_selection
                .as_mut()?
                .application
                .as_deref_mut(),
        }
    }

    /// The concrete machines a closed conformance application's occurrence
    /// telescope selected for its `Machine` binders, in binder order. Empty
    /// when the application retains no occurrence telescope.
    pub fn closed_application_selected_machines(
        &self,
        application: &super::ClosedConformanceApplication,
    ) -> &[crate::expression::StaticMachineArgument] {
        self.expression_table
            .static_machine_application_selected_machines(application.callable_application)
    }

    /// The value a specialization substitutes for one of its template's
    /// binders, when exactly one binding names that binder.
    pub fn specialization_binding(
        &self,
        specialization: &super::MachineSpecialization,
        parameter: symbols::SymbolHandle,
    ) -> Option<&crate::static_applications::StaticMachineBindingValue> {
        let mut bindings = self
            .expression_table
            .static_machine_application_bindings(specialization.binding_application)
            .iter()
            .filter(|binding| binding.parameter == parameter);
        let selected = bindings.next()?;
        bindings.next().is_none().then_some(&selected.value)
    }

    /// Rejoin the exact direct occurrence or the evidence-bound invocation.
    /// Neither an executable target nor another caller's tuple selects a map.
    pub fn selected_conformance_applications<'a>(
        &'a self,
        owner: symbols::SymbolHandle,
        selection_application: crate::static_applications::StaticMachineApplicationHandle,
        requirement: symbols::SymbolHandle,
        report_fingerprint: u64,
        commitment: super::ClosedConformanceApplicationCommitment,
    ) -> Vec<&'a super::ClosedConformanceApplication> {
        if selection_application.is_valid() {
            let source = self
                .expression_table
                .static_machine_application(selection_application);
            if source.lexical_owner != owner || source.registration_operation != requirement {
                return Vec::new();
            }
            let Some(selection) = &source.conformance_selection else {
                return Vec::new();
            };
            let Some(application) = selection.application.as_deref() else {
                return Vec::new();
            };
            let Some(operand) = self
                .expression_table
                .static_machine_application_arguments(selection_application)
                .get(selection.operand_ordinal as usize)
            else {
                return Vec::new();
            };
            let raw_arguments = operand
                .application
                .as_ref()
                .map_or(&[][..], |a| a.arguments.as_ref());
            let raw_lifetimes = operand
                .application
                .as_ref()
                .map_or(&[][..], |a| a.lifetime_arguments.as_ref());
            if raw_arguments != application.arguments.as_ref()
                || raw_lifetimes.len() != application.lifetime_arguments.len()
                || raw_lifetimes
                    .iter()
                    .zip(&application.lifetime_arguments)
                    .any(|(raw, actual)| raw.as_str() != actual)
                || operand.symbol != application.declaration
                || application.commitment != commitment
                || application.report_fingerprint != report_fingerprint
            {
                return Vec::new();
            }
            return vec![application];
        }
        super::closed_conformance_applications_of(
            &self.machine_specializations,
            owner,
            report_fingerprint,
            commitment,
        )
    }

    /// An authored member may be open because its selecting map owns captures,
    /// even when the member's independent invocation telescope is empty.
    /// Closed executable copies are not authored rows in that map.
    pub fn callable_body_has_open_parameters(&self, machine: &crate::machine::Machine) -> bool {
        machine.has_specialization_parameters()
            || self.conformances().iter().any(|conformance| {
                (!self.conformance_type_parameters(conformance).is_empty()
                    || !conformance.lifetime_parameters.is_empty())
                    && conformance.rows.iter().any(|row| {
                        row.source.owns_realization() && row.realization_machine == machine.symbol
                    })
            })
    }

    /// The unique authored template relation for an executable application.
    /// Capture and invocation are different records; consumers may project
    /// their shared declaration coordinate without replacing either frame.
    pub fn executable_application_template(
        &self,
        instance: symbols::SymbolHandle,
    ) -> Result<Option<symbols::SymbolHandle>, &'static str> {
        let mut templates = self
            .machine_specializations
            .iter()
            .filter(|row| row.instance == instance)
            .map(|row| row.template)
            .chain(
                self.captured_machine_applications
                    .iter()
                    .filter(|(_, row)| row.instance == instance)
                    .map(|(_, row)| row.template),
            );
        let template = templates.next();
        if templates.next().is_some() {
            return Err("executable application has ambiguous template ownership");
        }
        Ok(template)
    }

    /// Rejoin an executable entry through its invocation and capture applications.
    /// A selected map can supply captures only to its own exact authored row.
    pub fn realization_entry_template(
        &self,
        mut entry: symbols::SymbolHandle,
        application: &super::ClosedConformanceApplication,
    ) -> Option<(symbols::SymbolHandle, symbols::SymbolHandle)> {
        let mut visited = Vec::new();
        loop {
            let machine = self.machines().iter().find(|machine| {
                self.machine_states(machine)
                    .iter()
                    .any(|state| state.symbol == entry)
            })?;
            if visited.contains(&machine.symbol) {
                return None;
            }
            visited.push(machine.symbol);
            let invocations = self
                .machine_specializations
                .iter()
                .filter(|invocation| invocation.instance == machine.symbol)
                .collect::<Vec<_>>();
            let captures = self
                .captured_machine_applications
                .iter()
                .filter(|(_, capture)| capture.instance == machine.symbol)
                .map(|(_, capture)| capture)
                .collect::<Vec<_>>();
            if invocations.len() + captures.len() > 1 {
                return None;
            }
            if let [invocation] = invocations.as_slice() {
                let ordinal = self
                    .machine_states(machine)
                    .iter()
                    .position(|state| state.symbol == entry)?;
                let template = self.machine_by_symbol(invocation.template)?;
                entry = self.machine_states(template).get(ordinal)?.symbol;
                continue;
            }
            if let [capture] = captures.as_slice() {
                if &capture.conformance_application != application {
                    return None;
                }
                let selecting = self.conformance_by_symbol(application.declaration)?;
                if !selecting.rows.iter().any(|row| {
                    row.source.owns_realization() && row.realization_machine == capture.template
                }) {
                    return None;
                }
                let states = self.captured_machine_states.span_or_empty(capture.states);
                let mut states = states.iter().filter(|state| state.instance == entry);
                let template = states.next()?.template;
                if states.next().is_some() {
                    return None;
                }
                entry = template;
                continue;
            }
            return Some((machine.symbol, entry));
        }
    }

    /// Executable copying does not create new authored lifetime binders.
    /// Cyclic or malformed specialization custody has no declaration owner.
    pub fn lifetime_declaration_owner(
        &self,
        mut owner: symbols::SymbolHandle,
    ) -> symbols::SymbolHandle {
        let mut visited = Vec::new();
        if let Some(application) = self.closed_requirement_application(owner) {
            owner = application.template;
        }
        loop {
            let mut templates = self
                .machine_specializations
                .iter()
                .filter(|application| application.instance == owner)
                .map(|application| application.template)
                .chain(
                    self.captured_machine_applications
                        .iter()
                        .filter(|(_, application)| application.instance == owner)
                        .map(|(_, application)| application.template),
                );
            let Some(template) = templates.next() else {
                break;
            };
            if templates.next().is_some() {
                return symbols::SymbolHandle::invalid();
            }
            if visited.contains(&owner) {
                return symbols::SymbolHandle::invalid();
            }
            visited.push(owner);
            owner = template;
        }
        owner
    }
    super::typed_tree_table! {
        const_declarations => const_declarations(crate::constant::ConstDeclaration) [push_const_declaration];
        data_definitions => data_definitions(data::DataDefinition) [push_data_definition], inline;
        domain_definitions => domain_definitions(domain::DomainDefinition) [push_domain_definition];
        propositions => propositions(proposition::PropositionDefinition) [push_proposition];
        mathematical_definitions => mathematical_definitions(crate::mathematical::MathematicalDefinition);
        measures => measures(measure::MeasureDefinition) [push_measure];
        operators => operators(crate::operator::OperatorDefinition) [push_operator];
    }

    pub fn retain_authored_declaration_selections(
        &mut self,
        selections: language_semantics::declaration_selection::AuthoredDeclarationSelections,
    ) {
        self.tables.authored_declaration_selections = selections;
    }

    pub fn authored_declaration_selections(
        &self,
    ) -> &language_semantics::declaration_selection::AuthoredDeclarationSelections {
        &self.tables.authored_declaration_selections
    }

    /// The const declaration `symbol` declares, if any.
    pub fn const_declaration_by_symbol(
        &self,
        symbol: symbols::SymbolHandle,
    ) -> Option<&crate::constant::ConstDeclaration> {
        self.const_declarations()
            .iter()
            .find(|declaration| declaration.symbol == symbol)
    }

    pub fn record_resolved_authored_declaration_selection_once(
        &mut self,
        source_span: source::SourceSpan,
        exposure: language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure,
        kind: language_semantics::declaration_selection::AuthoredDeclarationSelectionKind,
        selected_symbol: symbols::SymbolHandle,
    ) -> Result<
        language_semantics::declaration_selection::AuthoredDeclarationSelectionOccurrenceId,
        language_semantics::declaration_selection::AuthoredDeclarationSelectionRecordError,
    > {
        if let Some(existing) = self
            .tables
            .authored_declaration_selections
            .resolved_occurrence(source_span, exposure, kind, selected_symbol)
        {
            return Ok(existing);
        }
        self.tables.authored_declaration_selections.record_resolved(
            source_span,
            exposure,
            kind,
            selected_symbol,
        )
    }

    /// Publish a derived application view without adding a nominal declaration.
    /// The type row retains the caller's region arguments; substituted fields
    /// keep the template's exact member symbols and source-selection custody.
    pub fn retain_data_application_definition(
        &mut self,
        application: crate::types::TypeReferenceHandle,
        definition: data::DataDefinition,
        is_closed: bool,
    ) {
        while self.tables.data_application_definitions.len()
            < self.type_reference_table.type_reference_count()
        {
            self.tables
                .data_application_definitions
                .insert(super::DataApplicationView::default());
        }
        let row = Handle::from_parts(application.arena_index(), application.generation());
        let definition = self.tables.data_definitions.insert(definition);
        *self.tables.data_application_definitions.get_mut(row) = super::DataApplicationView {
            definition,
            is_closed,
        };
    }

    /// Read only a producer-retained view for this exact application occurrence.
    pub fn data_application_definition(
        &self,
        application: crate::types::TypeReferenceHandle,
    ) -> Option<&data::DataDefinition> {
        if !self
            .type_reference_table
            .contains_type_reference(application)
        {
            return None;
        }
        let row = Handle::from_parts(application.arena_index(), application.generation());
        let definition = self.tables.data_application_definitions.get(row).definition;
        self.tables
            .data_definitions
            .is_valid(definition)
            .then(|| self.tables.data_definitions.get(definition))
    }

    /// Field substitution is not closure: open arguments remain universally
    /// checked, and cannot supply a concrete storage declaration.
    pub(crate) fn data_application_is_closed(
        &self,
        application: crate::types::TypeReferenceHandle,
    ) -> bool {
        let row = Handle::from_parts(application.arena_index(), application.generation());
        self.tables.data_application_definitions.get(row).is_closed
    }

    /// The first data declaration `name` spells, if any.
    pub fn data_definition_by_name(
        &self,
        name: &crate::name::Identifier,
    ) -> Option<&data::DataDefinition> {
        self.data_definitions()
            .iter()
            .find(|definition| definition.name == *name)
    }

    /// The first data declaration spelled `name`, if any.
    pub fn data_definition_by_name_str(&self, name: &str) -> Option<&data::DataDefinition> {
        self.data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == name)
    }

    /// The data declaration `machine` is attached to: the exact nominal
    /// symbol typing retained, never a same-spelled declaration from another
    /// package (every compilation loads bundled sources such as core's `Nat`
    /// beside the program's own). A machine without that symbol -- compiler
    /// synthesized, or a focused test program -- falls back to the one
    /// declaration of its attachment spelling; two such declarations resolve
    /// to none rather than to whichever loaded first.
    pub fn attached_data_definition(
        &self,
        machine: &crate::machine::Machine,
    ) -> Option<&data::DataDefinition> {
        if machine.attached_data_symbol.is_valid() {
            return self.data_definition_by_symbol(machine.attached_data_symbol);
        }
        let attached = machine.attached_data.as_ref()?;
        let mut matches = self
            .data_definitions()
            .iter()
            .filter(|definition| definition.name.as_str() == attached.as_str());
        let first = matches.next()?;
        matches.next().is_none().then_some(first)
    }

    pub fn data_type_parameters(
        &self,
        data_definition: &data::DataDefinition,
    ) -> &[data::TypeParameter] {
        self.data_type_parameters
            .span_or_empty(data_definition.type_parameters)
    }

    /// Syntax lowering places each ordinary Value binder's descriptor before
    /// authored members, in telescope order. Keep that producer association;
    /// executable consumers select the resulting field by its symbol.
    pub fn data_value_parameter_fields<'a>(
        &'a self,
        definition: &data::DataDefinition,
    ) -> impl Iterator<Item = (&'a data::TypeParameter, &'a data::DataField)> + 'a {
        self.data_type_parameters(definition)
            .iter()
            .filter(|parameter| matches!(parameter.kind, data::TypeParameterKind::Value { .. }))
            .zip(self.data_members(definition))
            .filter_map(|(parameter, member)| {
                let data::DataMember::Field(field) = member else {
                    return None;
                };
                // This is a producer invariant, never a spelling-based search.
                (parameter.name == field.name).then_some((parameter, field))
            })
    }

    pub fn push_data_member(
        &mut self,
        data_definition: &mut data::DataDefinition,
        member: data::DataMember,
    ) {
        self.data_members
            .append_to_span(&mut data_definition.members, member);
    }

    #[inline]
    pub fn data_members(&self, data_definition: &data::DataDefinition) -> &[data::DataMember] {
        self.data_members.span_or_empty(data_definition.members)
    }
    /// Every payload field across `members`: a field member contributes
    /// itself; a variant contributes its payload fields.
    pub fn data_member_fields<'a>(
        &'a self,
        members: &'a [data::DataMember],
    ) -> impl Iterator<Item = &'a data::DataField> + 'a {
        members.iter().flat_map(|member| match member {
            data::DataMember::Field(field) => std::slice::from_ref(field),
            data::DataMember::Variant(variant) => self.data_payload_fields(variant),
        })
    }

    /// The live `Field` members of `members`: erased fields are
    /// layout-resident but never runtime-addressable, and `Variant`
    /// members declare payload structure rather than a field position.
    pub fn live_data_fields<'a>(
        &'a self,
        members: &'a [data::DataMember],
    ) -> impl Iterator<Item = &'a data::DataField> + 'a {
        members.iter().filter_map(|member| match member {
            data::DataMember::Field(field) if !field.relevance.is_erased() => Some(field),
            _ => None,
        })
    }

    /// The unique non-primitive data definition a named type reference
    /// selects: `Ok(None)` for primitive or non-`Named` references and
    /// for a symbol that resolves to no definition, `Err` when the
    /// symbol is invalid, ambiguous, or mismatched with the authored
    /// name. `field_label` leads both rejections so callers name the
    /// traversal they were doing.
    pub fn exact_named_data_definition(
        &self,
        field_label: &str,
        type_reference: crate::types::TypeReferenceHandle,
    ) -> Result<Option<&data::DataDefinition>, String> {
        if self.primitive_type_reference(type_reference).is_some() {
            return Ok(None);
        }
        let crate::types::TypeReferenceNode::Named { symbol, name } =
            self.type_reference_table.type_reference(type_reference)
        else {
            return Ok(None);
        };
        if !symbol.is_valid() {
            return Err(format!(
                "{field_label} `{name}` has no exact nominal identity"
            ));
        }
        let mut matches = self
            .data_definitions()
            .iter()
            .filter(|definition| definition.symbol == *symbol);
        let Some(data) = matches.next() else {
            return Ok(None);
        };
        if matches.next().is_some() || data.name.as_str() != name.as_str() {
            return Err(format!(
                "{field_label} `{name}` has ambiguous or mismatched nominal identity"
            ));
        }
        Ok(Some(data))
    }

    /// The `Field` member `definition` declares as `symbol`, if any.
    pub fn data_field(
        &self,
        definition: &data::DataDefinition,
        symbol: symbols::SymbolHandle,
    ) -> Option<&data::DataField> {
        self.data_members(definition).iter().find_map(|member| {
            let data::DataMember::Field(field) = member else {
                return None;
            };
            (field.symbol == symbol).then_some(field)
        })
    }

    /// The field declared as `symbol` by any data definition — a member
    /// field or a variant payload field — with its owning definition.
    pub fn any_field_owner(
        &self,
        symbol: symbols::SymbolHandle,
    ) -> Option<(&data::DataDefinition, &data::DataField)> {
        self.data_definitions().iter().find_map(|definition| {
            self.data_member_fields(self.data_members(definition))
                .find(|field| field.symbol == symbol)
                .map(|field| (definition, field))
        })
    }

    /// The `Variant` member `definition` declares as `symbol`, if any.
    pub fn data_variant(
        &self,
        definition: &data::DataDefinition,
        symbol: symbols::SymbolHandle,
    ) -> Option<&data::DataVariant> {
        self.data_members(definition).iter().find_map(|member| {
            let data::DataMember::Variant(variant) = member else {
                return None;
            };
            (variant.symbol == symbol).then_some(variant)
        })
    }

    /// The `Field` member `definition` declares under `name`, if any.
    pub fn data_field_named(
        &self,
        definition: &data::DataDefinition,
        name: &str,
    ) -> Option<&data::DataField> {
        self.data_members(definition).iter().find_map(|member| {
            let data::DataMember::Field(field) = member else {
                return None;
            };
            (field.name.as_str() == name).then_some(field)
        })
    }

    /// The field declared as `symbol` by any of the program's data
    /// definitions, with its owning definition.
    pub fn field_owner(
        &self,
        symbol: symbols::SymbolHandle,
    ) -> Option<(&data::DataDefinition, &data::DataField)> {
        self.data_definitions().iter().find_map(|definition| {
            self.data_field(definition, symbol)
                .map(|field| (definition, field))
        })
    }

    /// The `Variant` member declared as `symbol` by any of the program's data
    /// definitions, with its owning definition.
    pub fn variant_owner(
        &self,
        symbol: symbols::SymbolHandle,
    ) -> Option<(&data::DataDefinition, &data::DataVariant)> {
        self.data_definitions().iter().find_map(|definition| {
            self.data_variant(definition, symbol)
                .map(|variant| (definition, variant))
        })
    }

    pub fn push_data_payload_field(
        &mut self,
        variant: &mut data::DataVariant,
        field: data::DataField,
    ) {
        self.data_payload_fields
            .append_to_span(&mut variant.payload, field);
    }

    #[inline]
    pub fn data_payload_fields(&self, variant: &data::DataVariant) -> &[data::DataField] {
        self.data_payload_fields.span_or_empty(variant.payload)
    }

    /// The payload field keyed `symbol` within `variant`, when declared.
    pub fn data_payload_field_by_symbol(
        &self,
        variant: &data::DataVariant,
        symbol: symbols::SymbolHandle,
    ) -> Option<&data::DataField> {
        self.data_payload_fields(variant)
            .iter()
            .find(|candidate| candidate.symbol == symbol)
    }

    pub fn domain_type_parameters(
        &self,
        domain: &domain::DomainDefinition,
    ) -> &[data::TypeParameter] {
        self.data_type_parameters
            .span_or_empty(domain.type_parameters)
    }

    pub fn push_domain_operator(
        &mut self,
        domain: &mut domain::DomainDefinition,
        operator: crate::operator::OperatorDefinition,
    ) {
        self.operators
            .append_to_span(&mut domain.operators, operator);
    }

    /// The domain definition `symbol` declares, if it exists.
    pub fn domain_definition(
        &self,
        symbol: symbols::SymbolHandle,
    ) -> Option<&domain::DomainDefinition> {
        self.domain_definitions()
            .iter()
            .find(|domain| domain.symbol == symbol)
    }

    pub fn domain_operators(
        &self,
        domain: &domain::DomainDefinition,
    ) -> &[crate::operator::OperatorDefinition] {
        self.operators.span_or_empty(domain.operators)
    }

    pub fn proof_facts(&self, domain: &domain::DomainDefinition) -> &[domain::ProofFact] {
        self.proof_facts.span_or_empty(domain.facts)
    }

    pub fn proof_fact_source_span(
        &self,
        handle: Handle<domain::ProofFact>,
    ) -> Option<source::SourceSpan> {
        self.tables
            .proof_fact_source_spans
            .get(proof_fact_source_span_index(handle))
            .copied()
            .flatten()
    }

    pub fn set_proof_fact_source_span(
        &mut self,
        handle: Handle<domain::ProofFact>,
        source_span: source::SourceSpan,
    ) {
        let index = proof_fact_source_span_index(handle);
        self.tables.proof_fact_source_spans.resize(index + 1, None);
        self.tables.proof_fact_source_spans[index] = Some(source_span);
    }

    /// The proposition definition `symbol` declares, if any.
    pub fn proposition_definition_by_symbol(
        &self,
        symbol: symbols::SymbolHandle,
    ) -> Option<&proposition::PropositionDefinition> {
        self.propositions()
            .iter()
            .find(|definition| definition.symbol == symbol)
    }

    pub fn proposition_binders(
        &self,
        proposition: &proposition::PropositionDefinition,
    ) -> &[proposition::PropositionBinder] {
        self.tables
            .proposition_binders
            .span_or_empty(proposition.binders)
    }

    pub fn push_proposition_binder(
        &mut self,
        proposition: &mut proposition::PropositionDefinition,
        binder: proposition::PropositionBinder,
    ) {
        self.tables
            .proposition_binders
            .append_to_span(&mut proposition.binders, binder);
    }

    pub fn proposition_parameters(
        &self,
        proposition: &proposition::PropositionDefinition,
    ) -> &[signature::StateParameter] {
        self.tables
            .state_parameters
            .span_or_empty(proposition.parameters)
    }

    pub fn push_proposition_parameter(
        &mut self,
        proposition: &mut proposition::PropositionDefinition,
        parameter: signature::StateParameter,
    ) {
        self.tables
            .state_parameters
            .append_to_span(&mut proposition.parameters, parameter);
    }

    pub fn push_mathematical_definition(
        &mut self,
        definition: crate::mathematical::MathematicalDefinition,
    ) {
        self.tables
            .mathematical_definitions
            .append_to_span(&mut self.roots.mathematical_definitions, definition);
    }

    pub fn push_mathematical_parameter(
        &mut self,
        definition: &mut crate::mathematical::MathematicalDefinition,
        parameter: crate::mathematical::MathematicalParameter,
    ) {
        self.tables
            .mathematical_parameters
            .append_to_span(&mut definition.parameters, parameter);
    }

    pub fn mathematical_parameters(
        &self,
        span: HandleSpan<crate::mathematical::MathematicalParameter>,
    ) -> &[crate::mathematical::MathematicalParameter] {
        self.tables.mathematical_parameters.span_or_empty(span)
    }

    pub fn insert_mathematical_type(
        &mut self,
        ty: crate::mathematical::MathematicalType,
    ) -> crate::mathematical::MathematicalTypeHandle {
        self.tables.mathematical_types.insert(ty)
    }

    pub fn mathematical_type(
        &self,
        handle: crate::mathematical::MathematicalTypeHandle,
    ) -> &crate::mathematical::MathematicalType {
        self.tables.mathematical_types.get(handle)
    }

    pub fn domain_path_members(
        &self,
        span: HandleSpan<crate::name::Identifier>,
    ) -> &[crate::name::Identifier] {
        self.domain_path_members.span_or_empty(span)
    }

    pub fn push_measure_path_member(
        &mut self,
        measure: &mut measure::MeasureDefinition,
        member: crate::name::Identifier,
    ) {
        self.measure_path_members
            .append_to_span(&mut measure.name, member);
    }

    pub fn measure_path_members(
        &self,
        span: HandleSpan<crate::name::Identifier>,
    ) -> &[crate::name::Identifier] {
        self.measure_path_members.span_or_empty(span)
    }

    /// The operator definition `symbol` declares, if any.
    pub fn operator_definition_by_symbol(
        &self,
        symbol: symbols::SymbolHandle,
    ) -> Option<&crate::operator::OperatorDefinition> {
        self.operators()
            .iter()
            .find(|definition| definition.symbol == symbol)
    }

    /// Append one token-bearing machine's operator-signature view. The view
    /// shares the machine's symbol and its parameter, contract, and type
    /// parameter spans; it is never a second declaration.
    pub fn push_machine_token_binding(&mut self, view: crate::operator::OperatorDefinition) {
        self.tables
            .operators
            .append_to_span(&mut self.roots.machine_token_bindings, view);
    }

    /// The operator-signature views of every token-bearing machine, in
    /// machine declaration order.
    pub fn machine_token_bindings(&self) -> &[crate::operator::OperatorDefinition] {
        self.tables
            .operators
            .span_or_empty(self.roots.machine_token_bindings)
    }

    /// The token-bearing machine binding `symbol` declares, if any.
    pub fn machine_token_binding_by_symbol(
        &self,
        symbol: symbols::SymbolHandle,
    ) -> Option<&crate::operator::OperatorDefinition> {
        self.machine_token_bindings()
            .iter()
            .find(|binding| binding.symbol == symbol)
    }

    /// Every authored operator: free operators plus each domain definition's
    /// members, in declaration order. Token-bearing machine signature views
    /// are not declarations; reach them through [`Self::machine_token_bindings`].
    pub fn all_operators(&self) -> impl Iterator<Item = &crate::operator::OperatorDefinition> + '_ {
        self.operators().iter().chain(
            self.domain_definitions()
                .iter()
                .flat_map(|domain| self.domain_operators(domain)),
        )
    }

    pub fn push_operator_path_member(
        &mut self,
        operator: &mut crate::operator::OperatorDefinition,
        member: crate::name::Identifier,
    ) {
        self.operator_path_members
            .append_to_span(&mut operator.name, member);
    }

    pub fn operator_type_parameters(
        &self,
        operator: &crate::operator::OperatorDefinition,
    ) -> &[data::TypeParameter] {
        self.data_type_parameters
            .span_or_empty(operator.type_parameters)
    }

    pub fn push_operator_parameter(
        &mut self,
        operator: &mut crate::operator::OperatorDefinition,
        parameter: signature::StateParameter,
    ) {
        self.state_parameters
            .append_to_span(&mut operator.parameters, parameter);
    }

    pub fn operator_parameters(
        &self,
        operator: &crate::operator::OperatorDefinition,
    ) -> &[signature::StateParameter] {
        self.state_parameters.span_or_empty(operator.parameters)
    }

    pub fn push_operator_contract(
        &mut self,
        operator: &mut crate::operator::OperatorDefinition,
        contract: signature::SignatureContract,
    ) {
        self.signature_contracts
            .append_to_span(&mut operator.contracts, contract);
    }

    pub fn operator_contracts(
        &self,
        operator: &crate::operator::OperatorDefinition,
    ) -> &[signature::SignatureContract] {
        self.signature_contracts.span_or_empty(operator.contracts)
    }

    pub fn operator_path_members(
        &self,
        span: HandleSpan<crate::name::Identifier>,
    ) -> &[crate::name::Identifier] {
        self.operator_path_members.span_or_empty(span)
    }

    /// The operator's `::`-joined path string for diagnostics and lookups.
    pub fn operator_path_string(&self, span: HandleSpan<crate::name::Identifier>) -> String {
        self.operator_path_members(span).joined_path()
    }
}
