/*
Quotients of mathematical data.

`data Class = Representative % related requires related satisfies
Equivalence<Representative, related> as Name;` over data with no layout has
no layout either. It is read here for the mathematical signature: the carrier
it is a quotient of, the relation as the formula its proposition is declared
as, and the three rows of the equivalence it selects. The signature states
the quotient with the kernel's quotient declarations
(wiki/spec/proofs/quotients.md, "Set-quotient foundation") and checks each
row against the sealed statement of its law.

A quotient of data that has storage is not read here; the ordinary reader
refuses it as before.

An operation on a quotient is given by an operation on its representatives:
`machine class_op(value: Class) -> Class { Quotient::lift::<op, respects,
transports>(value) }`. Such a body is not lowered. It is read here as the
machines it selects and the formal each parameter of `op` receives, and the
signature states what each selected theorem must have proved
(wiki/spec/proofs/quotients.md, "Lifting operations").
*/

use crate::source_state::data::DataDefinition;
use crate::source_state::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, QuotientOperationKind,
    QuotientOperationRequest, StaticMachineArgument, TableCallExpression,
};
use crate::source_state::proposition::{PropositionBody, PropositionFormula};
use crate::source_state::signature::SignatureContractKind;
use crate::source_state::state::State;
use crate::source_state::statement::{
    StatementNode, TransitionExit, TransitionGuardNode, TransitionTargetNode,
};
use semantic_vocabulary::{MachineId, Proposition, StructuralFieldId, StructuralTypeId};
use symbols::SymbolHandle;
use terminal_check_tables::{Checked, Construct};

use super::contracts::{AppliedProposition, ClauseScope, ResultTerm};
use super::places::RecordSource;
use super::{Program, Slot};
use crate::body::{
    CallTarget, Ceiling, Expression, ExpressionKind, RecordField, RecordType, StorageShape,
    ValueType,
};
use crate::{SourceOrigin, Unserved, refuse};

/// A quotient of mathematical data, as the signature needs it.
#[derive(Debug, Clone)]
pub(crate) struct QuotientRecord {
    pub(crate) record: StructuralTypeId,
    pub(crate) declaration: SymbolHandle,
    pub(crate) identity: String,
    /// The data the quotient is of.
    pub(crate) carrier: StructuralTypeId,
    pub(crate) relation: SymbolHandle,
    pub(crate) relation_name: String,
    /// The relation's formula, over its two parameters as places 1 and 2.
    pub(crate) formula: Proposition,
    /// Each row of the selected equivalence: the requirement's name and the
    /// machine that proves it.
    pub(crate) rows: Vec<(String, SymbolHandle)>,
}

/// An executable quotient of custody-free copyable runtime data, represented as
/// an opaque single-field record holding its representative.
#[derive(Debug, Clone)]
pub(crate) struct ExecutableQuotient {
    pub(crate) record: StructuralTypeId,
    pub(crate) declaration: SymbolHandle,
    pub(crate) carrier: ValueType,
    pub(crate) relation: SymbolHandle,
}

/// A machine whose body is one `Quotient::lift` or `Quotient::define`.
#[derive(Debug, Clone)]
pub(crate) struct LiftedOperation {
    /// `Quotient::define`, which passes its formals through unchanged, and
    /// not `Quotient::lift`.
    pub(crate) define: bool,
    /// The operation on representatives.
    pub(crate) representative: SymbolHandle,
    /// The theorem that the operation takes related values to related
    /// values.
    pub(crate) congruence: SymbolHandle,
    /// The theorem that what this machine requires gives what the operation
    /// requires, when `Quotient::lift` selects one.
    pub(crate) transport: Option<SymbolHandle>,
    /// For each parameter of the operation, the position of the formal of
    /// this machine it receives.
    pub(crate) arguments: Vec<u32>,
}

impl Program<'_> {
    /// The quotient operation the body of machine `id` is, when the body is
    /// exactly one.
    pub(super) fn lifted_operation(
        &mut self,
        id: MachineId,
        state: &State,
    ) -> Checked<Option<LiftedOperation>> {
        let typed = self.typed;
        let unserved = |what| refuse(id, Construct::Machine, Unserved::Machine(what));
        let [statement] = typed.statement_table.statements(state.statement_nodes) else {
            return Ok(None);
        };
        let expression = match statement {
            StatementNode::Expression(expression) => *expression,
            StatementNode::Transition(transition)
                if transition.patterns.is_empty()
                    && transition.guard == TransitionGuardNode::Always
                    && transition.exit == TransitionExit::Ordinary
                    && !transition.continuation.is_valid()
                    && transition.proof_selectors.is_empty()
                    && transition.target.is_valid() =>
            {
                match typed.statement_table.transition_target(transition.target) {
                    TransitionTargetNode::Value(value) => *value,
                    _ => return Ok(None),
                }
            }
            _ => return Ok(None),
        };
        let ExpressionNode::Call(call) = typed.expression_table.expression(expression) else {
            return Ok(None);
        };
        let Some(request) = &call.quotient_operation else {
            return Ok(None);
        };
        let selected =
            |program: &mut Self, argument: &StaticMachineArgument| -> Checked<SymbolHandle> {
                if argument.application.is_some()
                    || argument.const_literal.is_some()
                    || argument.evidence_projection.is_some()
                {
                    return Err(unserved(
                        "quotient operation that selects a machine with arguments of its own",
                    ));
                }
                let machine = program
                    .entered_machine(argument.symbol)
                    .filter(|machine| {
                        typed
                            .machine_by_symbol(*machine)
                            .is_some_and(|declared| !program.is_generic_template(declared))
                    })
                    .ok_or_else(|| {
                        unserved("quotient operation that selects something other than a machine")
                    })?;
                // The signature needs the selected machine's definition or law.
                program.machine(machine)?;
                Ok(machine)
            };
        let representative = selected(self, &request.representative_operation)?;
        let (congruence, transport) = match request.theorem_evidence.as_ref() {
            [congruence] => (selected(self, &congruence.application)?, None),
            [congruence, transport] if request.kind == QuotientOperationKind::Lift => (
                selected(self, &congruence.application)?,
                Some(selected(self, &transport.application)?),
            ),
            _ => {
                return Err(unserved(
                    "quotient operation that selects other than a congruence and one transport",
                ));
            }
        };
        let formals = typed
            .state_parameters
            .span(state.parameters)
            .ok_or_else(|| unserved("quotient operation lost its machine's parameters"))?;
        let mut arguments = Vec::new();
        for &argument in typed.expression_table.expression_handles(call.arguments) {
            let formal = match typed.expression_table.expression(argument) {
                ExpressionNode::Name(path) => formals
                    .iter()
                    .position(|formal| formal.symbol == path.symbol),
                _ => None,
            }
            .ok_or_else(|| {
                unserved("argument of a quotient operation other than one of the machine's formals")
            })?;
            arguments.push(formal as u32);
        }
        Ok(Some(LiftedOperation {
            define: request.kind == QuotientOperationKind::Define,
            representative,
            congruence,
            transport,
            arguments,
        }))
    }

    /// The quotient a value type is, if it is one read here.
    pub(crate) fn quotient_of(&self, value_type: ValueType) -> Option<&QuotientRecord> {
        let ValueType::Record(record, ..) = value_type else {
            return None;
        };
        self.quotients
            .iter()
            .find(|quotient| quotient.record == record)
    }

    /// Reads a quotient declaration over mathematical data as a type of the
    /// mathematical signature.
    pub(super) fn quotient_record(
        &mut self,
        id: MachineId,
        symbol: SymbolHandle,
        data: &DataDefinition,
    ) -> Checked<ValueType> {
        if let Some(known) = self
            .quotients
            .iter()
            .find(|quotient| quotient.declaration == symbol)
        {
            return Ok(ValueType::record(known.record));
        }
        let typed = self.typed;
        let unserved = |what| refuse(id, Construct::Machine, Unserved::Type(what));
        let quotient = data
            .quotient
            .as_ref()
            .ok_or_else(|| unserved("quotient data lost its quotient"))?;
        if !data.type_parameters.is_empty()
            || !data.lifetime_parameters.is_empty()
            || !data.requires_facts.is_empty()
            || data.cleanup_selection.is_some()
        {
            return Err(unserved("generic or constrained quotient data"));
        }
        let identity = typed
            .normalized_nominal_type_identity(symbol)
            .and_then(|identity| typed.type_identity_text(identity))
            .filter(|identity| !identity.is_empty())
            .ok_or_else(|| unserved("data without a complete application identity"))?;
        let carrier_type = self.value_type(id, quotient.carrier)?;
        let ValueType::Record(carrier, ..) = carrier_type else {
            return Err(unserved("quotient of something that is not data"));
        };
        let carrier_declared =
            terminal_check_tables::row(&self.record_sources, carrier, id, Construct::Machine)?
                .declaration;
        if !typed
            .unique_data_definition_by_symbol(carrier_declared)
            .is_some_and(|declared| self.is_mathematical_data(declared))
        {
            return Err(unserved("quotient of data that has storage"));
        }
        // The relation is a named proposition declared as one formula over
        // two values of the carrier.
        let relation = typed
            .proposition_definition_by_symbol(quotient.relation_symbol)
            .ok_or_else(|| unserved("quotient relation that is no declared proposition"))?;
        let PropositionBody::Transparent {
            proposition: PropositionFormula::BooleanExpression(formula),
        } = &relation.body
        else {
            return Err(unserved("quotient relation declared without a formula"));
        };
        if !relation.binders.is_empty() {
            return Err(unserved("quotient relation with a binder"));
        }
        let parameters = typed
            .state_parameters
            .span(relation.parameters)
            .ok_or_else(|| unserved("quotient relation lost its parameters"))?;
        let mut slots = Vec::with_capacity(parameters.len());
        for parameter in parameters.iter() {
            let (value_type, erased, reference) = self.formal(id, parameter)?;
            if value_type != carrier_type || erased || reference.is_some() {
                return Err(unserved(
                    "quotient relation over something other than two values of its carrier",
                ));
            }
            slots.push(Slot {
                symbol: parameter.symbol,
                value_type,
                assignable: false,
                homed: false,
                erased: false,
                borrowed: None,
                borrowed_regions: crate::body::Regions::NONE,
            });
        }
        if slots.len() != 2 {
            return Err(unserved(
                "quotient relation over something other than two values of its carrier",
            ));
        }
        let values = crate::body::formal_values(
            slots
                .iter()
                .map(|slot| (slot.value_type, slot.erased, slot.borrowed.is_some())),
            None,
        );
        let scope = ClauseScope {
            slots: &slots,
            values: &values,
            bound: &[],
            owner: relation.symbol,
            result: ResultTerm::Absent("result in a relation"),
            subject: None,
            formal_fields: true,
            logical: None,
        };
        // What the formula writes was written in the proposition.
        self.applied_propositions.push(AppliedProposition {
            declaration: Some(relation.symbol),
            arguments: Vec::new(),
        });
        let read = self.clause(id, &scope, *formula);
        self.applied_propositions.pop();
        let formula = read?;
        let selection = quotient
            .equivalence
            .as_ref()
            .ok_or_else(|| unserved("quotient that selects no equivalence of its relation"))?;
        if selection.relation_symbol != quotient.relation_symbol {
            return Err(unserved(
                "quotient equivalence selection names a different relation",
            ));
        }
        let conformance = typed
            .conformance_by_symbol(selection.conformance_symbol)
            .ok_or_else(|| unserved("quotient's equivalence is no declared conformance"))?;
        if conformance.trait_symbol != selection.trait_symbol
            || typed
                .trait_definition_by_symbol(selection.trait_symbol)
                .is_none_or(|declaration| declaration.name.as_str() != "MereEquivalence")
        {
            return Err(unserved(
                "quotient must select a closed MereEquivalence conformance",
            ));
        }
        let conformance_args = typed
            .type_reference_table
            .type_reference_handles(conformance.arguments);
        let [arg_carrier, arg_relation] = conformance_args else {
            return Err(unserved(
                "quotient MereEquivalence conformance must apply to carrier and relation",
            ));
        };
        let named_symbol = |handle| match typed.type_reference_table.type_reference(handle) {
            crate::source_state::types::TypeReferenceNode::Named { symbol, .. } => Some(*symbol),
            _ => None,
        };
        if named_symbol(*arg_carrier) != Some(carrier_declared) {
            return Err(unserved(
                "quotient selects a MereEquivalence conformance proved for a different carrier",
            ));
        }
        if named_symbol(*arg_relation) != Some(quotient.relation_symbol) {
            return Err(unserved(
                "quotient selects a MereEquivalence conformance proved for a different relation",
            ));
        }
        let rows = conformance
            .rows
            .iter()
            .map(|row| {
                (
                    row.requirement_name.as_str().to_owned(),
                    row.realization_machine,
                )
            })
            .collect();
        let record = self.records.push(RecordType {
            multiplicity: crate::body::source_multiplicity(data.properties.multiplicity),
            cleanup: None,
            identity: identity.clone(),
            shape: StorageShape::Sum(Vec::new()),
            requires: Vec::new(),
            authored_layout: false,
            erased: Vec::new(),
        });
        self.record_sources.push(RecordSource {
            constant_input_owner: None,
            declaration: symbol,
            cleanup: None,
            application: None,
            region_owner: SymbolHandle::invalid(),
            fields: Vec::new(),
            cases: Vec::new(),
            erased: Vec::new(),
        });
        if let Err(position) = self.value_records.binary_search(&record) {
            self.value_records.insert(position, record);
        }
        self.admitted_declarations.include(symbol);
        self.quotients.push(QuotientRecord {
            record,
            declaration: symbol,
            identity,
            carrier,
            relation: relation.symbol,
            relation_name: relation.name.as_str().to_owned(),
            formula,
            rows,
        });
        Ok(ValueType::record(record))
    }

    /// The executable quotient a value type names, when it is one.
    pub(crate) fn executable_quotient_of(
        &self,
        value_type: ValueType,
    ) -> Option<&ExecutableQuotient> {
        let ValueType::Record(record, qualification, borrows) = value_type else {
            return None;
        };
        if !qualification.domains.is_empty() || borrows != crate::body::Regions::NONE {
            return None;
        }
        self.executable_quotients
            .iter()
            .find(|quotient| quotient.record == record)
    }

    /// Executable quotients require a copyable, custody-free carrier with no
    /// domain qualifications, references, cleanup, or affine/linear content
    /// (`wiki/spec/proofs/quotients.md`, "Formation and representation").
    fn check_custody_free_copy_carrier(
        &self,
        id: MachineId,
        value_type: ValueType,
        visiting: &mut Vec<StructuralTypeId>,
    ) -> Checked<()> {
        let unserved = |what| refuse(id, Construct::Machine, Unserved::Type(what));
        match value_type {
            ValueType::Scalar(_, qualification) => {
                if !qualification.domains.is_empty() {
                    return Err(unserved(
                        "executable quotient carrier with domain-qualified custody",
                    ));
                }
                Ok(())
            }
            ValueType::Record(record, qualification, borrows) => {
                if !qualification.domains.is_empty() || borrows != crate::body::Regions::NONE {
                    return Err(unserved(
                        "executable quotient carrier with routed or borrowed custody",
                    ));
                }
                if visiting.contains(&record) {
                    return Ok(());
                }
                visiting.push(record);
                let declared = self.data_type(id, record)?;
                if declared.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
                    || declared.cleanup.is_some()
                {
                    visiting.pop();
                    return Err(unserved(
                        "executable quotient carrier must be copyable without cleanup",
                    ));
                }
                match &declared.shape {
                    StorageShape::Record(fields) => {
                        for field in fields {
                            self.check_custody_free_copy_carrier(id, field.value_type, visiting)?;
                        }
                    }
                    StorageShape::Sum(cases) => {
                        for case in cases {
                            for field in &case.fields {
                                self.check_custody_free_copy_carrier(
                                    id,
                                    field.value_type,
                                    visiting,
                                )?;
                            }
                        }
                    }
                    StorageShape::FixedArray {
                        element,
                        qualification,
                        ..
                    } => {
                        if !qualification.domains.is_empty() {
                            visiting.pop();
                            return Err(unserved(
                                "executable quotient carrier with domain-qualified array element",
                            ));
                        }
                        self.check_custody_free_copy_carrier(
                            id,
                            ValueType::record(*element),
                            visiting,
                        )?;
                    }
                    StorageShape::PrimitiveScalar { qualification, .. } => {
                        if !qualification.domains.is_empty() {
                            visiting.pop();
                            return Err(unserved(
                                "executable quotient carrier with domain-qualified custody",
                            ));
                        }
                    }
                    StorageShape::Opaque
                    | StorageShape::TypeParameter(_)
                    | StorageShape::ByteSequence(_)
                    | StorageShape::Reference { .. }
                    | StorageShape::ElementView { .. }
                    | StorageShape::AtomicCell(_) => {
                        visiting.pop();
                        return Err(unserved(
                            "executable quotient carrier with unsupported or custody-bearing storage",
                        ));
                    }
                }
                visiting.pop();
                Ok(())
            }
            ValueType::Rigid(_) => Err(unserved(
                "executable quotient carrier must be a concrete copyable type",
            )),
        }
    }

    /// Reads an executable quotient declaration as an opaque single-field copy
    /// record holding its representative (`wiki/spec/proofs/quotients.md`,
    /// "Execution through ordinary checked bodies").
    pub(super) fn executable_quotient_record(
        &mut self,
        id: MachineId,
        symbol: SymbolHandle,
        data: &DataDefinition,
    ) -> Checked<ValueType> {
        if let Some(known) = self
            .executable_quotients
            .iter()
            .find(|quotient| quotient.declaration == symbol)
        {
            return Ok(ValueType::record(known.record));
        }
        let typed = self.typed;
        let unserved = |what| refuse(id, Construct::Machine, Unserved::Type(what));
        let quotient = data
            .quotient
            .as_ref()
            .ok_or_else(|| unserved("quotient data lost its quotient"))?;
        if !data.type_parameters.is_empty()
            || !data.lifetime_parameters.is_empty()
            || !data.requires_facts.is_empty()
            || data.cleanup_selection.is_some()
        {
            return Err(unserved("generic or constrained quotient data"));
        }
        if crate::body::source_multiplicity(data.properties.multiplicity)
            != terminal_psi::StructuralMultiplicity::Unrestricted
        {
            return Err(unserved("executable quotient must be copyable"));
        }
        let identity = typed
            .normalized_nominal_type_identity(symbol)
            .and_then(|identity| typed.type_identity_text(identity))
            .filter(|identity| !identity.is_empty())
            .ok_or_else(|| unserved("data without a complete application identity"))?;
        let carrier_type = self.value_type(id, quotient.carrier)?;
        self.check_custody_free_copy_carrier(id, carrier_type, &mut Vec::new())?;
        let carrier_declared = match carrier_type {
            ValueType::Record(carrier, _, _) => Some(
                terminal_check_tables::row(&self.record_sources, carrier, id, Construct::Machine)?
                    .declaration,
            ),
            _ => None,
        };
        let relation = typed
            .proposition_definition_by_symbol(quotient.relation_symbol)
            .ok_or_else(|| unserved("quotient relation that is no declared proposition"))?;
        let PropositionBody::Transparent {
            proposition: PropositionFormula::BooleanExpression(formula),
        } = &relation.body
        else {
            return Err(unserved("quotient relation declared without a formula"));
        };
        if !relation.binders.is_empty() {
            return Err(unserved("quotient relation with a binder"));
        }
        let parameters = typed
            .state_parameters
            .span(relation.parameters)
            .ok_or_else(|| unserved("quotient relation lost its parameters"))?;
        let mut slots = Vec::with_capacity(parameters.len());
        for parameter in parameters.iter() {
            let (value_type, erased, reference) = self.formal(id, parameter)?;
            if value_type != carrier_type || erased || reference.is_some() {
                return Err(unserved(
                    "quotient relation over something other than two values of its carrier",
                ));
            }
            slots.push(Slot {
                symbol: parameter.symbol,
                value_type,
                assignable: false,
                homed: false,
                erased: false,
                borrowed: None,
                borrowed_regions: crate::body::Regions::NONE,
            });
        }
        if slots.len() != 2 {
            return Err(unserved(
                "quotient relation over something other than two values of its carrier",
            ));
        }
        let values = crate::body::formal_values(
            slots
                .iter()
                .map(|slot| (slot.value_type, slot.erased, slot.borrowed.is_some())),
            None,
        );
        let scope = ClauseScope {
            slots: &slots,
            values: &values,
            bound: &[],
            owner: relation.symbol,
            result: ResultTerm::Absent("result in a relation"),
            subject: None,
            formal_fields: true,
            logical: None,
        };
        self.applied_propositions.push(AppliedProposition {
            declaration: Some(relation.symbol),
            arguments: Vec::new(),
        });
        let read = self.clause(id, &scope, *formula);
        self.applied_propositions.pop();
        let _formula = read?;
        let selection = quotient
            .equivalence
            .as_ref()
            .ok_or_else(|| unserved("quotient that selects no equivalence of its relation"))?;
        if selection.relation_symbol != quotient.relation_symbol {
            return Err(unserved(
                "quotient equivalence selection names a different relation",
            ));
        }
        let conformance = typed
            .conformance_by_symbol(selection.conformance_symbol)
            .ok_or_else(|| unserved("quotient's equivalence is no declared conformance"))?;
        // The trait's declared name, not its spelling here: the fixture
        // names it through a qualified scope (`|relation|::Equivalence`).
        if conformance.trait_symbol != selection.trait_symbol
            || typed
                .trait_definition_by_symbol(selection.trait_symbol)
                .is_none_or(|declaration| {
                    !matches!(declaration.name.as_str(), "Equivalence" | "MereEquivalence")
                })
        {
            return Err(unserved(
                "quotient must select a closed Equivalence or MereEquivalence conformance",
            ));
        }
        let conformance_args = typed
            .type_reference_table
            .type_reference_handles(conformance.arguments);
        let [arg_carrier, arg_relation] = conformance_args else {
            return Err(unserved(
                "quotient equivalence conformance must apply to carrier and relation",
            ));
        };
        let named_symbol = |handle| match typed.type_reference_table.type_reference(handle) {
            crate::source_state::types::TypeReferenceNode::Named { symbol, .. } => Some(*symbol),
            _ => None,
        };
        if let Some(carrier_declared) = carrier_declared
            && named_symbol(*arg_carrier) != Some(carrier_declared)
        {
            return Err(unserved(
                "quotient selects an equivalence conformance proved for a different carrier",
            ));
        }
        if named_symbol(*arg_relation) != Some(quotient.relation_symbol) {
            return Err(unserved(
                "quotient selects an equivalence conformance proved for a different relation",
            ));
        }
        let row_machines: Vec<SymbolHandle> = conformance
            .rows
            .iter()
            .map(|row| row.realization_machine)
            .collect();
        self.fields_minted += 1;
        let field_id = StructuralFieldId::new(self.fields_minted)
            .ok_or_else(|| unserved("field identity overflow"))?;
        let record = self.records.push(RecordType {
            multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
            cleanup: None,
            identity: identity.clone(),
            shape: StorageShape::Record(vec![RecordField {
                id: field_id,
                identity: format!("{identity}::representative"),
                value_type: carrier_type,
                byte_predicates: semantic_vocabulary::ByteSequencePredicateSet::EMPTY,
            }]),
            requires: Vec::new(),
            authored_layout: false,
            erased: Vec::new(),
        });
        // Use the quotient's own declaration symbol so record/source rosters
        // and reflection identities match while no authored field symbol can
        // ever match it for direct member projection or struct literals.
        self.record_sources.push(RecordSource {
            constant_input_owner: None,
            declaration: symbol,
            cleanup: None,
            application: None,
            region_owner: SymbolHandle::invalid(),
            fields: vec![symbol],
            cases: Vec::new(),
            erased: Vec::new(),
        });
        if let Err(position) = self.value_records.binary_search(&record) {
            self.value_records.insert(position, record);
        }
        self.admitted_declarations.include(symbol);
        self.executable_quotients.push(ExecutableQuotient {
            record,
            declaration: symbol,
            carrier: carrier_type,
            relation: relation.symbol,
        });
        // Read each equivalence law row so stage 06 checks its proof.
        for machine in row_machines {
            self.machine(machine)?;
        }
        Ok(ValueType::record(record))
    }

    /// Lowers an executable `Quotient::lift` or `Quotient::define` call as an
    /// ordinary checked call to its representative machine after validating its
    /// congruence and optional transport theorems.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn quotient_operation_call(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        handle: ExpressionHandle,
        call: &TableCallExpression,
        request: &QuotientOperationRequest,
        source: SourceOrigin,
        expected: Option<ValueType>,
    ) -> Checked<Expression> {
        let typed = self.typed;
        let unserved = |what| refuse(id, Construct::Machine, Unserved::Call(what));
        let select_machine = |program: &mut Self,
                              argument: &StaticMachineArgument|
         -> Checked<(SymbolHandle, MachineId)> {
            if argument.application.is_some()
                || argument.const_literal.is_some()
                || argument.evidence_projection.is_some()
            {
                return Err(unserved(
                    "quotient operation that selects a machine with arguments of its own",
                ));
            }
            let machine = program
                .entered_machine(argument.symbol)
                .filter(|machine| {
                    typed
                        .machine_by_symbol(*machine)
                        .is_some_and(|declared| !program.is_generic_template(declared))
                })
                .ok_or_else(|| {
                    unserved("quotient operation that selects something other than a machine")
                })?;
            let machine_id = program.machine(machine)?;
            Ok((machine, machine_id))
        };
        let (rep_symbol, rep_id) = select_machine(self, &request.representative_operation)?;
        let ((cong_symbol, cong_id), transport) = match request.theorem_evidence.as_ref() {
            [congruence] => (select_machine(self, &congruence.application)?, None),
            [congruence, transport] if request.kind == QuotientOperationKind::Lift => (
                select_machine(self, &congruence.application)?,
                Some(select_machine(self, &transport.application)?),
            ),
            _ => {
                return Err(unserved(
                    "quotient operation that selects other than a congruence and one transport",
                ));
            }
        };
        let is_pure_terminating = |sig: &crate::body::Signature| {
            sig.declared_crash_routes().is_none_or(<[_]>::is_empty)
                && (matches!(&sig.reaches, Ceiling::Inferred)
                    || matches!(&sig.reaches, Ceiling::Authored(items) if items.is_empty()))
                && (matches!(&sig.invokes, Ceiling::Inferred)
                    || matches!(&sig.invokes, Ceiling::Authored(items) if items.is_empty()))
                && sig.operational.suspension != Some(true)
                && sig.operational.blocking != Some(true)
        };
        let rep_sig = self
            .signatures
            .get(rep_id)
            .ok_or_else(|| unserved("quotient representative machine lost its signature"))?
            .clone();
        if !is_pure_terminating(&rep_sig)
            || !rep_sig.erased.is_empty()
            || !rep_sig.borrowed.is_empty()
        {
            return Err(unserved(
                "quotient representative operation must be pure, terminating, and value-taking",
            ));
        }
        let cong_sig = self
            .signatures
            .get(cong_id)
            .ok_or_else(|| unserved("quotient congruence theorem lost its signature"))?
            .clone();
        if cong_sig.result.is_some() || !is_pure_terminating(&cong_sig) {
            return Err(unserved(
                "quotient congruence theorem must be resultless, pure, and terminating",
            ));
        }
        if let Some((_, transport_id)) = transport {
            let transport_sig = self
                .signatures
                .get(transport_id)
                .ok_or_else(|| unserved("quotient transport theorem lost its signature"))?;
            if transport_sig.result.is_some() || !is_pure_terminating(transport_sig) {
                return Err(unserved(
                    "quotient transport theorem must be resultless, pure, and terminating",
                ));
            }
        }
        let argument_handles = typed.expression_table.expression_handles(call.arguments);
        if argument_handles.len() != rep_sig.parameters.len() {
            return Err(unserved(
                "quotient operation argument count does not match representative",
            ));
        }
        let mut lowered_arguments = Vec::with_capacity(argument_handles.len());
        let mut arg_quotients: Vec<Option<ExecutableQuotient>> =
            Vec::with_capacity(argument_handles.len());
        for (&arg_handle, &param_type) in argument_handles.iter().zip(&rep_sig.parameters) {
            let arg_expected = self.wholly_anonymous(arg_handle).then_some(param_type);
            let arg_expr = self.expression(id, slots, arg_handle, arg_expected)?;
            let arg_type = arg_expr
                .result
                .ok_or_else(|| unserved("quotient operation argument has no value"))?;
            if let Some(quotient) = self.executable_quotient_of(arg_type).cloned() {
                if quotient.carrier != param_type {
                    return Err(unserved(
                        "quotient argument carrier does not match representative parameter",
                    ));
                }
                let arg_source = arg_expr.source;
                lowered_arguments.push(Expression {
                    call_context: None,
                    kind: ExpressionKind::Field {
                        base: Box::new(arg_expr),
                        field: 0,
                    },
                    result: Some(quotient.carrier),
                    source: arg_source,
                });
                arg_quotients.push(Some(quotient));
            } else {
                if arg_type != param_type {
                    return Err(unserved(
                        "quotient operation argument type does not match representative parameter",
                    ));
                }
                lowered_arguments.push(arg_expr);
                arg_quotients.push(None);
            }
        }
        let constructed_result = {
            let reference = typed
                .expression_table
                .constructed_value_type_reference(handle);
            if typed
                .type_reference_table
                .contains_type_reference(reference)
            {
                Some(self.value_type(id, reference)?)
            } else {
                None
            }
        };
        let requested_result = expected.or(constructed_result);
        let result_quotient = match (rep_sig.result, requested_result) {
            (Some(rep_result), Some(requested)) => {
                if let Some(quotient) = self.executable_quotient_of(requested).cloned() {
                    if quotient.carrier != rep_result {
                        return Err(unserved(
                            "quotient result carrier does not match representative return type",
                        ));
                    }
                    Some(quotient)
                } else {
                    if requested != rep_result {
                        return Err(unserved(
                            "quotient operation result type does not match representative",
                        ));
                    }
                    None
                }
            }
            (Some(_), None) => None,
            (None, None) => None,
            (None, Some(_)) => {
                return Err(unserved("resultless representative cannot produce a value"));
            }
        };
        self.validate_executable_congruence(
            id,
            rep_symbol,
            &rep_sig,
            cong_symbol,
            &arg_quotients,
            result_quotient.as_ref(),
        )?;
        let call_expr = Expression {
            call_context: None,
            kind: ExpressionKind::Call {
                callee: CallTarget::Machine(rep_id),
                required_contract: None,
                static_reach_binding: None,
                static_reach_arguments: Vec::new(),
                proof_arguments: Vec::new(),
                arguments: lowered_arguments,
                designated: None,
            },
            result: rep_sig.result,
            source,
        };
        if let Some(quotient) = result_quotient {
            Ok(Expression {
                call_context: None,
                kind: ExpressionKind::Record {
                    fields: vec![(0, call_expr)],
                    erased: Vec::new(),
                },
                result: Some(ValueType::record(quotient.record)),
                source,
            })
        } else {
            Ok(call_expr)
        }
    }

    /// Validates that the selected congruence theorem proves the exact schema
    /// required by `representative` over left/right representatives of each
    /// quotient parameter and one shared binder per pass-through parameter.
    fn validate_executable_congruence(
        &mut self,
        id: MachineId,
        rep_symbol: SymbolHandle,
        rep_sig: &crate::body::Signature,
        cong_symbol: SymbolHandle,
        arg_quotients: &[Option<ExecutableQuotient>],
        result_quotient: Option<&ExecutableQuotient>,
    ) -> Checked<()> {
        let typed = self.typed;
        let unserved = |what| refuse(id, Construct::Machine, Unserved::Call(what));
        let (cong_machine, cong_state) = self.declaration(id, cong_symbol)?;
        let cong_params = typed
            .state_parameters
            .span(cong_state.parameters)
            .ok_or_else(|| unserved("quotient congruence theorem lost its parameters"))?;
        let mut left_syms = Vec::with_capacity(arg_quotients.len());
        let mut right_syms = Vec::with_capacity(arg_quotients.len());
        let mut cursor = 0usize;
        for (index, quotient) in arg_quotients.iter().enumerate() {
            let expected_ty = rep_sig.parameters[index];
            let first = cong_params
                .get(cursor)
                .ok_or_else(|| unserved("quotient congruence theorem has too few parameters"))?;
            let (first_ty, first_erased, first_ref) = self.formal(id, first)?;
            if first_ty != expected_ty || first_erased || first_ref.is_some() {
                return Err(unserved(
                    "quotient congruence theorem parameter type does not match representative",
                ));
            }
            cursor += 1;
            left_syms.push(first.symbol);
            if quotient.is_some() {
                let second = cong_params.get(cursor).ok_or_else(|| {
                    unserved("quotient congruence theorem missing right representative parameter")
                })?;
                let (second_ty, second_erased, second_ref) = self.formal(id, second)?;
                if second_ty != expected_ty || second_erased || second_ref.is_some() {
                    return Err(unserved(
                        "quotient congruence theorem right parameter type does not match carrier",
                    ));
                }
                cursor += 1;
                right_syms.push(second.symbol);
            } else {
                right_syms.push(first.symbol);
            }
        }
        if cursor != cong_params.len() {
            return Err(unserved(
                "quotient congruence theorem has unexpected extra parameters",
            ));
        }
        let mut requires_count = 0usize;
        let mut ensures_exprs = Vec::new();
        for contract in typed
            .machine_contracts(cong_machine)
            .iter()
            .chain(typed.state_contracts(cong_state).iter())
        {
            let facts = typed.proof_facts.span_or_empty(contract.facts);
            match contract.kind {
                SignatureContractKind::Requires => requires_count += facts.len(),
                SignatureContractKind::Ensures => {
                    for fact in facts {
                        let expr = fact.as_expression().copied().ok_or_else(|| {
                            unserved("quotient congruence ensures is not an expression")
                        })?;
                        ensures_exprs.push(expr);
                    }
                }
                _ => {}
            }
        }
        let quotient_arg_count = arg_quotients.iter().filter(|q| q.is_some()).count();
        let max_requires = quotient_arg_count + 2 * rep_sig.requires.len();
        if requires_count > max_requires {
            return Err(unserved(
                "quotient congruence theorem requires extra unallowed premises",
            ));
        }
        let [ensures_expr] = ensures_exprs.as_slice() else {
            return Err(unserved(
                "quotient congruence theorem must state exactly one ensures clause",
            ));
        };
        let table = &typed.expression_table;
        let (left_call_handle, right_call_handle) =
            match (result_quotient, table.expression(*ensures_expr)) {
                (Some(res_q), ExpressionNode::Call(call))
                    if call.target_symbol == res_q.relation && !call.receiver.is_valid() =>
                {
                    let args = table.expression_handles(call.arguments);
                    let [left, right] = args else {
                        return Err(unserved(
                            "quotient congruence ensures relation must apply to two results",
                        ));
                    };
                    (*left, *right)
                }
                (None, ExpressionNode::Binary(binary))
                    if binary.operator == BinaryOperator::Equal =>
                {
                    (binary.left, binary.right)
                }
                (None, ExpressionNode::Call(call))
                    if typed
                        .symbols
                        .fixed_core_declaration_for_symbol(call.target_symbol)
                        == Some(symbols::FixedCoreDeclaration::Identity) =>
                {
                    let args = table.expression_handles(call.arguments);
                    let [left, right] = args else {
                        return Err(unserved(
                            "quotient congruence Identity ensures must apply to two results",
                        ));
                    };
                    (*left, *right)
                }
                _ => {
                    return Err(unserved(
                        "quotient congruence theorem does not prove the expected result relation",
                    ));
                }
            };
        let check_side = |call_handle: ExpressionHandle,
                          expected_syms: &[SymbolHandle]|
         -> Checked<()> {
            let ExpressionNode::Call(side_call) = table.expression(call_handle) else {
                return Err(unserved(
                    "quotient congruence ensures must relate calls to the representative operation",
                ));
            };
            if self.entered_machine(side_call.target_symbol) != Some(rep_symbol) {
                return Err(unserved(
                    "quotient congruence ensures calls a different machine than the representative",
                ));
            }
            let side_args = table.expression_handles(side_call.arguments);
            if side_args.len() != expected_syms.len() {
                return Err(unserved(
                    "quotient congruence representative call has wrong argument count",
                ));
            }
            for (&arg_handle, &expected_sym) in side_args.iter().zip(expected_syms) {
                let ExpressionNode::Name(name) = table.expression(arg_handle) else {
                    return Err(unserved(
                        "quotient congruence representative call argument must name its formal",
                    ));
                };
                if name.symbol != expected_sym {
                    return Err(unserved(
                        "quotient congruence representative call passes the wrong formal",
                    ));
                }
            }
            Ok(())
        };
        check_side(left_call_handle, &left_syms)?;
        check_side(right_call_handle, &right_syms)?;
        Ok(())
    }
}
