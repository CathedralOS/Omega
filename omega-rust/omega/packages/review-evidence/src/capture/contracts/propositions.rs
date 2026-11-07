// Contract proposition projection modules: `application` projects a proposition
// application, `binders` projects its binder arguments and evidence projections,
// `endpoint` resolves its endpoint and signature, and `evidence` projects its
// witness or proof-obligation evidence.

pub(in crate::capture) mod application;
pub(in crate::capture) mod binders;
pub(in crate::capture) mod endpoint;
pub(in crate::capture) mod evidence;
