//! Necessary scalar guards from every actual incoming control-flow path.
//!
//! A query never inserts its goal. Only an actual selected edge establishes a
//! Boolean condition. Joins intersect conditions; new scalar instances retire
//! old ones, and simultaneous edge bindings transport only captured values.
//! These mathematical conditions do not establish a native handle's origin.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use super::windows_handle_values::LocalScalarExpressions;
use semantic_vocabulary::{IdTable, ScalarTerm, ScalarType, ValueId};
use terminal_psi::{TerminalMachine, Terminator};
use terminal_semantics::descriptors::descriptor_lifetimes::DescriptorPosition;
use terminal_semantics::descriptors::descriptor_results::{
    DescriptorMachine, ScalarCaptureAnalysis, ScalarCaptureFailure,
};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct ScalarConditions {
    facts: BTreeMap<ScalarTerm, bool>,
}

impl ScalarConditions {
    fn require(&mut self, mut term: ScalarTerm, mut polarity: bool) -> bool {
        loop {
            match term {
                ScalarTerm::Boolean(value) => return value == polarity,
                ScalarTerm::BooleanNot { operand } => {
                    term = *operand;
                    polarity = !polarity;
                }
                ScalarTerm::BooleanEqual { left, right } => match (&*left, &*right) {
                    (ScalarTerm::Boolean(value), _) => {
                        polarity = polarity == *value;
                        term = *right;
                    }
                    (_, ScalarTerm::Boolean(value)) => {
                        polarity = polarity == *value;
                        term = *left;
                    }
                    _ => {
                        term = ScalarTerm::BooleanEqual { left, right };
                        break;
                    }
                },
                _ => break,
            }
        }
        // Live structural observations need their own version proof. The
        // ordinary guard path uses immutable read SSA binders instead.
        if has_storage_observation(&term) {
            return true;
        }
        match self.facts.get(&term) {
            Some(previous) => *previous == polarity,
            None => {
                self.facts.insert(term, polarity);
                true
            }
        }
    }

    pub(super) fn proves(&self, term: &ScalarTerm, polarity: bool) -> bool {
        let mut opposite = self.clone();
        !opposite.require(term.clone(), !polarity)
    }

    fn retire(&mut self, definitions: &[ValueId]) {
        let mut definitions = definitions.to_vec();
        definitions.sort_unstable();
        definitions.dedup();
        self.facts
            .retain(|term, _| !mentions_any(term, &definitions));
    }

    fn intersect(&mut self, incoming: &Self) -> bool {
        let before = self.facts.len();
        self.facts
            .retain(|term, polarity| incoming.facts.get(term) == Some(polarity));
        self.facts.len() != before
    }

    fn arrival(
        &self,
        parameters: &[ValueId],
        bindings: &[(ValueId, ScalarTerm)],
    ) -> Result<Self, String> {
        let mut result = self.clone();
        result.retire(parameters);
        let mut renamed = Vec::new();
        for (parameter, actual) in bindings {
            if let ScalarTerm::Value { id, scalar_type } = actual {
                renamed.push((
                    *id,
                    Some(ScalarTerm::Value {
                        id: *parameter,
                        scalar_type: *scalar_type,
                    }),
                ));
            }
        }
        // A repeated actual keeps the first target, as the previous ordered
        // map did. Other copies remain conservative rather than invented.
        renamed.sort_by_key(|(actual, _)| *actual);
        renamed.dedup_by_key(|(actual, _)| *actual);
        let renamed = IdTable::from_rows_with_gaps(renamed, 65_536)
            .map_err(|reason| format!("guard arrival namespace is invalid: {reason:?}"))?;
        let mut overwritten = parameters
            .iter()
            .filter(|parameter| renamed.get(**parameter).is_none_or(Option::is_none))
            .copied()
            .collect::<Vec<_>>();
        overwritten.sort_unstable();
        overwritten.dedup();
        for (term, &polarity) in &self.facts {
            // Capture all right-hand sides before installing any formal. Old
            // target instances with no exact actual transport are forgotten.
            if mentions_any(term, &overwritten) {
                continue;
            }
            let mut transported = term.clone();
            rename_values(&mut transported, &renamed);
            result.require(transported, polarity);
        }
        Ok(result)
    }
}

pub(super) struct HandleFlowConditions {
    pub snapshots: BTreeMap<DescriptorPosition, ScalarConditions>,
    pub excluded_edges: BTreeSet<(DescriptorPosition, usize)>,
}

impl HandleFlowConditions {
    pub(super) fn derive(
        machine: &DescriptorMachine,
        body: &TerminalMachine,
        expressions: &LocalScalarExpressions,
        captures: &mut ScalarCaptureAnalysis,
        scalar_captures: &terminal_semantics::descriptors::descriptor_results::TerminalScalarCaptures,
    ) -> Result<Self, String> {
        if machine.id != body.id
            || expressions.identity != machine.id
            || captures.identity() != machine.id
        {
            return Err("native guard contexts belong to different canonical machines".into());
        }
        if !machine.flow.contains_key(&machine.entry) {
            return Err("native handle guard flow has no canonical entry".into());
        }
        let mut active_expansion = expressions.definitions.map(|_, _| false);
        let mut condition_sites = BTreeMap::new();
        for block in &body.blocks {
            if let Terminator::Conditional {
                condition,
                when_true,
                when_false,
            } = &block.terminator
            {
                let position = (
                    block.id,
                    u32::try_from(block.operations.len())
                        .map_err(|_| "conditional ordinal exceeds its canonical position")?,
                );
                let step = machine
                    .flow
                    .get(&position)
                    .ok_or("native guard lost its canonical conditional cut")?;
                if step.successors.len() != 2
                    || step.successors[0].target != (when_true.target, 0)
                    || step.successors[1].target != (when_false.target, 0)
                {
                    return Err(
                        "native guard substitutes the canonical conditional successor order".into(),
                    );
                }
                condition_sites.insert(position, *condition);
            }
        }
        let mut result = Self {
            snapshots: BTreeMap::from([(machine.entry, ScalarConditions::default())]),
            excluded_edges: BTreeSet::new(),
        };
        let mut pending = VecDeque::from([machine.entry]);
        let mut queued = BTreeSet::from([machine.entry]);
        let mut steps = 0usize;
        while let Some(position) = pending.pop_front() {
            queued.remove(&position);
            steps += 1;
            if steps > 65_536 {
                return Err("native handle guard flow exceeds its finite analysis budget".into());
            }
            let step = machine
                .flow
                .get(&position)
                .ok_or("native handle guard flow lost a canonical position")?;
            let mut after = result.snapshots[&position].clone();
            if let Some(definitions) = machine.payload_scalar_definitions.get(&position) {
                after.retire(definitions);
            }
            for (ordinal, edge) in step.successors.iter().enumerate() {
                if !machine.flow.contains_key(&edge.target) {
                    return Err("native handle guard edge has no canonical destination".into());
                }
                let mut arrival = after.clone();
                if let Some(condition) = condition_sites.get(&position) {
                    let condition = expand(
                        &ScalarTerm::value(*condition, ScalarType::Boolean),
                        position,
                        expressions,
                        captures,
                        &mut active_expansion,
                    )?;
                    if !arrival.require(condition, ordinal == 0) {
                        result.excluded_edges.insert((position, ordinal));
                        continue;
                    }
                }
                result.excluded_edges.remove(&(position, ordinal));
                if edge.target.1 == 0 {
                    let parameters = machine
                        .payload_scalar_parameters
                        .get(&edge.target.0)
                        .map(Vec::as_slice)
                        .unwrap_or(&[]);
                    let bindings = scalar_captures
                        .arrivals
                        .get(&position)
                        .and_then(|edges| edges.get(ordinal))
                        .map(Vec::as_slice)
                        .unwrap_or(&[]);
                    let values = bindings
                        .iter()
                        .filter_map(|(parameter, operand)| {
                            operand.value_term().map(|actual| (*parameter, actual))
                        })
                        .collect::<Vec<_>>();
                    arrival = arrival.arrival(parameters, &values)?;
                }
                let changed = match result.snapshots.get_mut(&edge.target) {
                    Some(previous) => previous.intersect(&arrival),
                    None => {
                        result.snapshots.insert(edge.target, arrival);
                        true
                    }
                };
                if changed && queued.insert(edge.target) {
                    pending.push_back(edge.target);
                }
            }
        }
        Ok(result)
    }
}

fn expand(
    term: &ScalarTerm,
    observation: DescriptorPosition,
    expressions: &LocalScalarExpressions,
    captures: &mut ScalarCaptureAnalysis,
    active: &mut IdTable<ValueId, bool>,
) -> Result<ScalarTerm, String> {
    fn visit(
        term: &mut ScalarTerm,
        observation: DescriptorPosition,
        expressions: &LocalScalarExpressions,
        captures: &mut ScalarCaptureAnalysis,
        remaining: &mut usize,
        active: &mut IdTable<ValueId, bool>,
    ) -> Result<(), String> {
        *remaining = remaining
            .checked_sub(1)
            .ok_or("native handle guard expression exceeds its finite budget")?;
        if let ScalarTerm::Value { id, scalar_type } = term {
            let identity = *id;
            if !expressions.available_at(identity, observation) {
                return Err(
                    "native guard scalar definition does not dominate its actual use".into(),
                );
            }
            let (actual, definition) = expressions
                .definitions
                .get(identity)
                .and_then(Option::as_ref)
                .ok_or("native guard scalar has no canonical declaration")?;
            if *actual != *scalar_type {
                return Err("native guard scalar changes its declared carrier".into());
            }
            if let Some(definition) = definition {
                let source = *expressions
                    .producers
                    .get(identity)
                    .and_then(Option::as_ref)
                    .ok_or("computed guard has no real producing operation")?;
                let mut operands = Vec::new();
                collect_values(definition, &mut operands);
                operands.sort_unstable();
                operands.dedup();
                // Keep the saved boolean opaque if one of its captured operands
                // renewed afterwards. Re-expanding against a new native status
                // would otherwise change the meaning of this old snapshot.
                match captures.require(source, observation, &operands) {
                    Ok(()) => {}
                    Err(ScalarCaptureFailure::MissingOrRenewed) => return Ok(()),
                    Err(reason) => {
                        return Err(format!("native guard capture analysis refused: {reason:?}"));
                    }
                }
                if std::mem::replace(
                    active
                        .get_mut(identity)
                        .ok_or("guard operand is outside its prepared value namespace")?,
                    true,
                ) {
                    return Err("native guard has a cyclic scalar definition".into());
                }
                *term = definition.clone();
                visit(term, observation, expressions, captures, remaining, active)?;
                *active
                    .get_mut(identity)
                    .expect("expanded guard binder was validated") = false;
            }
        } else {
            for operand in term.operands_mut() {
                visit(
                    operand,
                    observation,
                    expressions,
                    captures,
                    remaining,
                    active,
                )?;
            }
        }
        Ok(())
    }
    let mut term = term.clone();
    visit(
        &mut term,
        observation,
        expressions,
        captures,
        &mut 4096,
        active,
    )?;
    Ok(term)
}

fn collect_values(term: &ScalarTerm, values: &mut Vec<ValueId>) {
    if let ScalarTerm::Value { id, .. } = term {
        values.push(*id);
    }
    for operand in term.operands() {
        collect_values(operand, values);
    }
}

fn mentions_any(term: &ScalarTerm, values: &[ValueId]) -> bool {
    matches!(term, ScalarTerm::Value { id, .. } if values.binary_search(id).is_ok())
        || term
            .operands()
            .into_iter()
            .any(|operand| mentions_any(operand, values))
}

fn has_storage_observation(term: &ScalarTerm) -> bool {
    matches!(
        term,
        ScalarTerm::StructuralRead { .. }
            | ScalarTerm::BooleanField { .. }
            | ScalarTerm::IntegerField { .. }
            | ScalarTerm::IeeeFloatField { .. }
            | ScalarTerm::ViewExtent { .. }
            | ScalarTerm::ByteSequencePredicate { .. }
    ) || term.operands().into_iter().any(has_storage_observation)
}

fn rename_values(term: &mut ScalarTerm, values: &IdTable<ValueId, Option<ScalarTerm>>) {
    if let ScalarTerm::Value { id, .. } = term {
        if let Some(Some(value)) = values.get(*id) {
            *term = value.clone();
        }
    } else {
        for operand in term.operands_mut() {
            rename_values(operand, values);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};

    fn value(id: u64) -> ScalarTerm {
        ScalarTerm::value(
            ValueId::new(id).unwrap(),
            ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap()),
        )
    }

    fn zero_test(id: u64) -> ScalarTerm {
        let scalar_type = IntegerType::new(IntegerSign::Signed, 32).unwrap();
        ScalarTerm::IntegerEqual {
            scalar_type,
            left: Box::new(value(id)),
            right: Box::new(ScalarTerm::Integer {
                scalar_type,
                value: IntegerValue::Signed(0),
            }),
        }
    }

    #[test]
    fn guards_follow_simultaneous_actuals_and_expire_on_renewal_or_join() {
        let first = ValueId::new(1).unwrap();
        let second = ValueId::new(2).unwrap();
        let mut old = ScalarConditions::default();
        assert!(old.require(zero_test(1), false));
        assert!(old.proves(&zero_test(1), false));
        let mut swapped = old
            .arrival(&[first, second], &[(first, value(2)), (second, value(1))])
            .unwrap();
        assert!(swapped.proves(&zero_test(2), false));
        assert!(!swapped.proves(&zero_test(1), false));
        swapped.retire(&[second]);
        assert!(!swapped.proves(&zero_test(2), false));
        assert!(old.intersect(&ScalarConditions::default()));
        assert!(!old.proves(&zero_test(1), false));
        assert!(old.proves(&ScalarTerm::Boolean(true), true));
        assert!(!old.proves(&ScalarTerm::Boolean(false), true));
    }
}
