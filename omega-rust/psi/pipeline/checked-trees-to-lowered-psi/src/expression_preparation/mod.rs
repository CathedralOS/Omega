//! Source-bound expressions prepared for emission.
//!
//! A checked scalar or boolean expression is turned into the bindings,
//! computation graph, qualifications and source custody an emitted body needs,
//! before `crate::emission` writes the operations. Every plan family above
//! reaches through here rather than reading checked expressions directly.

use crate::emission::scalar_types::terminal_scalar_type;
use crate::lowering_error::{LoweringError, unsupported};
use checked_trees::types::PrimitiveType;
use checked_trees::{
    CheckedBooleanExpression, CheckedIntegerBinaryKind, CheckedIntegerComparisonKind,
    CheckedScalarExpression, CheckedScalarExpressionRole, CheckedScalarSuccessor, CheckedTrees,
    CheckedUnitStructuralPathSegment,
};
use language_semantics::Multiplicity;
use numerics::arithmetic::ArithmeticDomain;
use semantic_vocabulary::{
    IntegerSign, IntegerValue, PlaceId, ScalarType, StructuralFieldId, StructuralTypeId,
};
use terminal_psi::{
    StructuralAccess, StructuralArgument, StructuralFieldType, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralTypeDeclaration, StructuralTypeShape,
};

pub(crate) mod bindings;
pub(crate) mod computation_graph;
pub(crate) mod prepare_expression;
pub(crate) mod qualifications;
pub(crate) mod source_custody;
