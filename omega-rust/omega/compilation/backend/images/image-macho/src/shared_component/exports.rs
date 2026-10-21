//! Encode and independently read the closed regular-export trie.
//!
//! Nodes use dense construction indices; serialized child offsets are laid out
//! to a fixed point because their ULEB widths affect following node positions.
//! The receiver walks byte offsets, checks the actual tree and exact extent,
//! and compares names/addresses with original selected definitions.

use crate::dyld_linking::write_uleb128;
use diagnostics::Diagnostic;

#[derive(Default)]
struct Node {
    address: Option<u64>,
    children: Vec<(u8, usize)>,
}

fn invalid(reason: &str) -> Diagnostic {
    Diagnostic::error(format!("Mach-O component export trie: {reason}"))
}

pub(super) fn encode(exports: &[(Vec<u8>, u64)]) -> Result<Vec<u8>, Diagnostic> {
    let mut nodes = vec![Node::default()];
    for (name, address) in exports {
        if name.is_empty() || name.contains(&0) {
            return Err(invalid(
                "export name is absent or outside the supported byte extent",
            ));
        }
        let mut node = 0;
        for byte in name {
            let next = if let Some((_, index)) =
                nodes[node].children.iter().find(|(edge, _)| edge == byte)
            {
                *index
            } else {
                let index = nodes.len();
                nodes.push(Node::default());
                nodes[node].children.push((*byte, index));
                index
            };
            node = next;
        }
        if nodes[node].address.replace(*address).is_some() {
            return Err(invalid("duplicate exported name"));
        }
    }
    for node in &mut nodes {
        node.children.sort_unstable_by_key(|(byte, _)| *byte);
    }
    let encode_node = |node: &Node, offsets: &[usize]| -> Result<Vec<u8>, Diagnostic> {
        let mut terminal = Vec::new();
        if let Some(address) = node.address {
            write_uleb128(&mut terminal, 0); // Regular symbol, no reexport/resolver flags.
            write_uleb128(&mut terminal, address);
        }
        let mut bytes = Vec::new();
        write_uleb128(&mut bytes, terminal.len() as u64);
        bytes.extend(terminal);
        bytes.push(
            u8::try_from(node.children.len()).map_err(|_| invalid("node child count overflows"))?,
        );
        for (edge, child) in &node.children {
            bytes.extend([*edge, 0]);
            write_uleb128(&mut bytes, offsets[*child] as u64);
        }
        Ok(bytes)
    };
    let mut offsets = vec![0; nodes.len()];
    // Every child offset starts at zero and only increases. Each ULEB width
    // can increase at most ten times; the finite bound is not a retry oracle.
    for _ in 0..nodes.len().saturating_mul(10).saturating_add(1) {
        let mut next = Vec::with_capacity(nodes.len());
        let mut bytes = Vec::new();
        for node in &nodes {
            next.push(bytes.len());
            bytes.extend(encode_node(node, &offsets)?);
        }
        if next == offsets {
            return Ok(bytes);
        }
        offsets = next;
    }
    Err(invalid(
        "child-offset layout did not reach its finite fixed point",
    ))
}

fn unsigned(bytes: &[u8], cursor: &mut usize) -> Result<u64, Diagnostic> {
    let mut value = 0u64;
    for shift in (0..70).step_by(7) {
        let byte = *bytes
            .get(*cursor)
            .ok_or_else(|| invalid("truncated ULEB"))?;
        *cursor += 1;
        if shift == 63 && byte & 0x7e != 0 {
            return Err(invalid("ULEB overflow"));
        }
        value |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            if shift != 0 && byte == 0 {
                return Err(invalid("noncanonical ULEB"));
            }
            return Ok(value);
        }
    }
    Err(invalid("ULEB overflow"))
}

pub(super) fn validate(bytes: &[u8], expected: &[(Vec<u8>, u64)]) -> Result<(), Diagnostic> {
    if bytes.is_empty() {
        return Err(invalid("missing root node"));
    }
    let mut visited = vec![false; bytes.len()];
    let mut pending = vec![(0usize, Vec::<u8>::new())];
    let mut extents = Vec::new();
    let mut actual = Vec::new();
    while let Some((offset, prefix)) = pending.pop() {
        let seen = visited
            .get_mut(offset)
            .ok_or_else(|| invalid("child lies outside trie"))?;
        if std::mem::replace(seen, true) {
            return Err(invalid("cycle or multiply-owned node"));
        }
        let mut cursor = offset;
        let terminal_size = usize::try_from(unsigned(bytes, &mut cursor)?)
            .map_err(|_| invalid("terminal extent overflows"))?;
        let terminal_end = cursor
            .checked_add(terminal_size)
            .filter(|end| *end <= bytes.len())
            .ok_or_else(|| invalid("terminal lies outside trie"))?;
        if terminal_size != 0 {
            let terminal = &bytes[..terminal_end];
            if prefix.is_empty() || unsigned(terminal, &mut cursor)? != 0 {
                return Err(invalid("nonregular or empty export"));
            }
            let address = unsigned(terminal, &mut cursor)?;
            if cursor != terminal_end {
                return Err(invalid("extra terminal payload"));
            }
            actual.push((prefix.clone(), address));
        }
        cursor = terminal_end;
        let count = *bytes
            .get(cursor)
            .ok_or_else(|| invalid("missing child count"))?;
        cursor += 1;
        let mut first_bytes = [false; 256];
        for _ in 0..count {
            let start = cursor;
            while *bytes
                .get(cursor)
                .ok_or_else(|| invalid("unterminated edge"))?
                != 0
            {
                cursor += 1;
            }
            if start == cursor || first_bytes[usize::from(bytes[start])] {
                return Err(invalid("absent or ambiguous edge"));
            }
            first_bytes[usize::from(bytes[start])] = true;
            let mut name = prefix.clone();
            name.extend_from_slice(&bytes[start..cursor]);
            if name.len() > bytes.len() {
                return Err(invalid("expanded name exceeds its byte extent"));
            }
            cursor += 1;
            let child = usize::try_from(unsigned(bytes, &mut cursor)?)
                .map_err(|_| invalid("child offset overflows"))?;
            pending.push((child, name));
        }
        extents.push((offset, cursor));
    }
    extents.sort_unstable();
    let mut end = 0;
    for (start, next) in extents {
        if start != end {
            return Err(invalid("overlapping, unreachable or gapped node bytes"));
        }
        end = next;
    }
    if end != bytes.len() {
        return Err(invalid("unreachable trailing bytes"));
    }
    actual.sort_unstable();
    let mut expected = expected.to_vec();
    expected.sort_unstable();
    if actual != expected {
        return Err(invalid(
            "names or offsets differ from original selected exports",
        ));
    }
    Ok(())
}
