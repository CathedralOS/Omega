use omega_checked_trees::expression::{ExpressionHandle, ExpressionTable};
use omega_control_flow::StateKey;
use omega_core::arena::{Arena, HandleSpan};

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeTextUse {
    pub source_key: StateKey,
    pub statement_index: usize,
    pub platform_call: String,
    pub expression: ExpressionHandle,
    pub source: RuntimeTextSource,
    pub append_newline: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeTextBuffer {
    pub source_key: StateKey,
    pub statement_index: usize,
    pub platform_call: String,
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
            platform_call: String::new(),
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
            platform_call: String::new(),
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
