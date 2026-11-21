//! Immutable target facts belong to the opaque arguments minted for one admitted
//! compiler invocation. A shared loan follows the atom through ordinary calls.
use crate::{
    TerminalAggregateValue, TerminalBoundaryStorageValue, TerminalEffect, TerminalEffectResult,
    TerminalInterpretError, TerminalScalarValue, TerminalStorageContents, TerminalStructuralValue,
};
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, IntegerSign, IntegerType, IntegerValue, ScalarType,
};
use terminal_psi::{BoundaryMachineDeclaration, TerminalModule};

pub(crate) struct IssuedLayoutTarget {
    pub(crate) facts: layout_plans::LayoutTargetFacts,
    pub(crate) arguments: Vec<TerminalStructuralValue>,
}

impl IssuedLayoutTarget {
    pub(crate) fn snapshot(
        &self,
        module: &TerminalModule,
        boundary: &BoundaryMachineDeclaration,
        effect: &TerminalEffect,
    ) -> Result<TerminalEffectResult, TerminalInterpretError> {
        let reject = TerminalInterpretError::UnsupportedSemanticVariant;
        let TerminalEffect::BoundaryCall {
            structural_arguments,
            ..
        } = effect
        else {
            return Err(reject("a layout target snapshot requires a boundary call"));
        };
        let [receiver] = structural_arguments.as_slice() else {
            return Err(reject("a layout target snapshot requires one receiver"));
        };
        if !self.arguments.contains(receiver) {
            return Err(reject(
                "layout target snapshot receiver was not issued for this invocation",
            ));
        }
        let fields = terminal_semantics::calls::layout_target::snapshot_fields(module, boundary)
            .map_err(reject)?;
        let integer = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
        let word = |value| TerminalStorageContents::Scalar {
            value: TerminalScalarValue::Integer {
                scalar_type: integer,
                value: IntegerValue::Unsigned(u128::from(value)),
            },
            stored_scalar: ScalarType::Integer(integer),
        };
        let member = |field, contents| TerminalBoundaryStorageValue {
            path: vec![CanonicalStructuralPathSegment::Field(field)],
            contents,
        };
        Ok(TerminalEffectResult::Aggregate(TerminalAggregateValue {
            structural_type: fields.result,
            result_case: None,
            fields: vec![
                member(
                    fields.profile,
                    TerminalStorageContents::Bytes(self.facts.profile.as_bytes().into()),
                ),
                member(fields.pointer_size, word(self.facts.pointer_size)),
                member(fields.pointer_alignment, word(self.facts.pointer_alignment)),
                member(
                    fields.little_endian,
                    TerminalStorageContents::Scalar {
                        value: TerminalScalarValue::Boolean(self.facts.little_endian),
                        stored_scalar: ScalarType::Boolean,
                    },
                ),
            ],
        }))
    }
}
