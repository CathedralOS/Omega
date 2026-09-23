//! The obligations a typed declaration carries beside its interface.
//!
//! `invocations` types the authored contract invocations a signature states,
//! `proof_facts` lowers a declaration's `where` facts and interns their
//! proposition membership instances, and `parameter_domains` builds the domain
//! membership a parameter's written constraints require. `crate::declarations`
//! and `crate::signatures` populate these while lowering a form;
//! `lowerer::finish` re-enters `proof_facts` once the typed trees are rebuilt.

pub(crate) mod invocations;
pub(crate) mod parameter_domains;
pub(crate) mod proof_facts;
