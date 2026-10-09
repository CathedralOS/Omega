//! Current custody follows the retained value graph into its receiving leaf.
//! Possible transfers fence disposal of the original slot. Definite transfers
//! additionally prove that every selected alternative moved that slot.

use super::*;
use checked_trees::{BorrowLoanOwnerSegment as OwnerSegment, CheckedStructuralValueHandle};
use typed_trees::data::DataMember;

type Path = Vec<CheckedUnitStructuralPathSegment>;

pub(super) struct Transfer {
    pub(super) source: Path,
    pub(super) destination: Destination,
}

pub(super) enum Destination {
    Holder(CheckedUnitStructuralResultBindingPlan, Vec<OwnerSegment>),
    /// The call consumes this source. Its returned descriptor occurrences
    /// retain exact supplying-loan identities independently of result nesting.
    Call {
        statement_index: u32,
    },
    Unknown,
}

#[derive(Default)]
pub(super) struct Transfers {
    pub(super) possible: Vec<Transfer>,
    pub(super) definite: Vec<Path>,
}

impl Transfers {
    fn without_receiver(mut self) -> Self {
        for transfer in &mut self.possible {
            transfer.destination = Destination::Unknown;
        }
        self
    }

    fn append(&mut self, other: Self) {
        self.possible.extend(other.possible);
        self.definite.extend(other.definite);
    }
}

struct Query<'a> {
    program: &'a dyn ::validation::CheckingInput,
    facts: &'a CheckFacts,
    results: &'a Results,
    binding: u32,
    operations: &'a [CheckedUnitEffectOperationPlan],
}

pub(super) fn transfers(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    results: &Results,
    binding: u32,
    operations: &[CheckedUnitEffectOperationPlan],
) -> Option<Transfers> {
    let query = Query {
        program,
        facts,
        results,
        binding,
        operations,
    };
    let mut output = Transfers::default();
    for operation in operations {
        output.append(query.operation(operation, &mut Vec::new())?);
    }
    Some(output)
}

impl Query<'_> {
    fn operation(
        &self,
        operation: &CheckedUnitEffectOperationPlan,
        active: &mut Vec<CheckedStructuralValueHandle>,
    ) -> Option<Transfers> {
        let mut output = Transfers::default();
        // Calls and stores transfer custody, but without a retained receiving
        // local they do not prove when that custody ends.
        if let Some(call) = operation.call() {
            output.append(self.call(call));
        }
        if let Some(value) = operation.installed_value()
            && self.belongs(value)
        {
            output.append(self.leaf(value.path.clone(), Destination::Unknown))
        }
        if let CheckedUnitEffectOperationPlan::EstablishStructuralValue { result, value, .. } =
            operation
        {
            output.append(self.value(*value, &[], result, &[], active)?);
        }
        Some(output)
    }
    fn belongs(&self, argument: &CheckedUnitStructuralArgumentPlan) -> bool {
        argument.access == CheckedStructuralAccess::Owned
            && match argument.source {
                CheckedUnitStructuralArgumentSourcePlan::StructuralResult { binding_ordinal } => {
                    binding_ordinal == self.binding
                }
                CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol } => {
                    self.results.iter().any(|(result, root)| {
                        result.binding_ordinal == self.binding
                            && *root == facts::PlaceRoot::Symbol(symbol)
                    })
                }
                _ => false,
            }
    }

    fn leaf(&self, source: Path, destination: Destination) -> Transfers {
        Transfers {
            definite: vec![source.clone()],
            possible: vec![Transfer {
                source,
                destination,
            }],
        }
    }

    fn call(&self, call: &checked_trees::CheckedUnitCallPlan) -> Transfers {
        let mut output = Transfers::default();
        for argument in &call.structural_arguments {
            if self.belongs(argument) {
                let destination = if call.structural_result().is_some() {
                    Destination::Call {
                        statement_index: call.coordinate.statement_index,
                    }
                } else {
                    Destination::Unknown
                };
                output.append(self.leaf(argument.path.clone(), destination));
            }
        }
        output
    }

    fn value(
        &self,
        value: CheckedStructuralValueHandle,
        selection: &[CheckedUnitStructuralPathSegment],
        result: &CheckedUnitStructuralResultBindingPlan,
        destination: &[OwnerSegment],
        active: &mut Vec<CheckedStructuralValueHandle>,
    ) -> Option<Transfers> {
        let values = &self.facts.values.structural_values;
        if !values.nodes.is_valid(value) || active.contains(&value) {
            return None;
        }
        active.push(value);
        let output = self.value_kind(
            &values.nodes.get(value).kind,
            selection,
            result,
            destination,
            active,
        );
        active.pop();
        output
    }

    fn value_kind(
        &self,
        kind: &CheckedStructuralValueKind,
        selection: &[CheckedUnitStructuralPathSegment],
        result: &CheckedUnitStructuralResultBindingPlan,
        destination: &[OwnerSegment],
        active: &mut Vec<CheckedStructuralValueHandle>,
    ) -> Option<Transfers> {
        let values = &self.facts.values.structural_values;
        let mut output = Transfers::default();
        match kind {
            CheckedStructuralValueKind::Place(argument) if self.belongs(argument) => {
                let mut path = argument.path.clone();
                path.extend_from_slice(selection);
                return Some(self.leaf(
                    path,
                    Destination::Holder(result.clone(), destination.to_vec()),
                ));
            }
            CheckedStructuralValueKind::Call { source_call } => {
                let mut matching = self
                    .operations
                    .iter()
                    .flat_map(|operation| operation.with_value_calls())
                    .filter_map(|operation| match operation {
                        CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                            calls, ..
                        } => Some(calls.as_slice()),
                        _ => None,
                    })
                    .flatten()
                    .filter(|call| {
                        matches!(values.nodes.get(call.value).kind,
                            CheckedStructuralValueKind::Call { source_call: captured }
                                if captured == *source_call)
                    });
                let call = matching.next()?;
                if matching.next().is_some() {
                    return None;
                }
                // The graph's dispatch intersects definite moves across its
                // arms. Flattening operand calls here instead would assert a
                // conditional owned argument moved on every path.
                for operand in call.operands().iter() {
                    output.append(self.operation(operand, active)?);
                }
                output.append(self.call(call.operation().call()?));
                return Some(output);
            }
            CheckedStructuralValueKind::Projection { source, path, .. } => {
                let mut selected = path.clone();
                selected.extend_from_slice(selection);
                return self.value(*source, &selected, result, destination, active);
            }
            CheckedStructuralValueKind::Record {
                data_symbol,
                fields,
            }
            | CheckedStructuralValueKind::StructuralCase {
                data_symbol,
                fields,
                ..
            } => {
                let data = self.program.data_definition_by_symbol(*data_symbol)?;
                // Common fields and payload fields have independent path
                // namespaces even when a case establishes both in one roster.
                let mut declared = self
                    .program
                    .data_members(data)
                    .iter()
                    .filter_map(|member| match member {
                        DataMember::Field(field) => Some((field, None)),
                        DataMember::Variant(_) => None,
                    })
                    .collect::<Vec<_>>();
                if let CheckedStructuralValueKind::StructuralCase { case, .. } = kind {
                    let variant =
                        self.program
                            .data_members(data)
                            .iter()
                            .find_map(|member| match member {
                                DataMember::Variant(variant) if variant.symbol == *case => {
                                    Some(variant)
                                }
                                _ => None,
                            })?;
                    declared.extend(
                        self.program
                            .data_payload_fields(variant)
                            .iter()
                            .map(|field| (field, Some(variant))),
                    );
                }
                for field in values.record_fields.span(*fields)? {
                    let (declaration, case) = declared
                        .iter()
                        .find(|(candidate, _)| candidate.symbol == field.field)?;
                    let mut relative = Vec::new();
                    let mut owner_path = Vec::new();
                    if let Some(case) = case {
                        relative.push(CheckedUnitStructuralPathSegment::Case(case.path_identity()));
                        owner_path.push(OwnerSegment::Case(case.symbol));
                    }
                    relative.push(CheckedUnitStructuralPathSegment::Field(
                        declaration.path_identity(),
                    ));
                    owner_path.push(OwnerSegment::Field(field.field));
                    let checked_trees::CheckedStructuralRecordFieldValue::Structural(child) =
                        field.value
                    else {
                        continue;
                    };
                    if selection.starts_with(&relative) {
                        // The result selects within this field: its container
                        // prefix no longer belongs to the receiving value.
                        output.append(self.value(
                            child,
                            &selection[relative.len()..],
                            result,
                            destination,
                            active,
                        )?);
                    } else if relative.starts_with(selection) {
                        // Whole construction or a selected case payload keeps
                        // the remaining field path inside the receiving value.
                        let mut path = destination.to_vec();
                        path.extend_from_slice(&owner_path[selection.len()..]);
                        output.append(self.value(child, &[], result, &path, active)?);
                    } else {
                        // Construction consumed the unselected sibling too;
                        // its custody is absent from the surviving projection.
                        output.append(
                            self.value(child, &[], result, &[], active)?
                                .without_receiver(),
                        );
                    }
                }
            }
            CheckedStructuralValueKind::FixedArray { elements } => {
                if let Some((CheckedUnitStructuralPathSegment::FixedIndex(selected), suffix)) =
                    selection.split_first()
                {
                    let selected = usize::try_from(*selected).ok()?;
                    elements.get(selected)?;
                    for (index, child) in elements.iter().enumerate() {
                        let transferred = if index == selected {
                            self.value(*child, suffix, result, destination, active)?
                        } else {
                            self.value(*child, &[], result, &[], active)?
                                .without_receiver()
                        };
                        output.append(transferred);
                    }
                    return Some(output);
                }
                if !selection.is_empty() {
                    return None;
                }
                for (index, child) in elements.iter().enumerate() {
                    let mut path = destination.to_vec();
                    path.push(OwnerSegment::FixedIndex(index));
                    output.append(self.value(*child, &[], result, &path, active)?);
                }
            }
            CheckedStructuralValueKind::Dispatch { arms, .. } => {
                let alternatives = values
                    .dispatch_arms
                    .span(*arms)?
                    .iter()
                    .map(|arm| self.value(arm.value, selection, result, destination, active))
                    .collect::<Option<Vec<_>>>()?;
                // Keep only source regions transferred on every arm. Taking
                // the union here would end a parent on an unselected move.
                output.definite = alternatives
                    .iter()
                    .flat_map(|arm| &arm.definite)
                    .filter(|path| {
                        alternatives
                            .iter()
                            .all(|arm| arm.definite.iter().any(|covered| path.starts_with(covered)))
                    })
                    .cloned()
                    .collect();
                output.possible = alternatives
                    .into_iter()
                    .flat_map(|arm| arm.possible)
                    .collect();
            }
            // Captures/copies keep their source custody; call arguments are
            // already represented by the retained operation call schedule.
            _ => {}
        }
        Some(output)
    }
}
