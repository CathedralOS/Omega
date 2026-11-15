//! A loop's decreasing measure must use the view that name resolution
//! selected. A familiar name does not turn an authored function into a
//! compiler ranking theory.

use super::{Program, Slot};
use crate::body::{Expression, ValueType};
use crate::{Unserved, refuse};
use language_semantics::{RankingViewId, RankingViewSelection};
use semantic_vocabulary::{IntegerSign, MachineId, ScalarType};
use terminal_check_tables::{Checked, Construct};
use typed_trees::machine::Machine;

impl Program<'_> {
    fn ranking_view(&self, id: MachineId, machine: &Machine) -> Checked<(RankingViewId, bool)> {
        let unsupported = |detail| refuse(id, Construct::Machine, Unserved::Contract(detail));
        let custody = self
            .typed
            .ranking_expression_custody_for(machine.symbol)
            .ok_or_else(|| unsupported("ranking view has no resolved selection"))?;
        let selected = match custody.view {
            RankingViewSelection::Builtin(RankingViewId::SLICE_LENGTH) => {
                (RankingViewId::SLICE_LENGTH, false)
            }
            RankingViewSelection::CanonicalDefault(view) => (view, true),
            RankingViewSelection::Declaration(symbol) => {
                use language_core::sealed_intrinsics::{CompilerPrimitive, SealedIntrinsic};
                let declaration = self
                    .typed
                    .machine_by_symbol(symbol)
                    .ok_or_else(|| unsupported("selected ranking view has no declaration"))?;
                if self
                    .typed
                    .symbols
                    .sealed_declaration(symbol, declaration.name.as_str())
                    != Some(SealedIntrinsic::CompilerPrimitive(
                        CompilerPrimitive::NaturalRanking,
                    ))
                {
                    return Err(unsupported(
                        "selected ranking view is not an admitted compiler ranking declaration",
                    ));
                }
                if declaration.supply_mode
                    != language_semantics::MachineSupplyMode::CompilerProvided
                    || declaration.body_is_present
                    || !declaration.type_parameters.is_empty()
                    || !declaration.lifetime_parameters.is_empty()
                    || !declaration.satisfies.is_empty()
                    || !declaration.contracts.is_empty()
                    || declaration.spelling.is_some()
                    || declaration.target.is_some()
                    || declaration.target_family_interface
                    || declaration.suspends
                    || declaration.blocks
                    || !declaration.invokes.is_empty()
                    || declaration.service_reach_row
                        != language_semantics::ServiceReachRowTable::EMPTY_ROW
                    || declaration
                        .termination_plan
                        .implementation_witness
                        .is_some()
                    || declaration.termination_plan.interface
                        != language_semantics::TerminationInterface::Published(
                            language_semantics::TerminationGuarantee::NoGuarantee,
                        )
                {
                    return Err(unsupported(
                        "selected ranking declaration differs from its closed catalog header",
                    ));
                }
                let [entry] = self.typed.machine_states(declaration) else {
                    return Err(unsupported(
                        "selected ranking declaration needs exactly one entry",
                    ));
                };
                let [parameter] = self.typed.state_parameters(entry) else {
                    return Err(unsupported(
                        "selected ranking declaration needs exactly one subject",
                    ));
                };
                let plain_u64 = |reference| {
                    self.typed
                        .type_reference_table
                        .contains_type_reference(reference)
                        && matches!(
                            self.typed.type_reference_table.type_reference(reference),
                            typed_trees::types::TypeReferenceNode::Named { symbol, .. }
                                if self.typed.symbols.builtin_type_atom(*symbol) == Some(symbols::BuiltinTypeAtom::U64)
                        )
                };
                if parameter.is_const
                    || parameter.is_mutable
                    || parameter.relevance != language_core::BindingRelevance::Relevant
                    || !plain_u64(parameter.type_reference)
                    || !plain_u64(entry.return_type)
                    || !entry.contracts.is_empty()
                    || !entry.statement_nodes.is_empty()
                {
                    return Err(unsupported(
                        "selected ranking declaration must have its exact plain u64 to u64 header",
                    ));
                }
                (RankingViewId::NAT_DESCENDING, false)
            }
            RankingViewSelection::Unresolved | RankingViewSelection::Builtin(_) => {
                return Err(unsupported(
                    "ranking view has no admitted resolved selection",
                ));
            }
        };
        if machine
            .termination_plan
            .implementation_witness
            .as_ref()
            .is_none_or(|witness| witness.ranking_view != selected.0)
        {
            return Err(unsupported("ranking tag differs from its selected view"));
        }
        Ok(selected)
    }

    /// Read the selected view over the body's actual formal bindings. Keep
    /// private measure expressions separate from published call guarantees.
    pub(super) fn ranking(
        &mut self,
        id: MachineId,
        machine: &Machine,
        slots: &[Slot],
    ) -> Checked<Option<Vec<Expression>>> {
        let Some(witness) = &machine.termination_plan.implementation_witness else {
            return Ok(None);
        };
        let unsupported = |detail| refuse(id, Construct::Machine, Unserved::Contract(detail));
        let (view, default) = self.ranking_view(id, machine)?;
        if witness.rank_range.is_some() || !witness.view_arguments.is_empty() {
            return Err(unsupported("ranking view arguments and bounded ranges"));
        }
        let subjects = typed_trees::ranking::resolve_machine_witness_subjects(self.typed, machine)
            .filter(|subjects| !subjects.is_empty())
            .ok_or_else(|| unsupported("ranking subjects have no resolved expressions"))?;
        if matches!(
            view,
            RankingViewId::NAT_DESCENDING | RankingViewId::SLICE_LENGTH
        ) && subjects.len() != 1
        {
            return Err(unsupported(
                "selected ranking view requires exactly one subject",
            ));
        }
        let mut ranks = Vec::with_capacity(subjects.len());
        for subject in subjects {
            let value = self.expression(id, slots, subject, None)?;
            let rank = match view {
                RankingViewId::NAT_DESCENDING => {
                    let Some(ValueType::Scalar(ScalarType::Integer(integer), _)) = value.result
                    else {
                        return Err(unsupported(
                            "natural ranking needs an unsigned integer subject",
                        ));
                    };
                    if integer.sign() != IntegerSign::Unsigned {
                        return Err(unsupported(
                            "natural ranking needs an unsigned integer subject",
                        ));
                    }
                    let catalog_carrier =
                        semantic_vocabulary::IntegerType::new(IntegerSign::Unsigned, 64)
                            .expect("u64 is a valid carrier");
                    if !default
                        && integer != catalog_carrier
                        && !integer.can_widen_to(catalog_carrier)
                    {
                        return Err(unsupported(
                            "selected natural ranking subject cannot embed exactly in its u64 input",
                        ));
                    }
                    // The sealed view is the identity descending order on u64.
                    // A smaller unsigned input forms through the same total
                    // widening rule used by ordinary numeric conversions. Its
                    // source order is exactly the restriction of that order,
                    // so canonical checking can retain the original carrier
                    // and prove the same comparisons without runtime casts.
                    value
                }
                RankingViewId::SLICE_LENGTH => {
                    let source = value.source;
                    self.measured_collection(
                        id,
                        value,
                        typed_trees::expression::CollectionMeasure::Length,
                        source,
                    )?
                }
                _ => {
                    return Err(unsupported(
                        "ranking view has no normalized integer relation",
                    ));
                }
            };
            if !matches!(rank.result, Some(ValueType::Scalar(ScalarType::Integer(integer), _))
                if !integer.is_address() && integer.bits().is_some())
            {
                return Err(unsupported("ranking needs a closed finite integer carrier"));
            }
            ranks.push(rank);
        }
        Ok(Some(ranks))
    }
}
