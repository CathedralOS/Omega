use omega_target_operations::{StateGuardLowering, StateGuardOperator};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StateGuardClause {
    pub lowering: StateGuardLowering,
    pub operator: StateGuardOperator,
    pub storage: crate::StateGuardOperandStorage,
    pub byte_offset: usize,
    pub right_storage: crate::StateGuardOperandStorage,
    pub right_byte_offset: usize,
    pub byte_size: usize,
    pub expected_value: i64,
    pub has_storage: bool,
    pub has_right_storage: bool,
    /// The compare is FLOAT-kinded: one operand is a constant float
    /// expression (the emission narrows f64 expectation bits by byte_size).
    /// Place-vs-place float conjuncts remain a follow-on (false).
    pub is_float: bool,
}

const INLINE_STATE_GUARD_CLAUSE_COUNT: usize = 4;

pub struct StateGuardClauses {
    inline: [Option<StateGuardClause>; INLINE_STATE_GUARD_CLAUSE_COUNT],
    len: usize,
    overflow: Vec<StateGuardClause>,
}

impl StateGuardClauses {
    pub(crate) fn new() -> Self {
        Self::with_capacity(0)
    }

    pub(crate) fn with_capacity(capacity: usize) -> Self {
        Self {
            inline: [None; INLINE_STATE_GUARD_CLAUSE_COUNT],
            len: 0,
            overflow: Vec::with_capacity(capacity.saturating_sub(INLINE_STATE_GUARD_CLAUSE_COUNT)),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn iter(&self) -> impl Iterator<Item = &StateGuardClause> {
        self.inline
            .iter()
            .take(self.len.min(INLINE_STATE_GUARD_CLAUSE_COUNT))
            .filter_map(Option::as_ref)
            .chain(self.overflow.iter())
    }

    pub(crate) fn push(&mut self, clause: StateGuardClause) {
        if self.len < INLINE_STATE_GUARD_CLAUSE_COUNT {
            self.inline[self.len] = Some(clause);
        } else {
            self.overflow.push(clause);
        }

        self.len += 1;
    }
}
