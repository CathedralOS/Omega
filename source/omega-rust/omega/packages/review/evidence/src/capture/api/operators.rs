use crate::capture::behavior::project_crash_cause;
use crate::capture::contracts::expressions::projection::project_contract_expression;
use crate::capture::contracts::facts::{ContractProjectionContext, project_contracts};
use crate::capture::semantics::declarations::{nominal_identity, reviewed_package_owns};
use crate::capture::semantics::signatures::parameters::project_type_parameters;
use crate::capture::semantics::types::review_signature_type_identity_with_binders;
use crate::capture::source::ProjectedReviewRow;
use crate::capture::source::contracts::project_contract_source_locations;
use crate::capture::source::parameters::{
    collect_callable_parameter_source_locations, collect_type_parameter_source_locations,
};
use crate::record::{
    PackageReviewCallableParameter, PackageReviewCrashRoute, PackageReviewCrashRouteGuard,
    PackageReviewOperatorCoordinate, PackageReviewOperatorShape,
};
use omega_compiler::CheckedCompilation;
use psi_core::PackageKeyIdentity;
use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;

pub(crate) fn project_operator_coordinate(
    compilation: &CheckedCompilation,
    declaration: &psi_typed_trees::operator::OperatorDefinition,
) -> Result<PackageReviewOperatorCoordinate, Vec<Diagnostic>> {
    let identity = nominal_identity(compilation, declaration.symbol)?;
    let overload = compilation.normalized_operator_overload_identity(declaration);
    Ok(PackageReviewOperatorCoordinate {
        identity,
        parameter_dispatch: overload.parameters().to_owned(),
        // Only explicitly named boundary requirements participate in
        // expected-result dispatch. Fixed tokens and ordinary named operators
        // remain operand-directed; their complete return type stays in the
        // row value so a change is one changed declaration, not remove/add.
        result_dispatch: if declaration.is_boundary && declaration.spelling.is_none() {
            overload.result_dispatch().identity()
        } else {
            String::new()
        },
    })
}

pub(crate) fn project_public_operators(
    compilation: &CheckedCompilation,
    package: PackageKeyIdentity,
) -> Result<Vec<ProjectedReviewRow<PackageReviewOperatorShape>>, Vec<Diagnostic>> {
    let derived = psi_typed_trees_to_checked_trees::derive_checked_operator_crash_contracts(
        &compilation.typed,
    );
    if derived != compilation.facts.operators.operator_crash_contracts {
        return Err(vec![Diagnostic::error(format!(
            "retained checked operator-crash evidence does not equal compiler rederivation (retained {} rows, derived {} rows)",
            compilation.facts.operators.operator_crash_contracts.len(),
            derived.len(),
        ))]);
    }
    let mut rows = Vec::new();
    let operators = compilation.operators().iter().chain(
        compilation
            .domain_definitions()
            .iter()
            .flat_map(|domain| compilation.domain_operators(domain)),
    );
    for declaration in operators.filter(|declaration| declaration.is_public) {
        let coordinate = project_operator_coordinate(compilation, declaration)?;
        if !reviewed_package_owns(&coordinate.identity, package)? {
            continue;
        }
        let declaration_path = coordinate.identity.path.as_str();
        let declaration_type_parameters = compilation.operator_type_parameters(declaration);
        let (binders, type_parameters) = project_type_parameters(
            compilation,
            declaration_type_parameters,
            "operator",
            declaration_path,
            &declaration.lifetime_parameters,
        )?;
        let parameters = compilation
            .operator_parameters(declaration)
            .iter()
            .map(|parameter| {
                Ok(PackageReviewCallableParameter {
                    name: parameter.name.as_str().to_owned(),
                    type_identity: review_signature_type_identity_with_binders(
                        compilation,
                        parameter.type_reference,
                        &binders,
                        &declaration.lifetime_parameters,
                    )?,
                    is_const: parameter.is_const,
                    is_mutable: parameter.is_mutable,
                    is_self: parameter.is_self,
                })
            })
            .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
        let context = ContractProjectionContext {
            subject_kind: "public operator",
            subject_name: declaration_path,
            owner: psi_checked_trees::ContractProofFactOwner::OperatorDeclaration {
                operator_symbol: declaration.symbol,
            },
            point: psi_facts::ProgramPoint::Definition {
                symbol: declaration.symbol,
            },
            parameters: compilation.operator_parameters(declaration),
            domain_symbol: None,
            data_symbol: None,
            lifetime_binders: &declaration.lifetime_parameters,
            selection_exposure: psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PublicInterface,
        };
        let contracts = project_contracts(
            compilation,
            compilation.operator_contracts(declaration),
            &context,
            &binders,
        )?;
        let matching_crash = compilation
            .facts
            .operators
            .operator_crash_contracts
            .iter()
            .filter(|checked| checked.operator_symbol() == declaration.symbol)
            .collect::<Vec<_>>();
        let [checked_crash] = matching_crash.as_slice() else {
            return Err(vec![Diagnostic::error(format!(
                "public operator `{declaration_path}` has {} exact checked crash-contract rows; expected one",
                matching_crash.len(),
            ))]);
        };
        let published_crash =
            project_operator_crash_routes(compilation, checked_crash, &context, &binders)?;
        let mut nested_source_locations = project_contract_source_locations(
            compilation,
            compilation.operator_contracts(declaration),
        )?;
        collect_callable_parameter_source_locations(
            compilation,
            compilation.operator_parameters(declaration),
            "public operator parameter",
            &mut nested_source_locations,
        )?;
        collect_type_parameter_source_locations(
            compilation,
            declaration_type_parameters,
            &mut nested_source_locations,
        )?;
        rows.push(ProjectedReviewRow {
            row: PackageReviewOperatorShape {
                coordinate,
                is_boundary: declaration.is_boundary,
                spelling: declaration.spelling,
                lifetime_parameter_count: declaration.lifetime_parameters.len(),
                type_parameters,
                parameters,
                return_type: review_signature_type_identity_with_binders(
                    compilation,
                    declaration.return_type,
                    &binders,
                    &declaration.lifetime_parameters,
                )?,
                contracts,
                published_crash,
            },
            declaration: declaration.symbol,
            nested_source_locations,
        });
    }
    rows.sort_by(|left, right| left.row.coordinate.cmp(&right.row.coordinate));
    if rows
        .windows(2)
        .any(|pair| pair[0].row.coordinate == pair[1].row.coordinate)
    {
        return Err(vec![Diagnostic::error(
            "public operator review produced a duplicate overload coordinate",
        )]);
    }
    Ok(rows)
}

pub(crate) fn project_operator_crash_routes(
    compilation: &CheckedCompilation,
    checked: &psi_checked_trees::CheckedOperatorCrashContract,
    context: &ContractProjectionContext<'_>,
    binders: &[(SymbolHandle, String)],
) -> Result<Vec<PackageReviewCrashRoute>, Vec<Diagnostic>> {
    use psi_typed_trees::domain::ProofFact;

    checked
        .buckets()
        .iter()
        .map(|bucket| {
            let alternative_guards = if bucket.is_unconditional() {
                if !bucket.facts().is_empty() {
                    return Err(vec![Diagnostic::error(format!(
                        "public operator `{}` has an unconditional checked crash bucket with retained guarded facts",
                        context.subject_name
                    ))]);
                }
                vec![PackageReviewCrashRouteGuard::Truth]
            } else {
                let mut guards = bucket
                    .facts()
                    .iter()
                    .map(|fact| {
                        let ProofFact::Expression(expression) = compilation.proof_facts.get(*fact)
                        else {
                            return Err(vec![Diagnostic::error(format!(
                                "public operator `{}` has a non-expression checked crash route",
                                context.subject_name
                            ))]);
                        };
                        project_contract_expression(
                            compilation,
                            context,
                            binders,
                            *expression,
                            Some(*fact),
                            0,
                        )
                        .map(PackageReviewCrashRouteGuard::Expression)
                    })
                    .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
                guards.sort();
                guards.dedup();
                if guards.is_empty() {
                    return Err(vec![Diagnostic::error(format!(
                        "public operator `{}` has an empty guarded checked crash bucket",
                        context.subject_name
                    ))]);
                }
                guards
            };
            Ok(PackageReviewCrashRoute {
                cause: project_crash_cause(bucket.cause()),
                alternative_guards,
            })
        })
        .collect()
}
