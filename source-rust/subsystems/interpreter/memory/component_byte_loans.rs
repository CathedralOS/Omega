//! Scoped loans of the original byte cells for an independently executed component.
use crate::boundary::errors::{TerminalInterpretError, malformed};
use crate::boundary::structural_inputs::byte_arrays::storage::{ArrayStorage, ArrayStorageHome};
use crate::boundary::values::{StructuralRuntimePlace, TerminalStructuralValue};
use crate::engine::execution::{TerminalExecution, frame_field_mut};
use crate::storage::byte_sequences::OriginalByteArrayWindow;
use crate::storage::byte_sequences::binding::ByteSequenceBinding;
use crate::storage::scalar_array::storage::ScalarArrayStorage;
use crate::storage::scalar_storage::{ScalarCell, ScalarLoanAccess, ScalarLoanScope};
use crate::storage::storage_addresses::ScalarStorageAddress;
use std::rc::Rc;

// A component only receives the admitted subrange. Its extent and addresses
// originate at the caller, whose frame and type indices are instance-local.
#[derive(Clone)]
pub(crate) struct ComponentByteArray {
    pub(crate) cells: Rc<ScalarArrayStorage<ScalarCell>>,
    pub(crate) first: Option<ScalarStorageAddress>,
}

impl ComponentByteArray {
    pub(crate) fn bind(
        &self,
        child: &mut TerminalExecution<'_>,
        home: TerminalStructuralValue,
    ) -> Result<ByteSequenceBinding, TerminalInterpretError> {
        let length = u64::try_from(self.cells.cells().len()).map_err(|_| malformed())?;
        let root = StructuralRuntimePlace::from(&home);
        if child
            .component_byte_arrays
            .insert(root.clone(), self.clone())?
            .is_some()
        {
            return Err(malformed());
        }
        let mut referent = home.clone();
        referent
            .path
            .push(terminal_psi::StructuralPathSegment::FixedRange {
                start: 0,
                end: length,
            });
        Ok(ByteSequenceBinding::MutableArray {
            array: ArrayStorage {
                referent: root,
                home: ArrayStorageHome::Component,
            },
            array_type: home.structural_type,
            offset: 0,
            length,
            referent,
            descriptor_home: Some(home),
        })
    }
}

impl TerminalExecution<'_> {
    pub(crate) fn lend_component_byte_array(
        &mut self,
        window: &OriginalByteArrayWindow,
        scope: &ScalarLoanScope,
    ) -> Result<ComponentByteArray, TerminalInterpretError> {
        let array = &window.array;
        let first = self.lent_array_window_address(window)?;
        let count = u64::try_from(window.length).map_err(|_| malformed())?;
        let end = window
            .offset
            .checked_add(window.length)
            .ok_or(malformed())?;
        let account = self.account.clone();
        let access = ScalarLoanAccess {
            borrow: terminal_psi::StructuralAccess::MutableBorrow,
            atomic: None,
        };
        let base = if matches!(array.home, ArrayStorageHome::Frame { .. }) {
            self.frame_byte_array_window(array)?.0
        } else {
            0
        };
        let cells = ScalarArrayStorage::collect(
            count,
            (window.offset..end).map(|index| match array.home {
                ArrayStorageHome::Component => {
                    let mut original = self
                        .component_byte_arrays
                        .require(&array.referent)?
                        .cells
                        .cells()
                        .get(index)
                        .ok_or(malformed())?
                        .clone();
                    scope.lend(&mut original, access)
                }
                ArrayStorageHome::Runtime => {
                    let bytes = self
                        .structural_byte_storage
                        .get_mut(&array.referent)
                        .ok_or(malformed())?;
                    let (value, backing) = bytes.capture_scalar(index)?;
                    let mut original = match backing {
                        Some(backing) => ScalarCell::resident(value, backing)?,
                        None => ScalarCell::Local(value),
                    };
                    let loan = scope.lend(&mut original, access)?;
                    bytes.prepare_resident_install(index, &original, account.as_ref())?;
                    bytes.install_resident(index, original)?;
                    Ok(loan)
                }
                ArrayStorageHome::Frame { depth, place, .. } => {
                    let index = base.checked_add(index).ok_or(malformed())?;
                    let bytes = frame_field_mut!(self, depth, scalar_array_values)
                        .backing_mut(&place)
                        .ok_or(malformed())?;
                    let mut original = bytes.cell(index).ok_or(malformed())?.clone();
                    let loan = scope.lend(&mut original, access)?;
                    bytes.install_resident(index, original)?;
                    Ok(loan)
                }
            }),
            account.as_ref(),
        )?;
        Ok(ComponentByteArray {
            cells: Rc::new(cells),
            first,
        })
    }
}
