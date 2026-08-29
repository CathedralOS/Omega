mod checked_evidence;
mod projection;

pub(crate) use projection::{
    ContractProjectionContext, project_callable_contracts, project_contracts,
    project_trait_requirement_contracts,
};
