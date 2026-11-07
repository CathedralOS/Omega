//! Check actual native scalar arguments independently of receiver permission.
//!
//! Closed literal values flow through every original origin alternative. Cyclic
//! equations are solved to a bounded fixed point; an unseeded cycle proves
//! nothing, and an external or unsupported value remains unknown. This is an
//! overapproximation across all callers, not permission to select one helpful
//! activation or to infer a handle's successful acquisition.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, ScalarTerm};

use super::windows_handle_calls::{CallTarget, Caller};
use super::windows_handle_contracts::WindowsHandleContract;
use super::windows_handle_inputs::{NativeHandleInput, WindowsHandleInputs};
use super::windows_handle_origins::{HandleOrigins, OriginKind, ScalarSubject};

const MAX_VALUES: usize = 64;
const MAX_COMBINATIONS: usize = 1_024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum ArgumentAtom {
    Integer(IntegerType, IntegerValue),
    Boolean(bool),
    /// The exact admitted GetCurrentProcess execution returns this process's
    /// pseudo-handle. Arbitrary integer bits do not manufacture this custody.
    CurrentProcess,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
enum ArgumentValues {
    #[default]
    Unestablished,
    Finite(BTreeSet<ArgumentAtom>),
    Unknown,
}

impl ArgumentValues {
    fn literal(atom: ArgumentAtom) -> Self {
        Self::Finite(BTreeSet::from([atom]))
    }

    fn join(&mut self, incoming: &Self) -> bool {
        if matches!(self, Self::Unknown) || matches!(incoming, Self::Unestablished) {
            return false;
        }
        if matches!(incoming, Self::Unknown) {
            *self = Self::Unknown;
            return true;
        }
        let Self::Finite(incoming) = incoming else {
            return false;
        };
        if matches!(self, Self::Unestablished) {
            *self = Self::Finite(incoming.clone());
            return true;
        }
        let Self::Finite(retained) = self else {
            return false;
        };
        let before = retained.len();
        retained.extend(incoming);
        if retained.len() > MAX_VALUES {
            *self = Self::Unknown;
            true
        } else {
            before != retained.len()
        }
    }
}

pub(super) struct HandleArgumentValues<'a, 'module> {
    natives: &'a BTreeMap<Caller, NativeHandleInput<'module>>,
    values: BTreeMap<ScalarSubject, ArgumentValues>,
}

/// No public constructor accepts these receipts. The original native call,
/// full scalar operand roots and the independently reconstructed equations
/// supply them. Success, alias transport and lifetime remain separate checks.
pub(super) struct FileAcquisitionArguments {
    pub caller: Caller,
    pub flags: BTreeSet<u32>,
}

pub(super) struct FileDuplicateArguments {
    pub caller: Caller,
    pub source: ScalarSubject,
}

pub(super) struct FileModeArguments {
    pub caller: Caller,
    pub handle: ScalarSubject,
}

pub(super) struct FileTransferArguments {
    pub caller: Caller,
    pub handle: ScalarSubject,
}

pub(super) struct FilePathQueryArguments {
    pub caller: Caller,
    pub capacities: BTreeSet<u32>,
}

pub(super) enum PreparedHandleArguments {
    Acquisition(FileAcquisitionArguments),
    Duplicate(FileDuplicateArguments),
    Mode(FileModeArguments),
    Transfer(FileTransferArguments),
    Path(FilePathQueryArguments),
    /// These contracts have no additional closed-scalar protocol. Their
    /// current handle words and all opaque input roles are still checked by
    /// the execution owner at the original call cut.
    CurrentValues,
}

/// Only protocol constants need backward closed-value equations. Actual handle
/// words, access-dependent buffers and opaque arguments are read and checked at
/// every reached native occurrence by the forward lifetime replay. Requiring a
/// context-free storage origin for them would reject valid captured aliases
/// before their actual caller and selected case can be examined.
pub(super) fn closed_argument_roots(
    input: &NativeHandleInput<'_>,
) -> Result<Vec<ScalarSubject>, String> {
    let ordinals: &[usize] = match input.reviewed.contract {
        WindowsHandleContract::CreateFile => &[2, 3, 5, 6],
        WindowsHandleContract::DuplicateHandle => &[0, 2, 5, 6],
        WindowsHandleContract::QueryFileMode => &[3, 4],
        WindowsHandleContract::ReadFile | WindowsHandleContract::WriteFile => &[3],
        WindowsHandleContract::QueryFinalPath => &[2],
        WindowsHandleContract::NoHandleIntervention
        | WindowsHandleContract::CurrentProcess
        | WindowsHandleContract::CloseHandle => &[],
    };
    ordinals
        .iter()
        .map(|ordinal| {
            input.scalars.get(ordinal).cloned().ok_or_else(|| {
                format!(
                    "native {:?} omits closed protocol argument {ordinal}",
                    input.call.caller
                )
            })
        })
        .collect()
}

/// Solve the complete original scalar equations once. Execution consumes
/// owned argument receipts without keeping a self-reference into its inputs
/// or rebuilding the origin graph at each dynamic call.
pub(super) fn prepare_handle_arguments(
    inputs: &WindowsHandleInputs<'_>,
) -> Result<BTreeMap<Caller, PreparedHandleArguments>, String> {
    let values = HandleArgumentValues::derive(inputs)?;
    inputs
        .natives
        .iter()
        .map(|(caller, input)| {
            let checked = match input.reviewed.contract {
                WindowsHandleContract::CreateFile => {
                    PreparedHandleArguments::Acquisition(values.acquisition(*caller)?)
                }
                WindowsHandleContract::DuplicateHandle => {
                    PreparedHandleArguments::Duplicate(values.duplicate(*caller)?)
                }
                WindowsHandleContract::QueryFileMode => {
                    PreparedHandleArguments::Mode(values.mode(*caller)?)
                }
                WindowsHandleContract::ReadFile | WindowsHandleContract::WriteFile => {
                    PreparedHandleArguments::Transfer(values.transfer(*caller)?)
                }
                WindowsHandleContract::QueryFinalPath => {
                    PreparedHandleArguments::Path(values.path_query(*caller)?)
                }
                WindowsHandleContract::NoHandleIntervention
                | WindowsHandleContract::CurrentProcess
                | WindowsHandleContract::CloseHandle => PreparedHandleArguments::CurrentValues,
            };
            Ok((*caller, checked))
        })
        .collect()
}

impl<'a, 'module> HandleArgumentValues<'a, 'module> {
    pub(super) fn derive(inputs: &'a WindowsHandleInputs<'module>) -> Result<Self, String> {
        Self::from_origins(&inputs.origins, &inputs.natives)
    }

    fn from_origins(
        origins: &HandleOrigins,
        natives: &'a BTreeMap<Caller, NativeHandleInput<'module>>,
    ) -> Result<Self, String> {
        let subjects = origins.equations.keys().cloned().collect::<Vec<_>>();
        let index = subjects
            .iter()
            .cloned()
            .enumerate()
            .map(|(ordinal, subject)| (subject, ordinal))
            .collect::<BTreeMap<_, _>>();
        let mut operands = vec![Vec::new(); subjects.len()];
        let mut dependents = vec![BTreeSet::new(); subjects.len()];
        for (ordinal, subject) in subjects.iter().enumerate() {
            for edge in &origins.equations[subject].sources {
                let source = *index
                    .get(&edge.source)
                    .ok_or("native argument equation omits an original operand")?;
                operands[ordinal].push(source);
                dependents[source].insert(ordinal);
            }
        }
        let mut values = vec![ArgumentValues::Unestablished; subjects.len()];
        let mut pending = (0..subjects.len()).collect::<VecDeque<_>>();
        let mut queued = vec![true; subjects.len()];
        let mut steps = 0usize;
        let mut promoted_unseeded = false;
        loop {
            while let Some(ordinal) = pending.pop_front() {
                queued[ordinal] = false;
                steps += 1;
                if steps > 1_048_576 {
                    return Err(
                        "native argument value equations exceed their finite analysis budget"
                            .into(),
                    );
                }
                let equation = &origins.equations[&subjects[ordinal]];
                let incoming = match &equation.kind {
                OriginKind::Alternatives => {
                    if operands[ordinal].is_empty() {
                        return Err("native argument has an empty original alternative roster".into());
                    }
                    let mut result = ArgumentValues::Unestablished;
                    for &source in &operands[ordinal] { result.join(&values[source]); }
                    result
                }
                OriginKind::Expression(term) => {
                    expression_values(term, &operands[ordinal], &values)
                }
                OriginKind::BoundaryReturn { caller, boundary } => {
                    if natives.get(caller).is_some_and(|input| {
                        matches!(input.call.target, CallTarget::Boundary(actual) if actual == *boundary)
                            && input.call.caller == *caller
                            && input.call.boundary == Some(*boundary)
                            && input.call.normal_return
                            && input.reviewed.contract == WindowsHandleContract::CurrentProcess
                    }) {
                        ArgumentValues::literal(ArgumentAtom::CurrentProcess)
                    } else { ArgumentValues::Unknown }
                }
                OriginKind::NativeOutput
                | OriginKind::IntrinsicReturn
                | OriginKind::ExternalInput
                | OriginKind::OpaqueObservation
                | OriginKind::InactiveCase => ArgumentValues::Unknown,
            };
                if values[ordinal].join(&incoming) {
                    for &dependent in &dependents[ordinal] {
                        if !queued[dependent] {
                            queued[dependent] = true;
                            pending.push_back(dependent);
                        }
                    }
                }
            }
            if promoted_unseeded {
                break;
            }
            // A cycle with no establishment cannot disappear from another
            // node's original alternative roster. Mark it unknown and replay
            // every dependent before any result can become a protocol receipt.
            promoted_unseeded = true;
            for ordinal in 0..values.len() {
                if matches!(values[ordinal], ArgumentValues::Unestablished) {
                    values[ordinal] = ArgumentValues::Unknown;
                    for &dependent in &dependents[ordinal] {
                        if !queued[dependent] {
                            queued[dependent] = true;
                            pending.push_back(dependent);
                        }
                    }
                }
            }
        }
        Ok(Self {
            natives,
            values: subjects.into_iter().zip(values).collect(),
        })
    }

    fn actual<'input>(
        &self,
        input: &'input NativeHandleInput<'_>,
        ordinal: usize,
    ) -> Result<(&'input ScalarSubject, &BTreeSet<ArgumentAtom>), String> {
        let subject = input.scalars.get(&ordinal).ok_or_else(|| {
            format!(
                "native {:?} omits actual scalar argument {ordinal}",
                input.call.caller
            )
        })?;
        match self.values.get(subject) {
            Some(ArgumentValues::Finite(values)) if !values.is_empty() => Ok((subject, values)),
            _ => Err(format!(
                "native {:?} argument {ordinal} has no complete closed value derivation",
                input.call.caller
            )),
        }
    }

    fn words(
        &self,
        input: &NativeHandleInput<'_>,
        ordinal: usize,
        sign: IntegerSign,
        bits: u16,
    ) -> Result<BTreeSet<u128>, String> {
        let (_, values) = self.actual(input, ordinal)?;
        let expected = IntegerType::new(sign, bits).map_err(|reason| {
            format!("native argument has an unsupported physical word type: {reason:?}")
        })?;
        values.iter().map(|value| match value {
            ArgumentAtom::Integer(actual, IntegerValue::Unsigned(value)) if *actual == expected => Ok(*value),
            ArgumentAtom::Integer(actual, IntegerValue::Signed(value)) if *actual == expected && *value >= 0 => u128::try_from(*value).map_err(|_| "native signed scalar cannot be represented by its required nonnegative word".into()),
            _ => Err(format!("native {:?} argument {ordinal} does not retain its required {sign:?}{bits} word", input.call.caller)),
        }).collect()
    }

    fn exact(
        &self,
        input: &NativeHandleInput<'_>,
        ordinal: usize,
        sign: IntegerSign,
        bits: u16,
        expected: u128,
    ) -> Result<(), String> {
        let values = self.words(input, ordinal, sign, bits)?;
        if values == BTreeSet::from([expected]) {
            Ok(())
        } else {
            Err(format!(
                "native {:?} argument {ordinal} is not the required exact value {expected}",
                input.call.caller
            ))
        }
    }

    fn current_process(&self, input: &NativeHandleInput<'_>, ordinal: usize) -> Result<(), String> {
        let (_, values) = self.actual(input, ordinal)?;
        if values == &BTreeSet::from([ArgumentAtom::CurrentProcess]) {
            Ok(())
        } else {
            Err(format!(
                "native {:?} argument {ordinal} is not this process's admitted pseudo-handle",
                input.call.caller
            ))
        }
    }

    pub(super) fn acquisition(&self, caller: Caller) -> Result<FileAcquisitionArguments, String> {
        let input = self
            .natives
            .get(&caller)
            .ok_or("native argument query has no original admitted call")?;
        if input.reviewed.contract != WindowsHandleContract::CreateFile {
            return Err("file acquisition arguments change their reviewed API".into());
        }
        self.exact(input, 3, IntegerSign::Unsigned, 64, 0)?;
        self.exact(input, 6, IntegerSign::Unsigned, 64, 0)?;
        let flags = self
            .words(input, 5, IntegerSign::Unsigned, 32)?
            .into_iter()
            .map(|value| {
                u32::try_from(value)
                    .map_err(|_| "file acquisition flags exceed their physical word".into())
            })
            .collect::<Result<BTreeSet<_>, String>>()?;
        self.words(input, 2, IntegerSign::Unsigned, 32)?
            .into_iter()
            .try_for_each(|value| {
                u32::try_from(value)
                    .map(|_| ())
                    .map_err(|_| "file sharing exceeds its physical word".to_owned())
            })?;
        // Validate the complete sharing word without imposing a new mask.
        // The actual read/write/delete sharing argument stays unchanged.
        Ok(FileAcquisitionArguments {
            caller: input.call.caller,
            flags,
        })
    }

    pub(super) fn duplicate(&self, caller: Caller) -> Result<FileDuplicateArguments, String> {
        let input = self
            .natives
            .get(&caller)
            .ok_or("native argument query has no original admitted call")?;
        if input.reviewed.contract != WindowsHandleContract::DuplicateHandle {
            return Err("file duplicate arguments change their reviewed API".into());
        }
        self.current_process(input, 0)?;
        self.current_process(input, 2)?;
        self.exact(input, 5, IntegerSign::Signed, 32, 0)?;
        // DUPLICATE_SAME_ACCESS, without DUPLICATE_CLOSE_SOURCE. The ignored
        // desired-access operand is not used as evidence of retained access.
        self.exact(input, 6, IntegerSign::Unsigned, 32, 2)?;
        let source = input
            .scalars
            .get(&1)
            .ok_or("duplicate has no original source-handle operand")?
            .clone();
        Ok(FileDuplicateArguments {
            caller: input.call.caller,
            source,
        })
    }

    pub(super) fn mode(&self, caller: Caller) -> Result<FileModeArguments, String> {
        let input = self
            .natives
            .get(&caller)
            .ok_or("native argument query has no original admitted call")?;
        if input.reviewed.contract != WindowsHandleContract::QueryFileMode {
            return Err("file mode arguments change their reviewed API".into());
        }
        self.exact(input, 3, IntegerSign::Unsigned, 32, 4)?;
        self.exact(input, 4, IntegerSign::Unsigned, 32, 16)?;
        let handle = input
            .scalars
            .get(&0)
            .ok_or("file mode query has no original handle operand")?
            .clone();
        Ok(FileModeArguments {
            caller: input.call.caller,
            handle,
        })
    }

    pub(super) fn transfer(&self, caller: Caller) -> Result<FileTransferArguments, String> {
        let input = self
            .natives
            .get(&caller)
            .ok_or("native transfer argument query has no original admitted call")?;
        if !matches!(
            input.reviewed.contract,
            WindowsHandleContract::ReadFile | WindowsHandleContract::WriteFile
        ) {
            return Err("file transfer arguments change their reviewed API".into());
        }
        self.exact(input, 3, IntegerSign::Unsigned, 64, 0)?;
        let handle = input
            .scalars
            .get(&0)
            .ok_or("file transfer has no original handle operand")?
            .clone();
        Ok(FileTransferArguments {
            caller: input.call.caller,
            handle,
        })
    }

    pub(super) fn path_query(&self, caller: Caller) -> Result<FilePathQueryArguments, String> {
        let input = self
            .natives
            .get(&caller)
            .ok_or("native path argument query has no original admitted call")?;
        if input.reviewed.contract != WindowsHandleContract::QueryFinalPath {
            return Err("file path arguments change their reviewed API".into());
        }
        let capacities = self
            .words(input, 2, IntegerSign::Unsigned, 32)?
            .into_iter()
            .map(|value| {
                u32::try_from(value)
                    .map_err(|_| "file path capacity exceeds its physical word".into())
            })
            .collect::<Result<BTreeSet<_>, String>>()?;
        Ok(FilePathQueryArguments {
            caller: input.call.caller,
            capacities,
        })
    }
}

fn expression_values(
    term: &ScalarTerm,
    operands: &[usize],
    values: &[ArgumentValues],
) -> ArgumentValues {
    match term {
        ScalarTerm::Integer { scalar_type, value } => {
            return if scalar_type.admits(*value) {
                ArgumentValues::literal(ArgumentAtom::Integer(*scalar_type, *value))
            } else {
                ArgumentValues::Unknown
            };
        }
        ScalarTerm::Boolean(value) => {
            return ArgumentValues::literal(ArgumentAtom::Boolean(*value));
        }
        _ => {}
    }
    if operands.len() != term.operands().len() || operands.is_empty() {
        return ArgumentValues::Unknown;
    }
    if operands
        .iter()
        .any(|&operand| matches!(values[operand], ArgumentValues::Unknown))
    {
        return ArgumentValues::Unknown;
    }
    if operands
        .iter()
        .any(|&operand| matches!(values[operand], ArgumentValues::Unestablished))
    {
        return ArgumentValues::Unestablished;
    }
    let mut combinations = vec![Vec::new()];
    for &operand in operands {
        let ArgumentValues::Finite(alternatives) = &values[operand] else {
            return ArgumentValues::Unknown;
        };
        if combinations
            .len()
            .checked_mul(alternatives.len())
            .is_none_or(|count| count > MAX_COMBINATIONS)
        {
            return ArgumentValues::Unknown;
        }
        let mut next = Vec::new();
        for prefix in combinations {
            for &alternative in alternatives {
                let mut combination = prefix.clone();
                combination.push(alternative);
                next.push(combination);
            }
        }
        combinations = next;
    }
    let mut result = BTreeSet::new();
    for combination in combinations {
        let mut instantiated = term.clone();
        for (operand, value) in instantiated.operands_mut().into_iter().zip(combination) {
            *operand = match value {
                ArgumentAtom::Integer(scalar_type, value) => {
                    ScalarTerm::Integer { scalar_type, value }
                }
                ArgumentAtom::Boolean(value) => ScalarTerm::Boolean(value),
                ArgumentAtom::CurrentProcess => return ArgumentValues::Unknown,
            };
        }
        let folded = if let Some((scalar_type, value)) = instantiated.integer_value() {
            ArgumentAtom::Integer(scalar_type, value)
        } else if let Some(value) = instantiated.boolean_value() {
            ArgumentAtom::Boolean(value)
        } else {
            return ArgumentValues::Unknown;
        };
        result.insert(folded);
        if result.len() > MAX_VALUES {
            return ArgumentValues::Unknown;
        }
    }
    if result.is_empty() {
        ArgumentValues::Unknown
    } else {
        ArgumentValues::Finite(result)
    }
}

#[cfg(test)]
mod tests {
    use super::super::windows_handle_origins::{OriginEdge, OriginEdgeRole, OriginEquation};
    use super::*;
    use semantic_vocabulary::{MachineId, ValueId};

    fn subject(id: u64) -> ScalarSubject {
        ScalarSubject::Value {
            machine: MachineId::new(1).unwrap(),
            value: ValueId::new(id).unwrap(),
        }
    }

    fn alternatives(sources: &[u64]) -> OriginEquation {
        OriginEquation {
            kind: OriginKind::Alternatives,
            sources: sources
                .iter()
                .map(|id| OriginEdge {
                    role: OriginEdgeRole::Operand,
                    source: subject(*id),
                })
                .collect(),
        }
    }

    fn zero() -> OriginEquation {
        OriginEquation {
            kind: OriginKind::Expression(ScalarTerm::Integer {
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 32).unwrap(),
                value: IntegerValue::Unsigned(0),
            }),
            sources: Vec::new(),
        }
    }

    #[test]
    fn arguments_retain_all_alternatives_and_need_a_real_seed_for_cycles() {
        let origins = HandleOrigins {
            equations: BTreeMap::from([
                (subject(1), zero()),
                (subject(2), alternatives(&[1, 2])),
                (
                    subject(3),
                    OriginEquation {
                        kind: OriginKind::ExternalInput,
                        sources: Vec::new(),
                    },
                ),
                (subject(4), alternatives(&[2, 3])),
                (subject(5), alternatives(&[5])),
                (subject(6), alternatives(&[1, 5])),
            ]),
        };
        let natives = BTreeMap::new();
        let analysis = HandleArgumentValues::from_origins(&origins, &natives).unwrap();
        assert_eq!(
            analysis.values[&subject(2)],
            ArgumentValues::literal(ArgumentAtom::Integer(
                IntegerType::new(IntegerSign::Unsigned, 32).unwrap(),
                IntegerValue::Unsigned(0)
            ))
        );
        assert_eq!(analysis.values[&subject(4)], ArgumentValues::Unknown);
        assert_eq!(analysis.values[&subject(5)], ArgumentValues::Unknown);
        assert_eq!(analysis.values[&subject(6)], ArgumentValues::Unknown);
    }

    #[test]
    fn changing_cyclic_words_widen_to_unknown_instead_of_selecting_an_early_value() {
        let scalar_type = IntegerType::new(IntegerSign::Unsigned, 32).unwrap();
        let expression = ScalarTerm::WrappingIntegerAdd {
            scalar_type,
            left: Box::new(ScalarTerm::value(
                ValueId::new(2).unwrap(),
                semantic_vocabulary::ScalarType::Integer(scalar_type),
            )),
            right: Box::new(ScalarTerm::Integer {
                scalar_type,
                value: IntegerValue::Unsigned(1),
            }),
        };
        let origins = HandleOrigins {
            equations: BTreeMap::from([
                (subject(1), zero()),
                (subject(2), alternatives(&[1, 3])),
                (
                    subject(3),
                    OriginEquation {
                        kind: OriginKind::Expression(expression),
                        sources: vec![
                            OriginEdge {
                                role: OriginEdgeRole::Operand,
                                source: subject(2),
                            },
                            OriginEdge {
                                role: OriginEdgeRole::Operand,
                                source: subject(4),
                            },
                        ],
                    },
                ),
                (
                    subject(4),
                    OriginEquation {
                        kind: OriginKind::Expression(ScalarTerm::Integer {
                            scalar_type,
                            value: IntegerValue::Unsigned(1),
                        }),
                        sources: Vec::new(),
                    },
                ),
            ]),
        };
        let natives = BTreeMap::new();
        let analysis = HandleArgumentValues::from_origins(&origins, &natives).unwrap();
        assert_eq!(analysis.values[&subject(2)], ArgumentValues::Unknown);
        assert_eq!(analysis.values[&subject(3)], ArgumentValues::Unknown);
    }
}
