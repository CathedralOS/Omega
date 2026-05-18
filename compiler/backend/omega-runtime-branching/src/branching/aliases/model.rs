use omega_checked_trees::expression::ExpressionHandle;
use omega_checked_trees::name::ProgramName;
use omega_control_flow::StateKey;
use omega_core::symbols::SymbolHandle;

const INLINE_RUNTIME_BRANCH_ALIAS_COUNT: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct RuntimeBranchAlias {
    pub(super) source_key: StateKey,
    pub(super) parameter_symbol: SymbolHandle,
    pub(super) parameter_name: ProgramName,
    pub(super) expression: ExpressionHandle,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct BranchParameterBinding {
    pub(crate) parameter_symbol: SymbolHandle,
    pub(crate) parameter_name: ProgramName,
    pub(crate) expression: ExpressionHandle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeBranchAliasBuffer {
    inline: [Option<RuntimeBranchAlias>; INLINE_RUNTIME_BRANCH_ALIAS_COUNT],
    len: usize,
    overflow: Vec<RuntimeBranchAlias>,
}

impl RuntimeBranchAliasBuffer {
    pub(crate) fn new() -> Self {
        Self {
            inline: std::array::from_fn(|_| None),
            len: 0,
            overflow: Vec::new(),
        }
    }

    pub(crate) fn iter(&self) -> impl DoubleEndedIterator<Item = &RuntimeBranchAlias> {
        self.inline
            .iter()
            .take(self.len.min(INLINE_RUNTIME_BRANCH_ALIAS_COUNT))
            .filter_map(Option::as_ref)
            .chain(self.overflow.iter())
    }

    pub(crate) fn set(&mut self, alias: RuntimeBranchAlias) {
        if let Some(existing_alias) = self.iter_mut().find(|existing_alias| {
            existing_alias.source_key == alias.source_key
                && existing_alias.parameter_symbol == alias.parameter_symbol
        }) {
            *existing_alias = alias;
            return;
        }

        if self.len < INLINE_RUNTIME_BRANCH_ALIAS_COUNT {
            self.inline[self.len] = Some(alias);
        } else {
            self.overflow.push(alias);
        }

        self.len += 1;
    }

    fn iter_mut(&mut self) -> impl Iterator<Item = &mut RuntimeBranchAlias> {
        self.inline
            .iter_mut()
            .take(self.len.min(INLINE_RUNTIME_BRANCH_ALIAS_COUNT))
            .filter_map(Option::as_mut)
            .chain(self.overflow.iter_mut())
    }
}

impl Default for RuntimeBranchAliasBuffer {
    fn default() -> Self {
        Self::new()
    }
}
