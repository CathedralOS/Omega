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
*/

use crate::source_state::data::DataDefinition;
use crate::source_state::proposition::{PropositionBody, PropositionFormula};
use semantic_vocabulary::{MachineId, Proposition, StructuralTypeId};
use symbols::SymbolHandle;
use terminal_check_tables::{Checked, Construct};

use super::contracts::{AppliedProposition, ClauseScope, ResultTerm};
use super::places::RecordSource;
use super::{Program, Slot};
use crate::lowering::body::{RecordType, StorageShape, ValueType};
use crate::lowering::{Unserved, refuse};

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

impl Program<'_> {
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
                borrowed_regions: crate::lowering::body::Regions::NONE,
            });
        }
        if slots.len() != 2 {
            return Err(unserved(
                "quotient relation over something other than two values of its carrier",
            ));
        }
        let values = crate::lowering::body::formal_values(
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
        let conformance = typed
            .conformance_by_symbol(selection.conformance_symbol)
            .ok_or_else(|| unserved("quotient's equivalence is no declared conformance"))?;
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
            multiplicity: crate::lowering::body::source_multiplicity(data.properties.multiplicity),
            cleanup: None,
            identity: identity.clone(),
            shape: StorageShape::Sum(Vec::new()),
            requires: Vec::new(),
            authored_layout: false,
        });
        self.record_sources.push(RecordSource {
            constant_input_owner: None,
            declaration: symbol,
            cleanup: None,
            application: None,
            region_owner: SymbolHandle::invalid(),
            fields: Vec::new(),
            cases: Vec::new(),
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
}
