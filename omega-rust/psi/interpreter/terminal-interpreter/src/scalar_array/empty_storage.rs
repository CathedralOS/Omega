//! Initialized zero-leaf scalar arrays retain their typed home without one
//! empty byte allocation per logical element. Sparse vacancy rows preserve
//! partial retirement: an ancestor's initialization cannot revive a removed
//! child. Constructor staging and retirement fund every new row before commit.

use crate::byte_sequences::ByteSequenceView;
use crate::errors::{TerminalInterpretError, malformed};
use crate::program::ProgramDeclarations;
use crate::runtime_rows::RuntimeRows;
use crate::values::StructuralRuntimePlace;
use evaluation_accounting::EvaluationInvocationAccount;
use semantic_vocabulary::StructuralTypeId;
use terminal_psi::{StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape};

#[derive(Clone, Copy)]
pub(crate) struct EmptyArrayState {
    structural_type: StructuralTypeId,
    initialized: bool,
}

pub(crate) struct EmptyArrayRetirement {
    root: StructuralRuntimePlace,
    vacancy: RuntimeRows<StructuralRuntimePlace, EmptyArrayState>,
}

impl EmptyArrayRetirement {
    pub(crate) fn root(&self) -> &StructuralRuntimePlace {
        &self.root
    }
}

pub(crate) struct EmptyArrayStorage {
    pub(crate) rows: RuntimeRows<StructuralRuntimePlace, EmptyArrayState>,
}

impl EmptyArrayStorage {
    pub(crate) fn new(account: Option<&EvaluationInvocationAccount>) -> Self {
        Self {
            rows: RuntimeRows::new(account),
        }
    }

    pub(crate) fn establish(
        &mut self,
        types: &ProgramDeclarations<'_, StructuralTypeDeclaration>,
        home: StructuralRuntimePlace,
        structural_type: StructuralTypeId,
    ) -> Result<(), TerminalInterpretError> {
        if terminal_semantics::scalar_array_leaf_shape(types.values(), structural_type)
            .is_none_or(|(_, count)| count != 0)
        {
            return Err(malformed());
        }
        crate::structural_storage::prune_rows(&mut self.rows, &home);
        self.rows.insert(
            home,
            EmptyArrayState {
                structural_type,
                initialized: true,
            },
        )?;
        Ok(())
    }

    fn containing(
        &self,
        home: &StructuralRuntimePlace,
        strict: bool,
    ) -> Option<(&StructuralRuntimePlace, &EmptyArrayState)> {
        let identity = StructuralRuntimePlace {
            opaque_identity: home.opaque_identity,
            path: Vec::new(),
        };
        crate::structural_storage::cells(&self.rows, &identity)
            .filter(|(parent, _)| {
                parent.opaque_identity == home.opaque_identity
                    && home.path.starts_with(&parent.path)
                    && (!strict || parent.path.len() < home.path.len())
            })
            .max_by_key(|(parent, _)| parent.path.len())
    }

    /// Prove complete initialization from explicit compact rows or actual
    /// legacy byte homes. Missing children are never generated from a count:
    /// subdivision visits only coordinates present in the supplied storage.
    pub(crate) fn initialized(
        &self,
        types: &ProgramDeclarations<'_, StructuralTypeDeclaration>,
        bytes: &RuntimeRows<StructuralRuntimePlace, ByteSequenceView>,
        home: &StructuralRuntimePlace,
        structural_type: StructuralTypeId,
    ) -> Result<bool, TerminalInterpretError> {
        if terminal_semantics::scalar_array_leaf_shape(types.values(), structural_type)
            .is_none_or(|(_, count)| count != 0)
        {
            return Err(malformed());
        }
        let mut pending = vec![(home.clone(), structural_type)];
        while let Some((home, structural_type)) = pending.pop() {
            if let Some((parent, state)) = self.containing(&home, false) {
                if select(
                    types,
                    state.structural_type,
                    &home.path[parent.path.len()..],
                )? != structural_type
                {
                    return Err(malformed());
                }
                if state.initialized {
                    // The nearest initialized prefix supplies all unmodified
                    // children. Only explicit vacancies need further evidence.
                    let mut previous: Option<&StructuralRuntimePlace> = None;
                    for (child, state) in crate::structural_storage::cells(&self.rows, &home) {
                        if state.initialized
                            || previous.is_some_and(|parent| child.path.starts_with(&parent.path))
                        {
                            continue;
                        }
                        pending.push((child.clone(), state.structural_type));
                        previous = Some(child);
                    }
                    continue;
                }
            }
            if crate::structural_inputs::byte_arrays::byte_array_length(types, structural_type)
                == Some(0)
                && bytes.get(&home).is_some_and(|view| view.len() == 0)
            {
                continue;
            }
            let StructuralTypeShape::FixedArray { element, length } =
                types.get(&structural_type).ok_or_else(malformed)?.shape
            else {
                return Err(malformed());
            };
            if length == 0 {
                return Ok(false);
            }
            let mut indices = Vec::new();
            for child in crate::structural_storage::cells(&self.rows, &home)
                .map(|(key, _)| key)
                .chain(crate::structural_storage::cells(bytes, &home).map(|(key, _)| key))
            {
                if let Some(StructuralPathSegment::FixedIndex(index)) =
                    child.path.get(home.path.len())
                {
                    if *index >= length {
                        return Err(malformed());
                    }
                    indices.push(*index);
                }
            }
            indices.sort_unstable();
            indices.dedup();
            if u64::try_from(indices.len()).ok() != Some(length) {
                return Ok(false);
            }
            for index in indices {
                let mut child = home.clone();
                child.path.push(StructuralPathSegment::FixedIndex(index));
                pending.push((child, element));
            }
        }
        Ok(true)
    }

    pub(crate) fn initialized_type(
        &self,
        types: &ProgramDeclarations<'_, StructuralTypeDeclaration>,
        bytes: &RuntimeRows<StructuralRuntimePlace, ByteSequenceView>,
        home: &StructuralRuntimePlace,
    ) -> Result<Option<StructuralTypeId>, TerminalInterpretError> {
        let Some((parent, state)) = self.containing(home, false) else {
            return Ok(None);
        };
        let selected = select(
            types,
            state.structural_type,
            &home.path[parent.path.len()..],
        )?;
        Ok(self
            .initialized(types, bytes, home, selected)?
            .then_some(selected))
    }

    pub(crate) fn initialized_byte_array(
        &self,
        types: &ProgramDeclarations<'_, StructuralTypeDeclaration>,
        bytes: &RuntimeRows<StructuralRuntimePlace, ByteSequenceView>,
        home: &StructuralRuntimePlace,
    ) -> Result<bool, TerminalInterpretError> {
        Ok(self
            .initialized_type(types, bytes, home)?
            .is_some_and(|selected| {
                crate::structural_inputs::byte_arrays::byte_array_length(types, selected) == Some(0)
            }))
    }

    pub(crate) fn stage_subtree(
        &mut self,
        types: &ProgramDeclarations<'_, StructuralTypeDeclaration>,
        original: &Self,
        bytes: &RuntimeRows<StructuralRuntimePlace, ByteSequenceView>,
        source: &StructuralRuntimePlace,
        destination: &StructuralRuntimePlace,
    ) -> Result<(), TerminalInterpretError> {
        if let Some(structural_type) = original.initialized_type(types, bytes, source)? {
            self.establish(types, destination.clone(), structural_type)?;
        }
        for (home, state) in crate::structural_storage::cells(&original.rows, source) {
            let mut target = destination.clone();
            target
                .path
                .extend_from_slice(&home.path[source.path.len()..]);
            self.rows.insert(target, *state)?;
        }
        Ok(())
    }

    /// Only a strict ancestor needs a vacancy: removing an entire explicit
    /// root already removes the evidence that could have initialized it.
    pub(crate) fn prepare_retirement(
        &self,
        types: &ProgramDeclarations<'_, StructuralTypeDeclaration>,
        account: Option<&EvaluationInvocationAccount>,
        root: &StructuralRuntimePlace,
    ) -> Result<EmptyArrayRetirement, TerminalInterpretError> {
        let mut vacancy = RuntimeRows::new(account);
        if let Some((parent, state)) = self.containing(root, true)
            && state.initialized
        {
            let structural_type = select(
                types,
                state.structural_type,
                &root.path[parent.path.len()..],
            )?;
            vacancy.insert(
                root.clone(),
                EmptyArrayState {
                    structural_type,
                    initialized: false,
                },
            )?;
        }
        self.rows.validate_append(&vacancy)?;
        Ok(EmptyArrayRetirement {
            root: root.clone(),
            vacancy,
        })
    }

    pub(crate) fn retire(
        &mut self,
        mut retirement: EmptyArrayRetirement,
    ) -> Result<(), TerminalInterpretError> {
        crate::structural_storage::prune_rows(&mut self.rows, &retirement.root);
        self.rows.append(&mut retirement.vacancy)
    }
}

fn select(
    types: &ProgramDeclarations<'_, StructuralTypeDeclaration>,
    mut structural_type: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Result<StructuralTypeId, TerminalInterpretError> {
    for segment in path {
        let StructuralPathSegment::FixedIndex(index) = segment else {
            return Err(malformed());
        };
        let StructuralTypeShape::FixedArray { element, length } =
            types.get(&structural_type).ok_or_else(malformed)?.shape
        else {
            return Err(malformed());
        };
        if *index >= length {
            return Err(malformed());
        }
        structural_type = element;
    }
    if terminal_semantics::scalar_array_leaf_shape(types.values(), structural_type)
        .is_none_or(|(_, count)| count != 0)
    {
        return Err(malformed());
    }
    Ok(structural_type)
}
