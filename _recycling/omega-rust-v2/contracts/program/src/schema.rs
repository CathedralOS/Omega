/// A stable schema identity and the member it denotes in authored order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NumberedMember {
    pub number: u64,
    pub declaration_index: usize,
}

/// Present for a numbered record, sum or structured payload, including one
/// containing only retirements. Both lists are sorted by stable identity.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SchemaNumbering {
    pub members: Vec<NumberedMember>,
    pub retired: Vec<u64>,
}
