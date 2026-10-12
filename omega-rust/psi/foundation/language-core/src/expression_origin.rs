//! Exact syntax-expression birth handles carried by lowering and graph copies.
//! A handle is interpreted only within the retained issuing syntax lineage;
//! neither source spans nor portable semantic identities select its payload.

use arena::Handle;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SourceExpression;

pub type ExpressionSourceOrigin = Handle<SourceExpression>;

/// Integer meaning issued by successful source normalization, including Match
/// result joins, independently of the value codec and any receiving Const slot.
/// Missing formation is never Anonymous.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ConstMatchIntegerMeaning {
    #[default]
    Unavailable,
    Anonymous,
    Typed {
        primitive: numerics::literals::LandedIntegerType,
        arithmetic_domain: numerics::arithmetic::ArithmeticDomain,
    },
}
