/*
Postorder term tables: the one wire shape for every recursive Terminal Psi term
family (scalar terms, integer math terms, propositions, proof terms,
certificates, capacity scalars and static bindings).

A recursive term is encoded as a `u32` entry count followed by its entries in
postorder. Each entry is the node's row: its form tag and non-recursive fields,
then one `u32` handle per recursive child, in the child's declared order. A
handle names an earlier entry of the same table, so nesting depth is bounded
only by the table's encoded size, and encoding and decoding are loops over an
explicit stack rather than host-stack recursion
(`wiki/spec/terminal-psi/encoding.md#canonical-form`).

The table is a tree, not a DAG: every entry except the roots is the child of
exactly the next entry that consumes it. The decoder keeps the entries not yet
consumed on a stack, and a row with `k` children must name exactly the top `k`
of that stack, in order. A handle to the current or a later entry is a forward
reference; any other mismatch (a consumed entry, a skipped entry, children out
of order) is noncanonical. Either refusal rejects the module, so one term has
exactly one table spelling and decoding re-encodes byte-for-byte.
*/

use crate::semantic_module::wire::bounded_count;
use crate::semantic_module::{CodecError, Reader, Writer};

/// A term family whose recursive children are encoded as table handles.
pub(crate) trait TableTerm: Sized {
    /// Names the family in refusals.
    const LABEL: &'static str;

    /// Pushes this node's recursive children in declared order.
    fn children<'term>(&'term self, children: &mut Vec<&'term Self>);

    /// Writes this node's row without its child handles.
    fn write_row(&self, writer: &mut Writer) -> Result<(), CodecError>;

    /// Reads one row's fields and takes its children with exactly one
    /// `children.take` call.
    fn read_row(
        reader: &mut Reader<'_>,
        children: &mut TableChildren<'_, Self>,
    ) -> Result<Self, CodecError>;
}

/// The completed entries a row may consume while it is decoded.
pub(crate) struct TableChildren<'stack, T> {
    pending: &'stack mut Vec<(u32, T)>,
    row: u32,
    label: &'static str,
}

impl<T> TableChildren<'_, T> {
    /// Reads `count` handles and returns the entries they name, in order.
    pub(crate) fn take(
        &mut self,
        reader: &mut Reader<'_>,
        count: usize,
    ) -> Result<Vec<T>, CodecError> {
        let base = self.pending.len().checked_sub(count);
        for position in 0..count {
            let handle = reader.u32()?;
            if handle >= self.row {
                return Err(CodecError::TermTableForwardReference(self.label));
            }
            let expected = base.and_then(|base| self.pending.get(base + position));
            if expected.map(|(entry, _)| *entry) != Some(handle) {
                return Err(CodecError::NonCanonicalTermTable(self.label));
            }
        }
        let base = base.ok_or(CodecError::NonCanonicalTermTable(self.label))?;
        Ok(self
            .pending
            .split_off(base)
            .into_iter()
            .map(|(_, term)| term)
            .collect())
    }
}

/// Encodes `roots` as one table whose unconsumed entries are the roots, in order.
pub(crate) fn encode_term_forest<'term, T: TableTerm + 'term>(
    writer: &mut Writer,
    roots: impl IntoIterator<Item = &'term T>,
) -> Result<(), CodecError> {
    encode_rows(
        writer,
        T::LABEL,
        roots,
        |term, children| {
            term.children(children);
            Ok(())
        },
        |term, writer| term.write_row(writer),
    )
}

/// The table encoder for nodes whose children need outside context (an arena).
/// `children` pushes a node's children in declared order; `write_row` writes
/// its row without the child handles.
pub(crate) fn encode_rows<'term, T: 'term>(
    writer: &mut Writer,
    label: &'static str,
    roots: impl IntoIterator<Item = &'term T>,
    mut children: impl FnMut(&'term T, &mut Vec<&'term T>) -> Result<(), CodecError>,
    mut write_row: impl FnMut(&'term T, &mut Writer) -> Result<(), CodecError>,
) -> Result<(), CodecError> {
    enum Step<'term, T> {
        Visit(&'term T),
        Emit(&'term T, usize),
    }
    let mut rows = Writer::default();
    let mut count = 0_u32;
    let mut completed: Vec<u32> = Vec::new();
    let mut pending = Vec::new();
    let mut scratch = Vec::new();
    for root in roots {
        pending.push(Step::Visit(root));
        while let Some(step) = pending.pop() {
            match step {
                Step::Visit(term) => {
                    scratch.clear();
                    children(term, &mut scratch)?;
                    pending.push(Step::Emit(term, scratch.len()));
                    pending.extend(scratch.iter().rev().map(|child| Step::Visit(*child)));
                }
                Step::Emit(term, child_count) => {
                    write_row(term, &mut rows)?;
                    let base = completed.len() - child_count;
                    for handle in completed.drain(base..) {
                        rows.u32(handle);
                    }
                    completed.push(count);
                    count = count
                        .checked_add(1)
                        .ok_or(CodecError::CollectionTooLong(label))?;
                }
            }
        }
    }
    writer.u32(count);
    writer.bytes(&rows.finish());
    Ok(())
}

/// Encodes one term as a table with exactly one root.
pub(crate) fn encode_term_table<T: TableTerm>(
    writer: &mut Writer,
    term: &T,
) -> Result<(), CodecError> {
    encode_term_forest(writer, std::iter::once(term))
}

/// Decodes a table into its roots, in order.
pub(crate) fn decode_term_forest<T: TableTerm>(
    reader: &mut Reader<'_>,
) -> Result<Vec<T>, CodecError> {
    decode_rows(reader, T::LABEL, 1, T::read_row)
}

/// The table decoder for rows that need outside context. Each row occupies at
/// least `minimum_row_bytes`; `read_row` takes the row's children with one
/// `TableChildren::take` call.
pub(crate) fn decode_rows<T>(
    reader: &mut Reader<'_>,
    label: &'static str,
    minimum_row_bytes: usize,
    mut read_row: impl FnMut(&mut Reader<'_>, &mut TableChildren<'_, T>) -> Result<T, CodecError>,
) -> Result<Vec<T>, CodecError> {
    let count = bounded_count(reader, minimum_row_bytes)?;
    let mut pending: Vec<(u32, T)> = Vec::new();
    for row in 0..count {
        let row = u32::try_from(row).map_err(|_| CodecError::CollectionTooLong(label))?;
        let mut children = TableChildren {
            pending: &mut pending,
            row,
            label,
        };
        let term = read_row(reader, &mut children)?;
        pending.push((row, term));
    }
    Ok(pending.into_iter().map(|(_, term)| term).collect())
}

/// Decodes a table that must hold exactly one root.
pub(crate) fn decode_term_table<T: TableTerm>(reader: &mut Reader<'_>) -> Result<T, CodecError> {
    let mut roots = decode_term_forest::<T>(reader)?;
    match (roots.pop(), roots.is_empty()) {
        (Some(root), true) => Ok(root),
        _ => Err(CodecError::NonCanonicalTermTable(T::LABEL)),
    }
}

#[cfg(test)]
mod tests {
    use semantic_vocabulary::{ScalarTerm, ScalarType, ValueId};

    use super::*;
    use crate::semantic_module::{decode_scalar_term, encode_scalar_term};

    /// Box trees drop recursively, so a deep test term is taken apart with an
    /// explicit stack before it is dropped.
    fn dismantle(term: ScalarTerm) {
        let mut pending = vec![term];
        while let Some(mut term) = pending.pop() {
            for operand in term.operands_mut() {
                pending.push(std::mem::replace(operand, ScalarTerm::boolean(false)));
            }
        }
    }

    fn negations(depth: usize) -> ScalarTerm {
        let mut term = ScalarTerm::value(ValueId::new(1).unwrap(), ScalarType::Boolean);
        for _ in 0..depth {
            term = ScalarTerm::boolean_not(term).unwrap();
        }
        term
    }

    fn encoded(term: &ScalarTerm) -> Vec<u8> {
        let mut writer = Writer::default();
        encode_scalar_term(&mut writer, term).unwrap();
        writer.finish()
    }

    #[test]
    fn a_hundred_thousand_deep_term_round_trips_on_the_default_stack() {
        std::thread::spawn(|| {
            let term = negations(100_000);
            let bytes = encoded(&term);
            assert_eq!(&bytes[..4], &100_001_u32.to_le_bytes());
            let mut reader = Reader::new(&bytes);
            let decoded = decode_scalar_term(&mut reader).unwrap();
            assert_eq!(reader.remaining(), 0);
            assert_eq!(encoded(&decoded), bytes);
            dismantle(decoded);
            dismantle(term);
        })
        .join()
        .expect("term tables encode and decode with an explicit stack");
    }

    #[test]
    fn rows_name_exactly_the_unconsumed_entries_before_them() {
        let bytes = encoded(&negations(2));
        // Rows: value (row 0), not -> 0 (row 1), not -> 1 (row 2).
        let second_handle = bytes.len() - 4;
        let first_handle = second_handle - 1 - 4;
        let refused = |offset: usize, handle: u32| {
            let mut changed = bytes.clone();
            changed[offset..offset + 4].copy_from_slice(&handle.to_le_bytes());
            decode_scalar_term(&mut Reader::new(&changed)).unwrap_err()
        };
        assert_eq!(
            refused(second_handle, 2),
            CodecError::TermTableForwardReference("scalar term")
        );
        assert_eq!(
            refused(second_handle, 7),
            CodecError::TermTableForwardReference("scalar term")
        );
        assert_eq!(
            refused(second_handle, 0),
            CodecError::NonCanonicalTermTable("scalar term")
        );
        assert_eq!(
            refused(first_handle, 1),
            CodecError::TermTableForwardReference("scalar term")
        );
        let mut two_roots = bytes.clone();
        two_roots.truncate(first_handle + 4);
        two_roots[..4].copy_from_slice(&2_u32.to_le_bytes());
        assert_eq!(
            decode_scalar_term(&mut Reader::new(&two_roots)).unwrap(),
            negations(1)
        );
        let mut leftover = encoded(&negations(1));
        leftover[..4].copy_from_slice(&3_u32.to_le_bytes());
        leftover.extend_from_slice(&bytes[4..4 + (first_handle - 1 - 4)]);
        assert_eq!(
            decode_scalar_term(&mut Reader::new(&leftover)).unwrap_err(),
            CodecError::NonCanonicalTermTable("scalar term")
        );
        let mut truncated = bytes.clone();
        truncated[..4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(
            decode_scalar_term(&mut Reader::new(&truncated)).unwrap_err(),
            CodecError::UnexpectedEnd
        );
    }
}
