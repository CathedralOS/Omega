use crate::contracts::effect_clauses;
use crate::{CrashCause, RequirementSlot, ServiceApplication, Site, ValueType};
use omega_lexer::SourceSpan;

pub(crate) struct MachineSignature {
    pub(crate) params: Vec<(SourceSpan, ValueType)>,
    pub(crate) result: ValueType,
    pub(crate) requires: bool,
    pub(crate) terminates: bool,
    pub(crate) site: Site,
    pub(crate) satisfies: Vec<RequirementSlot>,
    pub(crate) public: bool,
    pub(crate) crash_ceiling: Option<Vec<CrashCause>>,
    pub(crate) pause: effect_clauses::DeclaredPauseBounds,
    pub(crate) reach: Option<Vec<ServiceApplication>>,
}
