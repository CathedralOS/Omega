//! Derived declarations retain their semantic producer, independently of a
//! generated resolver label or the first source occurrence requesting it.
//! These identities supply no placement establishment or execution authority.

use super::constraint_identity::{atom, compound};
use super::identity_context::{
    TypeIdentityContext, TypeIdentityQualification, normalize_type_reference,
};
use crate::TypedTrees;
use crate::types::TypeReferenceNode;
use symbols::SymbolHandle;

pub(super) fn nominal_name(
    program: &TypedTrees,
    symbol: SymbolHandle,
    context: &TypeIdentityContext<'_>,
    hermetic: bool,
) -> Result<Option<String>, &'static str> {
    // The complete applied owner is package qualified even in a local query:
    // two admitted packages can publish the same logical P/T paths.
    let qualified = TypeIdentityContext {
        // Producer identity retains exact semantic value indices. Storage-only
        // erasure must not collapse the applied placement policy or schema.
        storage_value_indices: false,
        qualification: TypeIdentityQualification::PackageQualified,
        ..*context
    };
    for view in &program.placed_view_plans {
        let field = view.fields.iter().find(|field| {
            field.accessor_data_symbol == symbol
                || field
                    .accessor_targets
                    .iter()
                    .any(|target| target.machine_symbol == symbol || target.state_symbol == symbol)
        });
        if view.data_symbol != symbol && field.is_none() {
            continue;
        }
        let origin = program
            .data_definition_by_symbol(view.data_symbol)
            .and_then(|definition| definition.generic_instance)
            .ok_or("derived placed declaration has no retained applied origin")?;
        let TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            lifetime_arguments,
            ..
        } = program.type_reference_table.type_reference(origin)
        else {
            return Err("derived placed origin is not an exact generic application");
        };
        let [policy, schema] = program
            .type_reference_table
            .type_reference_handles(*arguments)
        else {
            return Err("derived placed origin lost its policy/schema operands");
        };
        if !view.template_symbol.is_valid()
            || *base_symbol != view.template_symbol
            || *base_symbol == view.data_symbol
            || !lifetime_arguments.is_empty()
            || !matches!(program.type_reference_table.type_reference(*policy), TypeReferenceNode::Named { symbol, .. } if *symbol == view.policy_symbol)
            || !matches!(program.type_reference_table.type_reference(*schema), TypeReferenceNode::Named { symbol, .. } if *symbol == view.schema_symbol)
            || [view.policy_symbol, view.schema_symbol].contains(&view.data_symbol)
        {
            return Err("derived placed origin changed its exact template/policy/schema join");
        }
        if hermetic
            && [view.template_symbol, view.policy_symbol, view.schema_symbol]
                .iter()
                .any(|symbol| {
                    program.symbols.symbol_package_identity(*symbol).is_none()
                        && program.symbols.symbol_source_origin(*symbol)
                            != Some(source::SourceOrigin::Toolchain)
                })
        {
            return Err("derived placed declaration contains an unowned nominal operand");
        }
        let owner = normalize_type_reference(program, origin, &qualified);
        if view.data_symbol == symbol {
            return Ok(Some(compound("derived-application", [owner])));
        }
        let field = field.ok_or("derived placed declaration lost its field producer")?;
        let selector = match field.member_identity {
            Some(identity) => atom("member", &identity.to_string()),
            None => atom("field", &field.field_name),
        };
        let accessor = compound("placed-field", [owner, selector]);
        if field.accessor_data_symbol == symbol {
            return Ok(Some(accessor));
        }
        for target in &field.accessor_targets {
            let role = if target.machine_symbol == symbol {
                "machine"
            } else if target.state_symbol == symbol {
                "state"
            } else {
                continue;
            };
            return Ok(Some(compound(
                "placed-operation",
                [
                    accessor,
                    atom("operation", &target.operation),
                    atom("role", role),
                ],
            )));
        }
        return Err("derived placed callable lost its exact operation producer");
    }
    Ok(None)
}
