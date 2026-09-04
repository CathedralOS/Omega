use super::super::canonical_encoding::{
    encode_definition_site, encode_integer_value, encode_len, encode_scalar_type,
};
use super::super::*;
use super::scalar_evaluation::ScalarConstantValue;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SccpValueState {
    Unknown,
    Boolean(bool),
    Integer(IntegerValue),
    Overdefined,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SccpValueRow {
    pub definition: ValueDefinition,
    pub state: SccpValueState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SccpBlockRow {
    pub block: BlockId,
    pub executable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SccpEdgeState {
    Executable,
    Inexecutable,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SccpEdgeRow {
    pub source: BlockId,
    pub edge: EdgeId,
    pub target: BlockId,
    pub state: SccpEdgeState,
}

/// Canonical result vocabulary for the coupled SCCP fixed point. It contains
/// every block, exact edge, and scalar definition in one machine, so a derived
/// fact identity cannot omit a competing incoming edge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SccpMachineSnapshot {
    pub blocks: Vec<SccpBlockRow>,
    pub edges: Vec<SccpEdgeRow>,
    pub values: Vec<SccpValueRow>,
}

pub fn derived_sccp_scalar_constant_fact_identity(
    input: OptimizationUnitIdentity,
    machine: MachineId,
    definition: ValueDefinition,
    constant: ScalarConstantValue,
    snapshot: &SccpMachineSnapshot,
) -> Option<ScalarConstantFactIdentity> {
    if snapshot
        .blocks
        .windows(2)
        .any(|pair| pair[0].block >= pair[1].block)
        || snapshot
            .edges
            .windows(2)
            .any(|pair| (pair[0].source, pair[0].edge) >= (pair[1].source, pair[1].edge))
        || snapshot
            .values
            .windows(2)
            .any(|pair| pair[0].definition.value >= pair[1].definition.value)
    {
        return None;
    }
    let expected_state = match (definition.scalar_type, constant) {
        (ScalarType::Boolean, ScalarConstantValue::Boolean(value)) => {
            SccpValueState::Boolean(value)
        }
        (ScalarType::Integer(_), ScalarConstantValue::Integer(value)) => {
            SccpValueState::Integer(value)
        }
        _ => return None,
    };
    if !snapshot
        .values
        .iter()
        .any(|row| row.definition == definition && row.state == expected_state)
    {
        return None;
    }

    let mut canonical = Vec::new();
    canonical.extend_from_slice(b"omega.psi-derived-sccp-scalar-constant-fact.v1\0");
    canonical.extend_from_slice(&input.bytes());
    canonical.extend_from_slice(&machine.get().to_le_bytes());
    canonical.extend_from_slice(&definition.value.get().to_le_bytes());
    encode_scalar_type(&mut canonical, definition.scalar_type);
    encode_definition_site(&mut canonical, definition.site);
    encode_scalar_constant_value(&mut canonical, constant);
    encode_len(&mut canonical, snapshot.blocks.len());
    for row in &snapshot.blocks {
        canonical.extend_from_slice(&row.block.get().to_le_bytes());
        canonical.push(u8::from(row.executable));
    }
    encode_len(&mut canonical, snapshot.edges.len());
    for row in &snapshot.edges {
        canonical.extend_from_slice(&row.source.get().to_le_bytes());
        canonical.extend_from_slice(&row.edge.get().to_le_bytes());
        canonical.extend_from_slice(&row.target.get().to_le_bytes());
        canonical.push(match row.state {
            SccpEdgeState::Executable => 1,
            SccpEdgeState::Inexecutable => 2,
            SccpEdgeState::Unknown => 3,
        });
    }
    encode_len(&mut canonical, snapshot.values.len());
    for row in &snapshot.values {
        canonical.extend_from_slice(&row.definition.value.get().to_le_bytes());
        encode_scalar_type(&mut canonical, row.definition.scalar_type);
        encode_definition_site(&mut canonical, row.definition.site);
        match row.state {
            SccpValueState::Unknown => canonical.push(1),
            SccpValueState::Boolean(value) => {
                canonical.push(2);
                canonical.push(u8::from(value));
            }
            SccpValueState::Integer(value) => {
                canonical.push(3);
                encode_integer_value(&mut canonical, value);
            }
            SccpValueState::Overdefined => canonical.push(4),
        }
    }
    Some(ScalarConstantFactIdentity::from_canonical_bytes(&canonical))
}

fn encode_scalar_constant_value(bytes: &mut Vec<u8>, constant: ScalarConstantValue) {
    match constant {
        ScalarConstantValue::Boolean(value) => {
            bytes.push(1);
            bytes.push(u8::from(value));
        }
        ScalarConstantValue::Integer(value) => {
            bytes.push(2);
            encode_integer_value(bytes, value);
        }
    }
}
