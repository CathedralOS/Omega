//! Canonical numeric declarations retain the same semantic contract before
//! realization and after artifact receiving. IEEE comparison is a predicate,
//! distinct from structural value equality: NaNs never equal themselves and
//! signed zeros compare equal. Both directions tie the Boolean result to it.

use language_semantics::numeric_intrinsics::{NumericIntrinsic, PrimitiveFloatBinaryOperation};
use semantic_vocabulary::{IeeeFloatComparisonKind, Proposition, ScalarTerm, ScalarType, ValueId};

/// The full normal-result contract of the supported closed primitive. Inputs
/// occupy values 1 and 2 and the normal result occupies value 3, just as an
/// ordinary scalar callable does. Unsupported numeric operations refuse.
pub fn ensures(meaning: NumericIntrinsic) -> Result<Vec<Proposition>, &'static str> {
    use PrimitiveFloatBinaryOperation as Operation;
    let NumericIntrinsic::FloatBinary { operation, format } = meaning else {
        return Err("numeric primitive has no canonical result contract yet");
    };
    let format = match format {
        numerics::literals::FloatFormat::F32 => semantic_vocabulary::IeeeFloatFormat::Binary32,
        numerics::literals::FloatFormat::F64 => semantic_vocabulary::IeeeFloatFormat::Binary64,
    };
    let (kind, swap) = match operation {
        Operation::Equal => (IeeeFloatComparisonKind::Equal, false),
        Operation::NotEqual => (IeeeFloatComparisonKind::NotEqual, false),
        Operation::Less => (IeeeFloatComparisonKind::LessThan, false),
        Operation::LessOrEqual => (IeeeFloatComparisonKind::LessOrEqual, false),
        Operation::Greater => (IeeeFloatComparisonKind::LessThan, true),
        Operation::GreaterOrEqual => (IeeeFloatComparisonKind::LessOrEqual, true),
        _ => return Err("numeric primitive has no canonical result contract yet"),
    };
    let input = |raw| {
        ScalarTerm::value(
            ValueId::new(raw).expect("fixed scalar formal"),
            ScalarType::IeeeFloat(format),
        )
    };
    let mut operands = [input(1), input(2)];
    if swap {
        operands.swap(0, 1);
    }
    let [left, right] = operands;
    let predicate = Proposition::ScalarIeeeFloatComparison {
        kind,
        format,
        left,
        right,
    };
    let result = Proposition::Equal(
        ScalarTerm::value(
            ValueId::new(3).expect("fixed scalar result"),
            ScalarType::Boolean,
        ),
        ScalarTerm::Boolean(true),
    );
    Ok(vec![
        Proposition::Implication {
            premise: Box::new(result.clone()),
            conclusion: Box::new(predicate.clone()),
        },
        Proposition::Implication {
            premise: Box::new(predicate),
            conclusion: Box::new(result),
        },
    ])
}

/// Rejoin a retained numeric primitive to its exact catalog declaration and
/// complete public contract. Neither a name nor a numeric tag grants execution.
pub fn validate_boundary(
    module: &terminal_psi::TerminalModule,
    boundary: &terminal_psi::BoundaryMachineDeclaration,
) -> Result<(), &'static str> {
    use terminal_psi::{BoundaryMachineResult, CheckedBoundaryExecution};
    let checked = boundary
        .declaration
        .as_ref()
        .ok_or("numeric primitive has no checked declaration")?;
    let meaning = boundary
        .numeric_intrinsic
        .ok_or("numeric primitive has no closed meaning")?;
    let expected_ensures = ensures(meaning)?;
    let NumericIntrinsic::FloatBinary { operation, format } = meaning else {
        return Err("numeric primitive is outside the supported catalog");
    };
    let format_name = format.name();
    let declaration = format!("Float::{}", operation.name());
    let [owner, state, semantic, callable, parameter, result] =
        coordinates(&declaration, format_name)?;
    let text = |identity| module.identities.text(identity);
    let source = &checked.source;
    let expected_scalar = match format {
        numerics::literals::FloatFormat::F32 => semantic_vocabulary::IeeeFloatFormat::Binary32,
        numerics::literals::FloatFormat::F64 => semantic_vocabulary::IeeeFloatFormat::Binary64,
    };
    macro_rules! require {
        ($condition:expr, $field:literal) => {
            if !$condition {
                return Err(concat!(
                    "numeric primitive differs from its exact Core declaration and full contract: ",
                    $field
                ));
            }
        };
    }
    require!(
        checked.execution == CheckedBoundaryExecution::NumericPrimitive,
        "execution kind"
    );
    require!(
        boundary.kind == semantic_vocabulary::BoundaryRequirementKind::Machine,
        "requirement kind"
    );
    require!(
        boundary.role == terminal_psi::BoundaryDeclarationRole::Callable,
        "callable role"
    );
    require!(
        text(boundary.owner.identity()) == Some(owner.as_str()),
        "exact owner"
    );
    require!(
        text(source.machine.identity()) == Some(owner.as_str()),
        "source machine owner"
    );
    require!(
        text(source.state.identity()) == Some(state.as_str()),
        "source entry state"
    );
    require!(
        text(source.semantic_declaration) == Some(semantic.as_str()),
        "semantic declaration"
    );
    require!(
        text(source.callable) == Some(callable.as_str()),
        "callable identity"
    );
    require!(
        boundary.identity == source.callable,
        "boundary callable coordinate"
    );
    require!(
        source.parameter_types.len() == 2
            && source
                .parameter_types
                .iter()
                .all(|&identity| text(identity) == Some(parameter.as_str())),
        "declared parameter types"
    );
    require!(
        text(source.result_type) == Some(result.as_str()),
        "declared result type"
    );
    require!(
        source.contract != [0; 32] && checked.service_declaration == source.contract,
        "complete contract commitment"
    );
    require!(
        !source.published_may_suspend && !source.published_may_block,
        "operational effects"
    );
    require!(
        source.specialization.is_none() && checked.method_arguments.is_empty(),
        "static application"
    );
    require!(
        boundary.scalar_parameters == [ScalarType::IeeeFloat(expected_scalar); 2],
        "scalar parameter formats"
    );
    require!(
        boundary.parameter_order == [terminal_psi::BoundaryParameterKind::Scalar; 2]
            && boundary.structural_parameters.is_empty(),
        "parameter telescope"
    );
    require!(
        matches!(boundary.result, BoundaryMachineResult::Scalar(result) if result.scalar_type == ScalarType::Boolean && result.qualifications == semantic_vocabulary::ScalarQualificationSetId::ZERO),
        "Boolean result"
    );
    require!(
        boundary.completion == semantic_vocabulary::BoundaryCompletion::NormalReturn
            && boundary.unconditional_termination,
        "completion and termination"
    );
    require!(
        boundary.attachment.is_none() && boundary.owning_service.is_none(),
        "attachment or service owner"
    );
    require!(
        boundary.result_reference_regions.is_empty()
            && boundary
                .result_lifetimes
                .as_ref()
                .is_some_and(|groups| groups.is_empty()),
        "result lifetime declaration"
    );
    require!(
        boundary.crash_routes.is_empty() && boundary.scalar_requires.is_empty(),
        "formation and crash clauses"
    );
    require!(
        boundary.scalar_ensures == expected_ensures,
        "full IEEE result equivalence"
    );
    require!(
        boundary.requires.is_empty()
            && boundary.normal_qualifications.is_empty()
            && boundary.scalar_domain_requirements.is_empty(),
        "domain obligations"
    );
    require!(
        boundary.reference_outputs.is_empty()
            && boundary.program_local_root_introductions.is_empty()
            && boundary.content_guarantees.is_empty(),
        "reference and content contracts"
    );
    require!(
        boundary.fixed_service_reach.is_empty()
            && boundary.published_service_ceiling.is_empty()
            && boundary.published_synchronous_entries.is_empty(),
        "service and entry ceilings"
    );
    Ok(())
}

fn coordinates(declaration: &str, format: &str) -> Result<[String; 6], &'static str> {
    use language_core::sealed_intrinsics::{
        SealedDeclarationSite, SealedIntrinsic, SealedIntrinsicCatalog,
    };
    use language_semantics::type_identity::encoding::{
        CoordinateTag, atom, bare, byte_atom, compound,
    };
    use language_semantics::type_identity::vocabulary::{Label, NameForm, OwnerForm, TypeForm};
    let (file, bytes) = language_core::sealed_intrinsics::float_operations_source();
    if SealedIntrinsicCatalog::classify(SealedDeclarationSite {
        file: std::path::Path::new(file),
        declaration,
        source: bytes,
    }) != Some(SealedIntrinsic::FloatOperationRequirement)
    {
        return Err("numeric primitive has no exact admitted catalog declaration");
    }
    static SOURCE: std::sync::OnceLock<Result<[u8; 32], &'static str>> = std::sync::OnceLock::new();
    let digest = SOURCE
        .get_or_init(|| {
            source::source_identity::toolchain_source_identity_digest(&source::SourceFile {
                source_id: source::SourceId(0),
                path: std::path::Path::new("/compiler/core").join(file),
                package_root: "/compiler/core".into(),
                package_identity: None,
                dependency_scope: source::DependencyScope::Product,
                origin: source::SourceOrigin::Toolchain,
                resolution_stratum: source::SourceResolutionStratum::Base,
                source: std::str::from_utf8(bytes)
                    .map_err(|_| "numeric catalog is not UTF-8")?
                    .into(),
            })
            .map_err(|_| "numeric catalog source has no exact identity")
        })
        .as_ref()
        .map_err(|reason| *reason)?;
    let exact = |path: &str| {
        compound(
            NameForm::Nominal,
            [
                byte_atom(OwnerForm::ToolchainSource, digest),
                atom(Label::Path, path),
            ],
        )
    };
    let ordinary = |name: &str| compound(TypeForm::Named, [atom(Label::Name, name)]);
    let declared =
        |name: &str| ordinary(&compound(NameForm::CompilerType, [atom(Label::Atom, name)]));
    let parameter = compound(
        CoordinateTag::Parameter,
        [
            atom(CoordinateTag::Mutable, "no"),
            atom(CoordinateTag::Const, "no"),
            ordinary(format),
        ],
    );
    let callable = compound(
        CoordinateTag::NamedCallable,
        [
            atom(
                CoordinateTag::Path,
                &compound(
                    NameForm::Nominal,
                    [bare(OwnerForm::Toolchain), atom(Label::Path, declaration)],
                ),
            ),
            atom(
                CoordinateTag::Parameters,
                &format!("{parameter},{parameter}"),
            ),
            atom(CoordinateTag::ResultDispatch, ""),
        ],
    );
    Ok([
        exact(declaration),
        exact(&format!(
            "{declaration}::{}",
            declaration
                .rsplit("::")
                .next()
                .ok_or("numeric declaration has no entry member")?
        )),
        format!("toolchain::{declaration}"),
        callable,
        declared(format),
        declared("bool"),
    ])
}
