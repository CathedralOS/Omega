//! Instantiate control occurrences before joining their private loan owners.
//!
//! An authored reader can receive a direct shared descriptor or a child of an
//! exclusive descriptor. Those arrivals have different live owner rosters.
//! Retain that distinction until the authored graph stops carrying the loans;
//! an ordinary equal frontier then rejoins without a runtime discriminator.
//! Source identities and checked operand schedules remain untouched.
use super::super::LoweringError;
use checked_trees::{
    CheckedComposedUnitControlMachinePlan as Plan, CheckedStructuralAccess,
    CheckedStructuralControlSuccessorPlan as Edge,
    CheckedStructuralControlTransferSourcePlan as Source,
    CheckedUnitStructuralArgumentSourcePlan as Root,
};
use typed_trees::type_identity::TypeIdentityId;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Owner {
    /// Source occurrence, not an emitted place or a new source declaration.
    birth: (usize, u32, u32),
    carrier: TypeIdentityId,
    parent: Option<Box<Owner>>,
}

impl Owner {
    fn contains_birth(&self, birth: (usize, u32, u32)) -> bool {
        self.birth == birth
            || self
                .parent
                .as_ref()
                .is_some_and(|p| p.contains_birth(birth))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Frontier {
    source: usize,
    parameters: Vec<Option<Owner>>,
}

pub(in crate::unit::attached_unit::composed_control) fn prepare(
    source: &Plan,
    values: &checked_trees::CheckedStructuralValuePlans,
) -> Result<Plan, LoweringError> {
    let Some(entry) = source.states.first() else {
        return Err(LoweringError::Unsupported("custody frontier has no entry"));
    };
    let mut frontiers = vec![Frontier {
        source: 0,
        parameters: vec![None; entry.structural_parameters.len()],
    }];
    let mut states = Vec::new();
    let mut cursor = 0;
    while cursor < frontiers.len() {
        let frontier = frontiers[cursor].clone();
        let mut state = source.states[frontier.source].clone();
        for edge in state.terminator.successors_mut() {
            let target = source
                .successor_position(edge)
                .ok_or(LoweringError::Unsupported(
                    "custody frontier successor disappeared",
                ))?;
            let incoming = arrive(source, values, &frontier, target, edge)?;
            let occurrence = match frontiers
                .iter()
                .position(|candidate| candidate == &incoming)
            {
                Some(position) => position,
                None => {
                    let position = frontiers.len();
                    frontiers.push(incoming);
                    position
                }
            };
            edge.target_occurrence = Some(u32::try_from(occurrence).map_err(|_| {
                LoweringError::Unsupported("custody frontier occurrence exceeds u32")
            })?);
        }
        states.push(state);
        cursor += 1;
    }
    let mut prepared = source.clone();
    prepared.states = states;
    Ok(prepared)
}

fn arrive(
    plan: &Plan,
    values: &checked_trees::CheckedStructuralValuePlans,
    source: &Frontier,
    target: usize,
    edge: &Edge,
) -> Result<Frontier, LoweringError> {
    let mut parameters = vec![None; plan.states[target].structural_parameters.len()];
    for transfer in &edge.transfers {
        let index = transfer.target_parameter_index as usize;
        let parameter = &plan.states[target].structural_parameters[index];
        if parameter.access == CheckedStructuralAccess::Owned {
            continue;
        }
        let subject = super::result_custody::loan_subject(&transfer.source, values);
        let parent_index = subject.and_then(|(subject, _)| match subject.source {
            Root::Parameter { parameter_index } => Some(parameter_index as usize),
            _ => None,
        });
        let (forwarded, carrier) = match &transfer.source {
            Source::Parameter { index } => (Some(*index as usize), None),
            Source::ProjectedLoan {
                subject,
                carrier_type_identity,
                ..
            } => {
                let forwarded = parent_index.filter(|index| {
                    subject.path.is_empty()
                        && plan.states[source.source].structural_parameters[*index].access
                            == parameter.access
                });
                (
                    forwarded,
                    carrier_type_identity
                        .is_valid()
                        .then_some(*carrier_type_identity),
                )
            }
            Source::EstablishedValue { establishment, .. } if subject.is_some() => (
                None,
                establishment
                    .structural_result_binding()
                    .map(|result| result.type_identity),
            ),
            _ => (None, None),
        };
        parameters[index] = if let Some(index) = forwarded {
            source.parameters[index].clone()
        } else if let Some(carrier) = carrier.filter(|carrier| carrier.is_valid()) {
            let parent = parent_index.and_then(|index| source.parameters[index].clone());
            let birth = (
                source.source,
                edge.statement_ordinal,
                transfer.target_parameter_index,
            );
            if parent
                .as_ref()
                .is_some_and(|parent| parent.contains_birth(birth))
            {
                return Err(LoweringError::Unsupported(
                    "captured state loan has an unbounded parent lifetime",
                ));
            }
            Some(Owner {
                birth,
                carrier,
                parent: parent.map(Box::new),
            })
        } else {
            None
        };
    }
    Ok(Frontier {
        source: target,
        parameters,
    })
}
