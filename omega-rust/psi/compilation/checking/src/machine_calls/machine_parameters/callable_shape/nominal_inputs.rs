//! A higher-order provider cannot strengthen its callback's nominal input.
//!
//! The outer requirement promises the nested input contract to callers; the
//! supplied higher-order body must accept that input. If the body demands a
//! named authority, the promised input must carry that exact application.
//! Structural coincidence cannot manufacture a satisfaction, including when
//! phantom trait arguments leave the callback's runtime carrier unchanged.
//! The ordinary callable formation still checks shape, predicates and effects.

use super::super::type_refinement::{BinderBinding, TypeBinding};
use super::complete_callable_type_matches;
use diagnostics::Diagnostic;
use typed_trees::data::{MachineParameterContract, TypeParameter};

#[allow(clippy::too_many_arguments)]
pub(super) fn validate(
    program: &dyn crate::CheckingInput,
    label: &str,
    promised: &MachineParameterContract,
    demanded: &MachineParameterContract,
    generic_types: &[&TypeParameter],
    bindings: &mut Vec<TypeBinding>,
    binder_bindings: &[BinderBinding],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let MachineParameterContract::Nominal {
        trait_definition: demanded_trait,
        requirement: demanded_requirement,
        trait_arguments: demanded_arguments,
        ..
    } = demanded
    else {
        return;
    };
    let mut candidate = bindings.clone();
    let admitted = match promised {
        MachineParameterContract::Nominal {
            trait_definition,
            requirement,
            trait_arguments,
            ..
        } => {
            trait_definition == demanded_trait
                && requirement == demanded_requirement
                && trait_arguments.len() == demanded_arguments.len()
                && trait_arguments
                    .iter()
                    .zip(demanded_arguments)
                    .all(|(promised, demanded)| {
                        complete_callable_type_matches(
                            program,
                            *demanded,
                            *promised,
                            generic_types,
                            &mut candidate,
                            binder_bindings,
                        )
                    })
        }
        _ => false,
    };
    if admitted {
        *bindings = candidate;
    } else {
        diagnostics.push(Diagnostic::error(format!(
            "{label} strengthens its nested nominal input authority; the promised callback does not carry the exact demanded requirement application"
        )));
    }
}
