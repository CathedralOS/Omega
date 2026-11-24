//! A qualification's exact core Content application. Its owner and algebra
//! are source declarations; neither field spelling nor a report fingerprint
//! identifies which application the machine implements.

use super::Program;
use crate::lowering::{Unserved, refuse};
use crate::source_state::domain::DomainDefinition;
use crate::source_state::machine::{Machine, SatisfiedDeclaration};
use crate::source_state::types::TypeReferenceNode;
use language_core::core_vocabulary::CoreVocabulary;
use semantic_vocabulary::MachineId;
use terminal_check_tables::{Checked, Construct};

impl Program<'_> {
    /// Check the source header before its ordinary mathematical or executable
    /// judgment. The projection body still owes that separate judgment.
    pub(super) fn projection_subject(&self, id: MachineId, machine: &Machine) -> Checked<()> {
        let typed = self.typed;
        if !typed
            .machine_trait_conformances(machine)
            .iter()
            .any(|edge| {
                typed.core_vocabulary(edge.symbol) == Some(CoreVocabulary::Content)
                    && edge
                        .requirement
                        .as_ref()
                        .is_some_and(|name| name.as_str() == "project")
            })
        {
            return Ok(());
        }
        let domain = typed
            .domain_definition(machine.attached_domain_symbol)
            .filter(|domain| domain.content_projections.contains(&machine.symbol))
            .ok_or_else(|| {
                refuse(
                    id,
                    Construct::Machine,
                    Unserved::Declaration(
                        "core Content projection lost its declaring domain's publication",
                    ),
                )
            })?;
        self.projection_header(id, domain, machine.symbol)
    }

    pub(super) fn projection_header(
        &self,
        id: MachineId,
        domain: &DomainDefinition,
        symbol: symbols::SymbolHandle,
    ) -> Checked<()> {
        let typed = self.typed;
        let missing = |what| refuse(id, Construct::Machine, Unserved::Declaration(what));
        let invalid = |rule| {
            crate::lowering::reject(
                id,
                crate::lowering::SourceOrigin::of(
                    typed
                        .symbols
                        .symbol_provenance_source_span(symbol)
                        .unwrap_or_default(),
                ),
                rule,
            )
        };
        let read = (|| {
            let (machine, state) = self.declaration(id, symbol)?;
            self.served(id, symbol)?;
            if machine.attached_domain_symbol != domain.symbol
                || !typed
                    .symbols
                    .same_symbol_source_package(symbol, domain.symbol)
                || !domain.type_parameters.is_empty()
                || !domain.index_arguments.is_empty()
            {
                return Err(missing(
                    "content projection without its closed declaring qualification",
                ));
            }
            let Some([edge]) = typed.machine_trait_conformances(machine).as_array() else {
                return Err(missing(
                    "content projection with additional conformance obligations",
                ));
            };
            let Some(SatisfiedDeclaration::Trait {
                definition,
                requirement,
            }) = crate::source_state::machine::resolve_satisfied_declaration(typed, machine, edge)
            else {
                return Err(missing(
                    "content projection lost its selected trait requirement",
                ));
            };
            if typed.core_vocabulary(definition.symbol) != Some(CoreVocabulary::Content)
                || requirement.name.as_str() != "project"
                || edge.requirement_symbol != requirement.symbol
                || edge.alias.is_some()
                || edge.via_expression.is_valid()
                || edge.external_binding.is_some()
                || !edge.trait_lifetime_arguments.is_empty()
            {
                return Err(missing(
                    "content projection does not implement the core project requirement",
                ));
            }
            let applied =
                crate::source_state::trait_definition::AppliedTraitScope::from_machine_conformance(
                    typed, machine, edge,
                )
                .ok_or_else(|| missing("content projection lost its complete trait application"))?;
            let [(_, owner), (_, algebra)] = applied.substitutions.as_slice() else {
                return Err(missing(
                    "content projection needs its owner and algebra applications",
                ));
            };
            let exact = |reference, declaration| {
                crate::source_state::reflection::construction::schema_type_identity(
                    typed,
                    reference,
                    declaration,
                )
                .map_err(|_| missing("content projection without an exact closed type identity"))
            };
            let Some([subject]) = typed.state_parameters(state).as_array() else {
                return Err(missing("content projection without exactly one subject"));
            };
            let TypeReferenceNode::Reference {
                referee,
                access: language_core::ReferenceAccess::Shared,
                ..
            } = typed
                .type_reference_table
                .type_reference(subject.type_reference)
            else {
                return Err(invalid(
                    "content projection subject must be a shared borrow of its owner",
                ));
            };
            if subject.is_const
                || subject.is_mutable
                || subject.relevance.is_erased()
                || exact(*referee, symbol)? != exact(*owner, symbol)?
                || exact(*owner, symbol)? != exact(domain.target_type, domain.symbol)?
                || exact(*algebra, symbol)? != exact(state.return_type, symbol)?
            {
                return Err(invalid(
                    "content projection signature must match its declaring carrier and selected algebra",
                ));
            }
            let TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                lifetime_arguments,
                ..
            } = typed.type_reference_table.type_reference(*algebra)
            else {
                return Err(invalid(
                    "core Content requires IntervalSet<Space> or CountedQuantity<Unit>",
                ));
            };
            let [parameter] = typed
                .type_reference_table
                .type_reference_handles(*arguments)
            else {
                return Err(missing(
                    "content algebra without exactly one coordinate space or unit",
                ));
            };
            if !lifetime_arguments.is_empty() {
                return Err(missing("content algebra with region arguments"));
            }
            if !matches!(
                typed.core_vocabulary(*base_symbol),
                Some(CoreVocabulary::IntervalSet | CoreVocabulary::CountedQuantity)
            ) {
                return Err(invalid(
                    "core Content requires IntervalSet<Space> or CountedQuantity<Unit>",
                ));
            }
            exact(*parameter, symbol)?;
            Ok(())
        })();
        read.map_err(|failure| self.read_failure_at(failure, symbol, "the core Content projection"))
    }
}
