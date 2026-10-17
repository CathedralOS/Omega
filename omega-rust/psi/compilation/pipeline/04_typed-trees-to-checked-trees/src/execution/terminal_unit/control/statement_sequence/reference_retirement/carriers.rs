//! Materialized root descriptors have occurrence identities even when no
//! source local holds them. The checked call's input-origin roster joins those
//! occurrences into the same disposal order as named loans. Empty source-loan
//! spans must not make an anonymous carrier disappear from that order.

use super::*;

impl Retirement<'_> {
    /// A temporary has no authored use after its enclosing statement, but its
    /// returned children can retain it. Reverse production schedules each child
    /// before the exact input descriptor that supplied it.
    pub(in crate::execution::terminal_unit::control::statement_sequence) fn carrier_children_retired(
        &self,
        parent: &CheckedUnitStructuralResultBindingPlan,
    ) -> bool {
        self.operations.iter().all(|operation| {
            if let CheckedUnitEffectOperationPlan::EstablishLocal(
                checked_trees::CheckedUnitLocalBindingPlan::StructuralValue {
                    result,
                    value,
                    calls,
                    ..
                },
            ) = operation
            {
                if result.binding_ordinal == parent.binding_ordinal
                    || self.carrier_retired(result, &mut Vec::new())
                {
                    return true;
                }
                let values = &self.facts.values.structural_values;
                let reference = values.nodes.get(*value).type_reference;
                let Some(leaves) = validation::reference_result_custody::record_carrier_sources(
                    self.program,
                    reference,
                    CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                        binding_ordinal: result.binding_ordinal,
                    },
                ) else {
                    return false;
                };
                return leaves.iter().all(|leaf| {
                    let Some(sources) =
                        validation::reference_result_custody::stored_carrier_sources(
                            self.program,
                            self.facts,
                            *value,
                            &leaf.path,
                        )
                    else {
                        return false;
                    };
                    sources.iter().all(|source| match source {
                        validation::reference_result_custody::StoredCarrierSource::Borrow {
                            source,
                            ..
                        } => !self.source_retains_carrier(source, parent),
                        validation::reference_result_custody::StoredCarrierSource::Call {
                            value,
                            ..
                        } => calls
                            .iter()
                            .find(|call| call.value == *value)
                            .and_then(|call| call.operation().call())
                            .is_some_and(|call| !self.call_retains_carrier(call, parent)),
                        // Copying a resident descriptor retains its original
                        // backing, not the bytes of the supplying carrier home.
                        validation::reference_result_custody::StoredCarrierSource::Place(_)
                        | validation::reference_result_custody::StoredCarrierSource::Image {
                            ..
                        } => true,
                    })
                });
            }
            // Forming a descriptor retains its exact backing occurrence,
            // even when no authored local gives the descriptor a loan row.
            // Calls and direct formations therefore share the same result
            // dependency order; source-loan absence is not early death.
            if let CheckedUnitEffectOperationPlan::EstablishReference { result, source, .. } =
                operation
            {
                return !self.source_retains_carrier(source, parent)
                    || self.carrier_retired(result, &mut Vec::new());
            }
            let Some(call) = operation.call() else {
                return true;
            };
            let Some(result) = call.structural_result() else {
                return true;
            };
            if result.binding_ordinal == parent.binding_ordinal
                || !self.order.precedes(
                    result.statement_index as usize,
                    self.statement_index as usize,
                )
            {
                return true;
            }
            let retains = self.call_retains_carrier(call, parent);
            !retains || self.carrier_retired(result, &mut Vec::new())
        })
    }

    fn call_retains_carrier(
        &self,
        call: &checked_trees::CheckedUnitCallPlan,
        parent: &CheckedUnitStructuralResultBindingPlan,
    ) -> bool {
        call.structural_custody().is_some_and(|custody| {
            custody.retained_reference_inputs.iter().any(|index| {
                call.structural_arguments
                    .get(*index as usize)
                    .is_some_and(|source| self.source_retains_carrier(source, parent))
            })
        })
    }

    fn source_retains_carrier(
        &self,
        source: &CheckedUnitStructuralArgumentPlan,
        parent: &CheckedUnitStructuralResultBindingPlan,
    ) -> bool {
        if let Some(binding) = source.source_structural_result_binding_ordinal() {
            binding == parent.binding_ordinal
        } else {
            source.structural_storage_root().is_some_and(|root| {
                self.results
                    .iter()
                    .any(|(binding, candidate)| binding == parent && *candidate == root)
            })
        }
    }

    fn carrier_retired(
        &self,
        result: &CheckedUnitStructuralResultBindingPlan,
        active: &mut Vec<u32>,
    ) -> bool {
        if active.contains(&result.binding_ordinal) {
            return false;
        }
        // A call's anonymous output can be the selected return without ever
        // acquiring a local symbol. Capture it before closing its inputs;
        // the independent completion judgment still checks their ancestry
        // and every sibling's actual disposition on this edge.
        if self.at_exit
            && self.returned.is_some_and(|returned| {
                super::returned_reference_binding(self.order, returned, result)
            })
        {
            return true;
        }
        let source = CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
            binding_ordinal: result.binding_ordinal,
        };
        if self
            .operations
            .iter()
            .chain(self.pending)
            .any(|operation| match operation {
                CheckedUnitEffectOperationPlan::ReleaseReference {
                    source: released,
                    path,
                    statement_index,
                    defer_to_exit,
                    ..
                } => {
                    *released == source
                        && path.is_empty()
                        && self
                            .order
                            .at_or_before(*statement_index as usize, self.statement_index as usize)
                        && (!*defer_to_exit || self.at_exit)
                }
                CheckedUnitEffectOperationPlan::ContinuationCleanup {
                    cleanup_actions,
                    boundary,
                    ..
                } => {
                    let statement = match boundary {
                        checked_trees::CheckedUnitCleanupBoundary::StatementEntry {
                            statement_index,
                        }
                        | checked_trees::CheckedUnitCleanupBoundary::Replacement {
                            statement_index,
                            ..
                        } => *statement_index,
                        _ => return false,
                    };
                    statement <= self.statement_index
                        && statement > result.statement_index
                        && cleanup_actions.iter().any(|action| {
                            matches!(action,
                            checked_trees::CheckedNormalCleanupAction::Discard(discard)
                                if discard.source == source && discard.path.is_empty())
                        })
                }
                _ => false,
            })
        {
            return true;
        }
        if let Some(loans) = super::carrier_loans(
            self.facts,
            self.machine,
            self.state.symbol,
            result,
            self.results,
        ) && !loans.is_empty()
            && self
                .facts
                .borrow
                .loan_group(loans)
                .is_some_and(|mut group| group.all(|(handle, _)| self.holder_retired(handle)))
        {
            return true;
        }
        let Some(transfers) = transfers(
            self.program,
            self.facts,
            self.results,
            result.binding_ordinal,
            self.operations,
        ) else {
            return false;
        };
        if !transfers.definite.iter().any(|path| path.is_empty()) {
            return false;
        }
        active.push(result.binding_ordinal);
        let retired = !transfers.possible.is_empty()
            && transfers.possible.iter().all(|transfer| {
                match &transfer.destination {
                    Destination::Holder(receiver, _) => self.carrier_retired(receiver, active),
                    // Consumption retires this holder, but a returned carrier
                    // can retain its parents. Follow that receiving occurrence
                    // before declaring the transferred descriptor settled.
                    Destination::Call {
                        statement_index,
                        structural_result,
                    } => {
                        if !self
                            .order
                            .precedes(*statement_index as usize, self.statement_index as usize)
                        {
                            return false;
                        }
                        if !structural_result {
                            // Scalar operand calls are retained computations,
                            // not standalone operations. Consumption ends the
                            // descriptor; any stored receiving loans still
                            // retain their exact supplying occurrence.
                            return super::carrier_loans(
                                self.facts,
                                self.machine,
                                self.state.symbol,
                                result,
                                self.results,
                            )
                            .is_some_and(|loans| {
                                self.facts
                                    .borrow
                                    .loan_group(loans)
                                    .is_some_and(|mut group| {
                                        group.all(|(supplier, _)| {
                                            self.call_receivers_retired(
                                                supplier,
                                                *statement_index,
                                                false,
                                                &mut Vec::new(),
                                            )
                                        })
                                    })
                            });
                        }
                        let mut found = false;
                        let retired = self
                            .operations
                            .iter()
                            .filter_map(|operation| operation.call())
                            .filter(|call| call.coordinate.statement_index == *statement_index)
                            .filter(|call| {
                                call.structural_arguments.iter().any(|argument| {
                                    argument.access == checked_trees::CheckedStructuralAccess::Owned
                                        && (argument.source_structural_result_binding_ordinal()
                                            == Some(result.binding_ordinal)
                                            || argument.structural_storage_root().is_some_and(
                                                |root| {
                                                    self.results.iter().any(
                                                        |(binding, candidate)| {
                                                            binding == result && *candidate == root
                                                        },
                                                    )
                                                },
                                            ))
                                })
                            })
                            .all(|call| {
                                found = true;
                                let Some(receiver) = call.structural_result() else {
                                    return true;
                                };
                                let Some(target) = self.program.state_by_symbol(call.target.state)
                                else {
                                    return false;
                                };
                                if validation::borrow_contracts::substituted_result_is_view_free(
                                    self.program,
                                    target.return_type,
                                    &[],
                                ) {
                                    return true;
                                }
                                self.carrier_retired(receiver, active)
                            });
                        found && retired
                    }
                    _ => false,
                }
            });
        active.pop();
        retired
    }
}
