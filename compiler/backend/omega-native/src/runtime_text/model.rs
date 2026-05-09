use omega_control_flow::StateKey;
use omega_core::arena::{Arena, HandleSpan};
use omega_typed_program::expression::Expression;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeTextPlan {
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
    pub expression: Expression,
    pub source: RuntimeTextSource,
    pub append_newline: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeTextBuffer {
    pub source_key: StateKey,
    pub statement_index: usize,
    pub platform_call: String,
    pub target: Expression,
    pub byte_capacity: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeTextSlot {
    pub place: Expression,
    pub byte_capacity: usize,
    pub has_input_buffer: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeTextWrite {
    pub source_key: StateKey,
    pub statement_index: usize,
    pub target: Expression,
    pub value: Expression,
    pub kind: RuntimeTextWriteKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeTextBuilder {
    pub source_key: StateKey,
    pub statement_index: usize,
    pub target: Expression,
    pub segments: HandleSpan<RuntimeTextBuilderSegment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeTextBuilderSegment {
    pub expression: Expression,
    pub kind: RuntimeTextBuilderSegmentKind,
}

impl Default for RuntimeTextWrite {
    fn default() -> Self {
        Self {
            source_key: StateKey::default(),
            statement_index: 0,
            target: Expression::String(String::new()),
            value: Expression::String(String::new()),
            kind: RuntimeTextWriteKind::OtherExpression,
        }
    }
}

impl Default for RuntimeTextBuilder {
    fn default() -> Self {
        Self {
            source_key: StateKey::default(),
            statement_index: 0,
            target: Expression::String(String::new()),
            segments: HandleSpan::empty(),
        }
    }
}

impl Default for RuntimeTextBuilderSegment {
    fn default() -> Self {
        Self {
            expression: Expression::String(String::new()),
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
            place: Expression::String(String::new()),
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
            target: Expression::String(String::new()),
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
            expression: Expression::String(String::new()),
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
