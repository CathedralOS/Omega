//! Authored mathematical telescope structure, before kernel admission.
//!
//! This plan aligns source operands and distinguishes universe, type and term
//! slots. It neither interprets an operand nor proves its carrier or level.

use super::{FixedCore, FixedCoreVocabulary};
use typed_trees::data::{TypeParameter, TypeParameterKind};
use typed_trees::expression::StaticMachineArgument;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

#[derive(Clone, Copy)]
pub enum MathematicalCarrierHead {
    Level,
    Type(TypeReferenceHandle),
    MalformedLevel,
    MalformedType(usize),
    Term,
}

pub fn mathematical_carrier_head(
    program: &dyn crate::CheckingInput,
    carrier: TypeReferenceHandle,
    vocabulary: &FixedCoreVocabulary,
) -> MathematicalCarrierHead {
    if !program
        .type_reference_table
        .contains_type_reference(carrier)
    {
        return MathematicalCarrierHead::Term;
    }
    match program.type_reference_table.type_reference(carrier) {
        TypeReferenceNode::Named { symbol, name } => match vocabulary.classify(*symbol, name) {
            Some(FixedCore::Level) => MathematicalCarrierHead::Level,
            Some(FixedCore::Type) => MathematicalCarrierHead::Type(TypeReferenceHandle::invalid()),
            _ => MathematicalCarrierHead::Term,
        },
        TypeReferenceNode::Generic {
            base_symbol,
            base_name,
            arguments,
            ..
        } => match vocabulary.classify(*base_symbol, base_name) {
            Some(FixedCore::Level) => MathematicalCarrierHead::MalformedLevel,
            Some(FixedCore::Type) => {
                match program
                    .type_reference_table
                    .type_reference_handles(*arguments)
                {
                    [universe] => MathematicalCarrierHead::Type(*universe),
                    arguments => MathematicalCarrierHead::MalformedType(arguments.len()),
                }
            }
            _ => MathematicalCarrierHead::Term,
        },
        _ => MathematicalCarrierHead::Term,
    }
}

#[derive(Clone, Copy)]
pub enum MathematicalBinderKind {
    Level,
    Type,
    Term,
}

pub fn mathematical_binder_kind(
    program: &dyn crate::CheckingInput,
    binder: &TypeParameter,
    vocabulary: &FixedCoreVocabulary,
) -> Option<MathematicalBinderKind> {
    match binder.kind {
        TypeParameterKind::Type => Some(MathematicalBinderKind::Type),
        TypeParameterKind::Const { type_reference }
        | TypeParameterKind::Value { type_reference } => {
            match mathematical_carrier_head(program, type_reference, vocabulary) {
                MathematicalCarrierHead::Level => Some(MathematicalBinderKind::Level),
                MathematicalCarrierHead::Type(_) => Some(MathematicalBinderKind::Type),
                MathematicalCarrierHead::Term => Some(MathematicalBinderKind::Term),
                MathematicalCarrierHead::MalformedLevel
                | MathematicalCarrierHead::MalformedType(_) => None,
            }
        }
        _ => None,
    }
}

pub struct MathematicalGenericArgument<'source> {
    pub binder: &'source TypeParameter,
    pub kind: MathematicalBinderKind,
    /// Only inferred universe slots have no authored operand.
    pub argument: Option<&'source StaticMachineArgument>,
}

pub fn source_generic_argument_plan<'source>(
    program: &dyn crate::CheckingInput,
    binders: &'source [TypeParameter],
    arguments: &'source [StaticMachineArgument],
    vocabulary: &FixedCoreVocabulary,
) -> Option<Vec<MathematicalGenericArgument<'source>>> {
    let kinds = binders
        .iter()
        .map(|binder| mathematical_binder_kind(program, binder, vocabulary))
        .collect::<Option<Vec<_>>>()?;
    let level_count = kinds
        .iter()
        .filter(|kind| matches!(kind, MathematicalBinderKind::Level))
        .count();
    let omit_levels =
        arguments.len() != binders.len() && arguments.len() == binders.len() - level_count;
    if arguments.len() != binders.len() && !omit_levels {
        return None;
    }
    let mut arguments = arguments.iter();
    binders
        .iter()
        .zip(kinds)
        .map(|(binder, kind)| {
            let argument = if omit_levels && matches!(kind, MathematicalBinderKind::Level) {
                None
            } else {
                Some(arguments.next()?)
            };
            Some(MathematicalGenericArgument {
                binder,
                kind,
                argument,
            })
        })
        .collect()
}
