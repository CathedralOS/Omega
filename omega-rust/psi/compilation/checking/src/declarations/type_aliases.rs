//! Alias definitions own ordinary formation and declaration-bound obligations.

use crate::declarations::symbols::TopLevelSymbols;
use crate::value_custody::type_references::{
    TypeParameterScope, TypeReferenceOwner, validate_const_binder_carriers,
    validate_type_reference_handle,
};
use diagnostics::Diagnostic;

pub(crate) fn validate(
    program: &dyn crate::CheckingInput,
    symbols: &TopLevelSymbols<'_>,
    diagnostics: &mut Vec<Diagnostic>,
    obligations: &mut Vec<checked_trees::IntegerRangeObligation>,
) {
    for alias in program.type_aliases() {
        let parameters = program
            .tables
            .data_type_parameters
            .span_or_empty(alias.type_parameters);
        let scope =
            TypeParameterScope::authored(alias.symbol, parameters, &alias.lifetime_parameters);
        if symbols::builtin_type_symbols()
            .iter()
            .any(|(_, name)| name.as_str() == alias.name.as_str())
        {
            diagnostics.push(Diagnostic::error(format!(
                "type alias `{}` shadows a builtin type",
                alias.name
            )));
        }
        validate_const_binder_carriers(
            program,
            symbols,
            &format!("type alias `{}`", alias.name),
            parameters,
            scope,
            diagnostics,
            obligations,
        );
        crate::declarations::state_signatures::validate_machine_parameter_signatures(
            program,
            symbols,
            diagnostics,
            parameters,
            parameters,
            &alias.lifetime_parameters,
            alias.name.as_str(),
            obligations,
        );
        crate::declarations::traits::validate_conformance_bounds(
            program,
            "type alias",
            alias.name.as_str(),
            &alias.conformance_bounds,
            symbols,
            scope,
            diagnostics,
            obligations,
        );
        validate_type_reference_handle(
            program,
            alias.expansion,
            symbols,
            diagnostics,
            TypeReferenceOwner::TypeAlias {
                name: alias.name.as_str(),
                generic_depth: 0,
            },
            scope,
            obligations,
        );
    }
}
