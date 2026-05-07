use crate::signature::StateSignature;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Platform {
    pub name: String,
    pub states: Vec<StateSignature>,
}
