//! A loop's decreasing measure must use the view that name resolution
//! selected. A familiar name does not turn an authored function into a
//! compiler ranking theory.
use crate::read::refuse_at;

use super::{Program, Slot};
use crate::Unserved;
use crate::body::{Expression, ExpressionKind, Policy, RankRange, Ranking, Strict, ValueType};
use crate::source_state::machine::Machine;
use semantic_vocabulary::RankingViewId;
use semantic_vocabulary::{IntegerSign, IntegerType, MachineId, ScalarType};
use source_declarations::RankingViewSelection;
use terminal_check_tables::Checked;

impl Program<'_> {
    fn ranking_view(&self, id: MachineId, machine: &Machine) -> Checked<(RankingViewId, bool)> {
        let unsupported = |detail| refuse_at(id, Unserved::Contract(detail));
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
                let catalog = RankingViewId::canonical(declaration.name.as_str())
                    .and_then(RankingViewId::catalog_declaration)
                    .ok_or_else(|| {
                        unsupported("selected ranking declaration has no catalog row")
                    })?;
                if declaration.supply_mode
                    != source_declarations::MachineSupplyMode::CompilerProvided
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
                        != crate::source_tables::ServiceReachRowTable::EMPTY_ROW
                    || declaration
                        .termination_plan
                        .implementation_witness
                        .is_some()
                    || declaration.termination_plan.interface
                        != source_declarations::TerminationInterface::Published(
                            source_declarations::TerminationGuarantee::NoGuarantee,
                        )
                {
                    return Err(unsupported(
                        "selected ranking declaration differs from its closed catalog header",
                    ));
                }
                let Some([entry]) = self.typed.machine_states(declaration).as_array() else {
                    return Err(unsupported(
                        "selected ranking declaration needs exactly one entry",
                    ));
                };
                let parameters = self.typed.state_parameters(entry);
                if parameters.len() != catalog.subject_count {
                    return Err(unsupported(match catalog.subject_count {
                        1 => "selected ranking declaration needs exactly one subject",
                        _ => "selected ranking declaration has another subject count",
                    }));
                }
                let plain_u64 = |reference| {
                    self.typed
                        .type_reference_table
                        .contains_type_reference(reference)
                        && matches!(
                            self.typed.type_reference_table.type_reference(reference),
                            crate::source_state::types::TypeReferenceNode::Named { symbol, .. }
                                if self.typed.symbols.builtin_type_atom(*symbol) == Some(symbols::BuiltinTypeAtom::U64)
                        )
                };
                if parameters.iter().any(|parameter| {
                    parameter.is_const
                        || parameter.is_mutable
                        || parameter.relevance != language_core::BindingRelevance::Relevant
                        || !plain_u64(parameter.type_reference)
                }) || !plain_u64(entry.return_type)
                    || !entry.contracts.is_empty()
                    || !entry.statement_nodes.is_empty()
                {
                    return Err(unsupported(
                        "selected ranking declaration must have its exact plain u64 to u64 header",
                    ));
                }
                (catalog.view, false)
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
    ) -> Checked<Option<Ranking>> {
        let Some(witness) = &machine.termination_plan.implementation_witness else {
            return Ok(None);
        };
        let unsupported = |detail| refuse_at(id, Unserved::Contract(detail));
        if self.mathematical {
            // contracts.md, "Citation and induction": "Proof/compile-time
            // recursion may rank on mathematical structure." The subject is
            // the formal the body dispatches on, and the signature admits a
            // recursive call only at a field of the dispatched case.
            let subjects =
                crate::source_state::ranking::resolve_machine_witness_subjects(self.typed, machine)
                    .ok_or_else(|| unsupported("ranking subjects have no resolved expressions"))?;
            let [subject] = subjects.as_slice() else {
                return Err(unsupported(
                    "structural ranking requires exactly one subject",
                ));
            };
            let value = self.expression(id, slots, *subject, None)?;
            if !matches!(value.result, Some(ValueType::Record(record, ..))
                if self.self_containing.binary_search(&record).is_ok())
            {
                return Err(unsupported(
                    "structural ranking needs data that contains itself",
                ));
            }
            return Ok(Some(Ranking::without_range(vec![value])));
        }
        let (view, default) = self.ranking_view(id, machine)?;
        let view_arguments = self
            .typed
            .ranking_expression_custody_for(machine.symbol)
            .map_or(&[][..], |custody| custody.view_arguments.as_slice());
        if view_arguments.len() != witness.view_arguments.len() {
            return Err(unsupported(
                "ranking view arguments have no resolved expressions",
            ));
        }
        if view != RankingViewId::NAT_INCREASING_TO && !view_arguments.is_empty() {
            return Err(unsupported("ranking view arguments"));
        }
        let mut fixed_bound = None;
        let subjects =
            crate::source_state::ranking::resolve_machine_witness_subjects(self.typed, machine)
                .filter(|subjects| !subjects.is_empty())
                .ok_or_else(|| unsupported("ranking subjects have no resolved expressions"))?;
        let rank = match (view, subjects.as_slice()) {
            (RankingViewId::NAT_BOUNDED_DISTANCE, [lower, upper]) => {
                let (cursor, limit) = self.distance_operands(id, slots, *lower, *upper, default)?;
                distance(cursor, limit)
            }
            (RankingViewId::NAT_INCREASING_TO, [cursor]) => {
                let [limit] = view_arguments else {
                    return Err(unsupported("IncreasingTo requires one fixed bound"));
                };
                let (cursor, limit) =
                    self.distance_operands(id, slots, *cursor, *limit, default)?;
                fixed_bound = Some(Box::new(limit.clone()));
                distance(cursor, limit)
            }
            (RankingViewId::NAT_DESCENDING, [subject]) => {
                let value = self.expression(id, slots, *subject, None)?;
                natural_subject(id, &value, default)?;
                value
            }
            (RankingViewId::SLICE_LENGTH, [subject]) => {
                let value = self.expression(id, slots, *subject, None)?;
                let source = value.source;
                self.measured_collection(
                    id,
                    slots,
                    value,
                    crate::source_state::expression::CollectionMeasure::Length,
                    source,
                )?
            }
            (RankingViewId::NAT_BOUNDED_DISTANCE, _) => {
                return Err(unsupported(
                    "bounded distance requires exactly two subjects",
                ));
            }
            (
                RankingViewId::NAT_DESCENDING
                | RankingViewId::NAT_INCREASING_TO
                | RankingViewId::SLICE_LENGTH,
                _,
            ) => {
                return Err(unsupported(
                    "selected ranking view requires exactly one subject",
                ));
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
        let range = if witness.rank_range.is_some() {
            let range = self
                .typed
                .ranking_expression_custody_for(machine.symbol)
                .and_then(|selected| selected.rank_range)
                .ok_or_else(|| unsupported("rank range has no resolved endpoints"))?;
            let crate::source_state::expression::ExpressionNode::Range(range) =
                *self.node(id, range)?
            else {
                return Err(unsupported("ranking range must be a resolved interval"));
            };
            if !range.start.is_valid() || !range.end.is_valid() {
                return Err(unsupported(
                    "ranking range needs a lower and an upper bound",
                ));
            }
            let scalar_type = rank.result.and_then(ValueType::scalar);
            let expected = scalar_type.map(ValueType::plain);
            let lower = self.expression(id, slots, range.start, expected)?;
            let upper = self.expression(id, slots, range.end, expected)?;
            if [lower.result, upper.result]
                .into_iter()
                .any(|ty| ty.and_then(ValueType::scalar) != scalar_type)
            {
                return Err(unsupported(
                    "ranking range bounds must use the rank's integer carrier",
                ));
            }
            Some(Box::new(RankRange {
                lower,
                upper,
                upper_inclusive: range.end_inclusive,
            }))
        } else {
            None
        };
        Ok(Some(Ranking {
            measures: vec![rank],
            fixed_bound,
            range,
        }))
    }

    fn distance_operands(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        lower: crate::source_state::expression::ExpressionHandle,
        upper: crate::source_state::expression::ExpressionHandle,
        default: bool,
    ) -> Checked<(Expression, Expression)> {
        use crate::source_state::expression::ExpressionNode;
        let anonymous = |node: &ExpressionNode| matches!(node, ExpressionNode::Integer(literal) if literal.landing().is_none());
        let lower_anonymous = anonymous(self.node(id, lower)?);
        let upper_anonymous = anonymous(self.node(id, upper)?);
        let (lower, upper) = if lower_anonymous && !upper_anonymous {
            let upper = self.expression(id, slots, upper, None)?;
            let lower = self.expression(id, slots, lower, upper.result)?;
            (lower, upper)
        } else {
            let lower_context = lower_anonymous.then_some(ValueType::plain(ScalarType::Integer(
                catalog_natural_carrier(),
            )));
            let lower = self.expression(id, slots, lower, lower_context)?;
            let upper_context = if upper_anonymous { lower.result } else { None };
            let upper = self.expression(id, slots, upper, upper_context)?;
            (lower, upper)
        };
        let lower_type = natural_subject(id, &lower, default)?;
        let upper_type = natural_subject(id, &upper, default)?;
        if lower_type != upper_type {
            return Err(refuse_at(
                id,
                Unserved::Contract("bounded distance needs one common unsigned integer carrier"),
            ));
        }
        Ok((lower, upper))
    }
}

// Both operands have the same unsigned carrier. The distance is zero past the limit.
fn distance(lower: Expression, upper: Expression) -> Expression {
    let result = lower.result;
    Expression {
        call_context: None,
        source: lower.source,
        kind: ExpressionKind::Strict {
            operation: Strict::Subtract,
            policy: Some(Policy::Saturating),
            left: Box::new(upper),
            right: Box::new(lower),
        },
        result: result.and_then(ValueType::scalar).map(ValueType::plain),
    }
}

fn catalog_natural_carrier() -> IntegerType {
    IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 is a valid carrier")
}

fn natural_subject(id: MachineId, value: &Expression, default: bool) -> Checked<IntegerType> {
    let unsupported = |detail| refuse_at(id, Unserved::Contract(detail));
    let Some(ValueType::Scalar(ScalarType::Integer(integer), _)) = value.result else {
        return Err(unsupported(
            "natural ranking needs an unsigned integer subject",
        ));
    };
    if integer.sign() != IntegerSign::Unsigned {
        return Err(unsupported(
            "natural ranking needs an unsigned integer subject",
        ));
    }
    let catalog = catalog_natural_carrier();
    // Restricting the u64 order and clamped difference to a smaller unsigned
    // carrier preserves both denotations and avoids executing widening casts.
    if !default && integer != catalog && !integer.can_widen_to(catalog) {
        return Err(unsupported(
            "selected natural ranking subject cannot embed exactly in its u64 input",
        ));
    }
    Ok(integer)
}
