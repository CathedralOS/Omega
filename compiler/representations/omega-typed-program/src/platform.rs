use crate::name::ProgramName;
use crate::signature::StateSignature;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Platform {
    pub name: ProgramName,
    pub states: Vec<StateSignature>,
}
