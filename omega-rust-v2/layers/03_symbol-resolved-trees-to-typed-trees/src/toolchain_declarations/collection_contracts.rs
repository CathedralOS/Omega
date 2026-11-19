use super::core_interface::{identity, interface_site};
use crate::{Access, ContractPredicate, ContractTerm, InterfaceContract, MachineEffects, TypedSignature, TypedStage, ValueType};
use omega_lexer::SourceSpan;
use omega_parser::NodeKind;
use std::sync::OnceLock;

#[derive(Clone, Copy)]
pub(crate) enum CollectionPrimitive {
    Read,
    Element,
    ElementMut,
    Range,
    RangeMut,
}

impl CollectionPrimitive {
    pub(crate) fn signature(self) -> &'static TypedSignature {
        &collection_contracts().signatures[self as usize]
    }
    pub(crate) fn requires_copy(self) -> bool {
        matches!(self, Self::Read)
    }
}

/// Bracket selection and later proof checking share these five sealed declarations.
pub fn collection_contracts() -> &'static TypedStage {
    static CONTRACTS: OnceLock<TypedStage> = OnceLock::new();
    CONTRACTS.get_or_init(|| {
        use CollectionPrimitive::*;
        use ContractPredicate::*;
        use ContractTerm::*;
        let mut stage = TypedStage::default();
        for (primitive, name) in [
            (Read, "read"),
            (Element, "element"),
            (ElementMut, "element_mut"),
            (Range, "range"),
            (RangeMut, "range_mut"),
        ] {
            let range = matches!(primitive, Range | RangeMut);
            let mutable = matches!(primitive, ElementMut | RangeMut);
            let access = if mutable { Access::Mut } else { Access::Shared };
            let mut id = identity(format!("indexing::{name}"), NodeKind::Machine);
            id.operands
                .push(if mutable { "@ mut [ #TypeParam0 ]" } else { "@ [ #TypeParam0 ]" }.into());
            let endpoint_count = if range { 2 } else { 1 };
            id.operands
                .extend((0..endpoint_count).map(|_| "u64".to_owned()));
            let element = ValueType::Parameter {
                owner: Box::new(id.clone()),
                index: 0,
            };
            let view = ValueType::Reference(access.clone(), Box::new(ValueType::Slice(Box::new(element.clone()))));
            let mut parameters = vec![(SourceSpan::new(0, 0), view.clone())];
            parameters.extend((0..endpoint_count).map(|_| (SourceSpan::new(0, 0), ValueType::integer("u64").unwrap())));
            let result = match primitive {
                Read => element,
                Element | ElementMut => ValueType::Reference(access, Box::new(element)),
                Range | RangeMut => view,
            };
            stage.signatures.push(TypedSignature {
                site: interface_site(&id),
                machine: id.clone(),
                parameters,
                result,
            });
            stage.machine_effects.push(MachineEffects {
                machine: id.clone(),
                pause: Default::default(),
                fixed_pause: Default::default(),
                callback_pause: Default::default(),
                reach: vec![],
                invokes: vec![],
                crash_ceiling: Some(vec![]),
                terminates: false,
            });
            stage.interface_contracts.push(InterfaceContract {
                declaration: id,
                requires: if range {
                    &[
                        LessEqual(Argument(1), Argument(2)),
                        LessEqual(Argument(2), Length(&Argument(0))),
                    ]
                } else {
                    &[Less(Argument(1), Length(&Argument(0)))]
                },
                ensures: if range {
                    &[Equal(Length(&Result), Subtract(&Argument(2), &Argument(1)))]
                } else {
                    &[]
                },
                copy_parameters: if primitive.requires_copy() { &[0] } else { &[] },
            });
        }
        stage
    })
}
