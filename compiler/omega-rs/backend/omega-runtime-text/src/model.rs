use omega_control_flow::StateKey;
use psi_arena::{Arena, HandleSpan};
use psi_checked_trees::expression::{ExpressionHandle, ExpressionTable};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeTextPlan {
    pub expressions: ExpressionTable,
    pub uses: Arena<RuntimeTextUse>,
    pub buffers: Arena<RuntimeTextBuffer>,
    pub slots: Arena<RuntimeTextSlot>,
    pub writes: Arena<RuntimeTextWrite>,
    pub builders: Arena<RuntimeTextBuilder>,
    pub builder_segments: Arena<RuntimeTextBuilderSegment>,
}

impl RuntimeTextPlan {
    pub fn with_capacity(
        use_capacity: usize,
        buffer_capacity: usize,
        slot_capacity: usize,
        write_capacity: usize,
        builder_capacity: usize,
        builder_segment_capacity: usize,
    ) -> Self {
        Self {
            expressions: ExpressionTable::with_expression_capacity(
                use_capacity
                    .saturating_add(buffer_capacity.saturating_mul(2))
                    .saturating_add(write_capacity.saturating_mul(2))
                    .saturating_add(builder_capacity)
                    .saturating_add(builder_segment_capacity),
            ),
            uses: Arena::with_capacity(use_capacity),
            buffers: Arena::with_capacity(buffer_capacity),
            slots: Arena::with_capacity(slot_capacity),
            writes: Arena::with_capacity(write_capacity),
            builders: Arena::with_capacity(builder_capacity),
            builder_segments: Arena::with_capacity(builder_segment_capacity),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeTextUse {
    pub source_key: StateKey,
    pub statement_index: usize,
    pub expression: ExpressionHandle,
    pub source: RuntimeTextSource,
    pub append_newline: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeTextBuffer {
    pub source_key: StateKey,
    pub statement_index: usize,
    pub target: ExpressionHandle,
    pub text_place: ExpressionHandle,
    pub byte_capacity: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeTextSlot {
    pub place: ExpressionHandle,
    pub byte_capacity: usize,
    pub has_input_buffer: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeTextWrite {
    pub source_key: StateKey,
    pub statement_index: usize,
    pub target: ExpressionHandle,
    pub value: ExpressionHandle,
    pub kind: RuntimeTextWriteKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeTextBuilder {
    pub source_key: StateKey,
    pub statement_index: usize,
    pub target: ExpressionHandle,
    pub segments: HandleSpan<RuntimeTextBuilderSegment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeTextBuilderSegment {
    pub expression: ExpressionHandle,
    pub kind: RuntimeTextBuilderSegmentKind,
}

impl Default for RuntimeTextWrite {
    fn default() -> Self {
        Self {
            source_key: StateKey::default(),
            statement_index: 0,
            target: ExpressionHandle::invalid(),
            value: ExpressionHandle::invalid(),
            kind: RuntimeTextWriteKind::OtherExpression,
        }
    }
}

impl Default for RuntimeTextBuilder {
    fn default() -> Self {
        Self {
            source_key: StateKey::default(),
            statement_index: 0,
            target: ExpressionHandle::invalid(),
            segments: HandleSpan::empty(),
        }
    }
}

impl Default for RuntimeTextBuilderSegment {
    fn default() -> Self {
        Self {
            expression: ExpressionHandle::invalid(),
            kind: RuntimeTextBuilderSegmentKind::OtherExpression,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RuntimeTextWriteKind {
    StaticText,
    StoredCopy,
    GeneratedString,
    #[default]
    OtherExpression,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RuntimeTextBuilderSegmentKind {
    StaticText,
    StoredPlace,
    #[default]
    OtherExpression,
}

impl Default for RuntimeTextSlot {
    fn default() -> Self {
        Self {
            place: ExpressionHandle::invalid(),
            byte_capacity: 0,
            has_input_buffer: false,
        }
    }
}

impl Default for RuntimeTextBuffer {
    fn default() -> Self {
        Self {
            source_key: StateKey::default(),
            statement_index: 0,
            target: ExpressionHandle::invalid(),
            text_place: ExpressionHandle::invalid(),
            byte_capacity: 0,
        }
    }
}

impl Default for RuntimeTextUse {
    fn default() -> Self {
        Self {
            source_key: StateKey::default(),
            statement_index: 0,
            expression: ExpressionHandle::invalid(),
            source: RuntimeTextSource::OtherExpression,
            append_newline: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RuntimeTextSource {
    StoredPlace,
    GeneratedString,
    MutablePlace,
    #[default]
    OtherExpression,
}
