//! Post-typing replay and installation of one derived placed-view plan.

use std::collections::BTreeMap;

use psi_access_plans::{AtomicPermissions, FieldAccess, ValidatedPlacementPlan};
use psi_diagnostics::Diagnostic;
use psi_language_core::atomic::AtomicObservingCompareExchangeOperation;
use psi_language_semantics::Multiplicity;
use psi_typed_trees::TypedTrees;

use super::{PlacedViewRecord, accessor_operations};

fn observing_result_contracts(
    operations: AtomicPermissions,
    multiplicity: Multiplicity,
) -> Result<
    Option<Vec<psi_typed_trees::typed_trees::PlacedAtomicObservingResultContract>>,
    Multiplicity,
> {
    if !operations.compare_exchange && !operations.compare_exchange_once {
        return Ok(None);
    }
    if multiplicity != Multiplicity::Unrestricted {
        return Err(multiplicity);
    }
    let mut observing_results = Vec::new();
    if operations.compare_exchange {
        let operation = AtomicObservingCompareExchangeOperation::Decisive;
        observing_results.push(
            psi_typed_trees::typed_trees::PlacedAtomicObservingResultContract {
                operation,
                result_shape: operation.result_shape(),
            },
        );
    }
    if operations.compare_exchange_once {
        let operation = AtomicObservingCompareExchangeOperation::SingleAttempt;
        observing_results.push(
            psi_typed_trees::typed_trees::PlacedAtomicObservingResultContract {
                operation,
                result_shape: operation.result_shape(),
            },
        );
    }
    Ok(Some(observing_results))
}

pub(super) fn install_placed_view_plan(
    typed: &mut TypedTrees,
    record: &PlacedViewRecord,
    placement: &ValidatedPlacementPlan,
) -> Result<(), Vec<Diagnostic>> {
    let policy_name = record
        .policy_machine
        .strip_suffix("::plan")
        .unwrap_or(&record.policy_machine);
    let policy = typed
        .data_definitions()
        .iter()
        .find(|definition| {
            definition.name.as_str() == policy_name
                && typed.symbols.symbol_source_span(definition.symbol) == Some(record.policy_source)
        })
        .ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "placed view `{}` lost nominal policy `{}` after typing",
                record.synthetic_name, record.policy_machine
            ))]
        })?;
    let policy_plan_machine = typed
        .machines()
        .iter()
        .find(|machine| {
            machine.name.as_str() == record.policy_machine
                && typed.symbols.symbol_source_span(machine.symbol)
                    == Some(record.policy_machine_source)
        })
        .ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "placed view `{}` lost exact policy machine `{}` after typing",
                record.synthetic_name, record.policy_machine
            ))]
        })?;
    let schema = typed
        .data_definitions()
        .iter()
        .find(|definition| {
            definition.name.as_str() == record.schema_data
                && typed.symbols.symbol_source_span(definition.symbol) == Some(record.schema_source)
        })
        .ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "placed view `{}` lost schema `{}` after typing",
                record.synthetic_name, record.schema_data
            ))]
        })?;
    let schema_fields = typed
        .data_members(schema)
        .iter()
        .filter_map(|member| match member {
            psi_typed_trees::data::DataMember::Field(field) => Some(field),
            psi_typed_trees::data::DataMember::Variant(_) => None,
        })
        .map(|field| (field.name.as_str().to_owned(), field))
        .collect::<BTreeMap<_, _>>();
    let view = typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == record.synthetic_name)
        .ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "placed view `{}` lost its derived data definition after typing",
                record.synthetic_name
            ))]
        })?;
    let view_fields = typed
        .data_members(view)
        .iter()
        .filter_map(|member| match member {
            psi_typed_trees::data::DataMember::Field(field) => {
                Some((field.name.as_str().to_owned(), field))
            }
            psi_typed_trees::data::DataMember::Variant(_) => None,
        })
        .collect::<BTreeMap<_, _>>();
    let schema_symbol = schema.symbol;
    let data_symbol = view.symbol;
    let policy_symbol = policy.symbol;
    let policy_plan_machine_symbol = policy_plan_machine.symbol;

    let mut fields = Vec::new();
    for entry in placement.access().plan().entries() {
        if matches!(entry.access(), FieldAccess::Inaccessible) {
            continue;
        }
        let schema_field = schema_fields.get(entry.field()).copied().ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "placed view `{}` lost schema field `{}` after typing",
                record.synthetic_name,
                entry.field()
            ))]
        })?;
        let view_field = view_fields.get(entry.field()).copied().ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "placed view `{}` lost admitted field `{}` after typing",
                record.synthetic_name,
                entry.field()
            ))]
        })?;
        if schema_field.identity != view_field.identity {
            return Err(vec![Diagnostic::error(format!(
                "placed view `{}` field `{}` changed stable member identity during accessor derivation",
                record.synthetic_name,
                entry.field()
            ))]);
        }
        let accessor_name = typed
            .named_type_reference(view_field.type_reference)
            .map(|name| name.as_str().to_owned())
            .ok_or_else(|| {
                vec![Diagnostic::error(format!(
                    "placed view `{}` field `{}` lost its nominal accessor type after typing",
                    record.synthetic_name,
                    entry.field()
                ))]
            })?;
        let accessor_type_symbol = typed
            .type_reference_table
            .type_symbol(view_field.type_reference);
        let accessor_data = typed
            .data_definitions()
            .iter()
            .filter(|definition| {
                if accessor_type_symbol.is_valid() {
                    definition.symbol == accessor_type_symbol
                } else {
                    // Atomic synthesized carriers currently retain their
                    // specialized nominal spelling while their operation law
                    // remains in the exact Atomic typed carrier.
                    matches!(entry.access(), FieldAccess::Atomic { .. })
                        && definition.name.as_str() == accessor_name
                }
            })
            .collect::<Vec<_>>();
        let [accessor_data] = accessor_data.as_slice() else {
            return Err(vec![Diagnostic::error(format!(
                "placed view `{}` field `{}` must retain one exact generated accessor data definition",
                record.synthetic_name,
                entry.field()
            ))]);
        };
        let mut accessor_targets = Vec::new();
        for operation in accessor_operations(entry.access()) {
            let machines = typed
                .machines()
                .iter()
                .filter(|machine| {
                    machine.attached_data.as_ref().is_some_and(|attached| {
                        attached.as_str() == accessor_name
                            && machine
                                .name
                                .as_str()
                                .rsplit("::")
                                .next()
                                .is_some_and(|name| name == operation)
                    })
                })
                .collect::<Vec<_>>();
            let [machine] = machines.as_slice() else {
                return Err(vec![Diagnostic::error(format!(
                    "placed view `{}` field `{}` must retain one exact generated `{operation}` accessor machine",
                    record.synthetic_name,
                    entry.field()
                ))]);
            };
            let [state] = typed.machine_states(machine) else {
                return Err(vec![Diagnostic::error(format!(
                    "placed view `{}` field `{}` generated `{operation}` accessor must have one exact callable state",
                    record.synthetic_name,
                    entry.field()
                ))]);
            };
            accessor_targets.push(psi_typed_trees::typed_trees::PlacedAccessorTarget {
                operation: operation.to_owned(),
                machine_symbol: machine.symbol,
                state_symbol: state.symbol,
            });
        }
        let atomic_resident = match entry.access() {
            FieldAccess::Atomic {
                transfer_width_bits,
                operations,
                ..
            } => {
                let multiplicity = typed.type_multiplicity(schema_field.type_reference);
                match observing_result_contracts(*operations, multiplicity) {
                    Ok(None) => None,
                    Ok(Some(observing_results)) => {
                        Some(psi_typed_trees::typed_trees::PlacedAtomicResidentContract {
                            field_symbol: schema_field.symbol,
                            resident_type: schema_field.type_reference,
                            multiplicity,
                            transfer_width_bits: *transfer_width_bits,
                            compare_exchange: operations.compare_exchange,
                            compare_exchange_once: operations.compare_exchange_once,
                            observing_results,
                        })
                    }
                    Err(multiplicity) => {
                        return Err(vec![Diagnostic::error(format!(
                            "placed view `{}` field `{}` cannot admit observing compare-exchange for a {multiplicity:?} resident; failure exposes the resident and requires unrestricted copyability",
                            record.synthetic_name,
                            entry.field()
                        ))]);
                    }
                }
            }
            _ => None,
        };
        fields.push(psi_typed_trees::typed_trees::PlacedFieldPlan {
            field_name: entry.field().to_owned(),
            member_identity: schema_field.identity,
            field_symbol: schema_field.symbol,
            accessor_name,
            accessor_type: view_field.type_reference,
            accessor_data_symbol: accessor_data.symbol,
            accessor_targets,
            value_type: schema_field.type_reference,
            access: entry.access().clone(),
            atomic_resident,
        });
    }
    typed
        .placed_view_plans
        .push(psi_typed_trees::typed_trees::PlacedViewPlan {
            data_name: record.synthetic_name.clone(),
            data_symbol,
            policy_name: policy_name.to_owned(),
            policy_symbol,
            policy_plan_machine_symbol,
            schema_name: record.schema_data.clone(),
            schema_symbol,
            placement: placement.clone(),
            fields,
        });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{AtomicPermissions, Multiplicity, observing_result_contracts};

    #[test]
    fn noncopy_residents_remain_rowless_for_try_only_and_reject_observing_axes() {
        for multiplicity in [Multiplicity::Affine, Multiplicity::Linear] {
            let try_only = AtomicPermissions {
                try_exchange: true,
                try_exchange_once: true,
                ..AtomicPermissions::default()
            };
            assert_eq!(
                observing_result_contracts(try_only, multiplicity),
                Ok(None),
                "non-observing permissions retain no observing or encoding authority"
            );

            for observing in [
                AtomicPermissions {
                    compare_exchange: true,
                    ..AtomicPermissions::default()
                },
                AtomicPermissions {
                    compare_exchange_once: true,
                    ..AtomicPermissions::default()
                },
            ] {
                assert_eq!(
                    observing_result_contracts(observing, multiplicity),
                    Err(multiplicity),
                    "each observing failure axis requires an unrestricted resident"
                );
            }
        }
    }
}
