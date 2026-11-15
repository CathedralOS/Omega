//! Canonical numeric declarations retain the same semantic contract before
//! realization and after artifact receiving. Arithmetic retains its complete
//! FloatMeaning equation. IEEE comparison is a different predicate,
//! distinct from structural value equality: NaNs never equal themselves and
//! signed zeros compare equal. Both directions tie the Boolean result to it.

use language_semantics::numeric_intrinsics::{NumericIntrinsic, PrimitiveFloatBinaryOperation};
use semantic_vocabulary::{IeeeFloatComparisonKind, Proposition, ScalarTerm, ScalarType, ValueId};

/// One semantic relation serves a comparison's full result contract and its
/// use as a source predicate. Selection has already authenticated the exact
/// declaration; this description neither resolves a spelling nor grants execution.
struct Comparison {
    kind: IeeeFloatComparisonKind,
    format: semantic_vocabulary::IeeeFloatFormat,
    reverse_operands: bool,
}

impl Comparison {
    fn from_meaning(meaning: NumericIntrinsic) -> Result<Self, &'static str> {
        use PrimitiveFloatBinaryOperation as Operation;
        let NumericIntrinsic::FloatBinary { operation, format } = meaning else {
            return Err("numeric primitive has no canonical result contract yet");
        };
        let format = match format {
            numerics::literals::FloatFormat::F32 => semantic_vocabulary::IeeeFloatFormat::Binary32,
            numerics::literals::FloatFormat::F64 => semantic_vocabulary::IeeeFloatFormat::Binary64,
        };
        let (kind, reverse_operands) = match operation {
            Operation::Equal => (IeeeFloatComparisonKind::Equal, false),
            Operation::NotEqual => (IeeeFloatComparisonKind::NotEqual, false),
            Operation::Less => (IeeeFloatComparisonKind::LessThan, false),
            Operation::LessOrEqual => (IeeeFloatComparisonKind::LessOrEqual, false),
            Operation::Greater => (IeeeFloatComparisonKind::LessThan, true),
            Operation::GreaterOrEqual => (IeeeFloatComparisonKind::LessOrEqual, true),
            _ => return Err("numeric primitive has no canonical result contract yet"),
        };
        Ok(Self {
            kind,
            format,
            reverse_operands,
        })
    }

    fn predicate(
        self,
        mut left: ScalarTerm,
        mut right: ScalarTerm,
    ) -> Result<Proposition, &'static str> {
        let operand = ScalarType::IeeeFloat(self.format);
        if left.scalar_type() != operand || right.scalar_type() != operand {
            return Err("numeric predicate changes its declared operand format");
        }
        if self.reverse_operands || (self.kind.is_symmetric() && left > right) {
            std::mem::swap(&mut left, &mut right);
        }
        Ok(Proposition::ScalarIeeeFloatComparison {
            kind: self.kind,
            format: self.format,
            left,
            right,
        })
    }
}

/// The truth of an authenticated Boolean comparison call, with exact actuals.
/// IEEE comparison remains distinct from structural equality of stored bits.
pub fn comparison_predicate(
    meaning: NumericIntrinsic,
    left: ScalarTerm,
    right: ScalarTerm,
) -> Result<Proposition, &'static str> {
    Comparison::from_meaning(meaning)?.predicate(left, right)
}

/// The full normal-result contract of the supported closed primitive. Its
/// declaration-ordered inputs precede the result, as in an ordinary scalar
/// callable. Unsupported catalog operations refuse.
pub fn ensures(meaning: NumericIntrinsic) -> Result<Vec<Proposition>, &'static str> {
    use language_semantics::numeric_intrinsics::NumericType;
    use numerics::float_semantics::{
        FloatSemanticOperation,
        FloatSemanticValueKind::{Format, Integer, Meaning},
    };
    let semantic = match meaning {
        NumericIntrinsic::FloatBinary { operation, .. }
            if matches!(
                operation,
                PrimitiveFloatBinaryOperation::Add
                    | PrimitiveFloatBinaryOperation::Subtract
                    | PrimitiveFloatBinaryOperation::Multiply
                    | PrimitiveFloatBinaryOperation::Divide
            ) =>
        {
            FloatSemanticOperation::from_source_identity(
                "FloatSemantics",
                operation.name(),
                &[Format, Meaning, Meaning],
                Meaning,
            )
        }
        NumericIntrinsic::Conversion {
            source,
            target,
            domain: numerics::arithmetic::ArithmeticDomain::Exact,
        } if target.is_float() => {
            let (name, operand) = match source {
                NumericType::Integer(carrier) => ("from_integer", Integer(carrier)),
                NumericType::F32 | NumericType::F64 => ("convert", Meaning),
            };
            FloatSemanticOperation::from_source_identity(
                "FloatSemantics",
                name,
                &[Format, operand],
                Meaning,
            )
        }
        _ => None,
    };
    if let Some(operation) = semantic {
        let signature = meaning
            .signature()
            .ok_or("numeric primitive has no scalar signature")?;
        let operand = signature
            .operand
            .scalar_type()
            .ok_or("numeric primitive has no scalar operand")?;
        let result = result_scalar_type(meaning)?;
        let ScalarType::IeeeFloat(format) = result else {
            return Err("numeric primitive has no float result format");
        };
        let result_id = u64::try_from(signature.operand_count + 1)
            .map_err(|_| "numeric primitive has too many parameters")?;
        let clause = semantic_vocabulary::FloatMeaningClause::result_equation(
            ScalarTerm::value(
                ValueId::new(result_id).ok_or("numeric primitive has no result identity")?,
                result,
            ),
            operation.contract_identity(),
            format,
            (1..result_id).map(|raw| {
                ScalarTerm::value(ValueId::new(raw).expect("nonzero scalar formal"), operand)
            }),
        )?;
        return Ok(vec![Proposition::FloatMeaningEqual(clause)]);
    }
    let comparison = Comparison::from_meaning(meaning)?;
    let input = |raw| {
        ScalarTerm::value(
            ValueId::new(raw).expect("fixed scalar formal"),
            ScalarType::IeeeFloat(comparison.format),
        )
    };
    let left = input(1);
    let right = input(2);
    let predicate = comparison.predicate(left, right)?;
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

/// The selected primitive's actual scalar result, including its permanent
/// float format. Catalog rows with structural results stay outside this path.
pub fn result_scalar_type(meaning: NumericIntrinsic) -> Result<ScalarType, &'static str> {
    use language_semantics::numeric_intrinsics::NumericResult;
    let signature = meaning
        .signature()
        .ok_or("numeric primitive has no scalar signature")?;
    match signature.result {
        NumericResult::Boolean => Ok(ScalarType::Boolean),
        NumericResult::Primitive(carrier) => carrier
            .scalar_type()
            .ok_or("numeric primitive has no scalar result"),
        NumericResult::FloatClass => Err("numeric primitive has a structural result"),
    }
}

fn carrier_name(carrier: language_semantics::numeric_intrinsics::NumericType) -> String {
    use language_semantics::numeric_intrinsics::NumericType;
    match carrier {
        NumericType::F32 => "f32".to_owned(),
        NumericType::F64 => "f64".to_owned(),
        NumericType::Integer(carrier) => carrier.to_string(),
    }
}

/// The catalog declaration is derived from the closed operation and signature,
/// never from a caller's spelling. Custody is checked separately against Core.
pub fn declaration_path(meaning: NumericIntrinsic) -> Result<String, &'static str> {
    match meaning {
        NumericIntrinsic::FloatBinary { operation, .. } => {
            Ok(format!("Float::{}", operation.name()))
        }
        NumericIntrinsic::Conversion { source, target, .. } if target.is_float() => Ok(format!(
            "{}::from_{}",
            carrier_name(target).to_ascii_uppercase(),
            carrier_name(source)
        )),
        _ => Err("numeric primitive is outside the supported catalog"),
    }
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
    let signature = meaning
        .signature()
        .ok_or("numeric primitive has no scalar signature")?;
    let parameter_name = carrier_name(signature.operand);
    let expected_result = result_scalar_type(meaning)?;
    let result_name = match expected_result {
        ScalarType::Boolean => "bool",
        ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary32) => "f32",
        ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary64) => "f64",
        _ => return Err("numeric primitive has an unsupported result carrier"),
    };
    let declaration = declaration_path(meaning)?;
    let [owner, state, semantic, callable, parameter, result] = coordinates(
        &declaration,
        &parameter_name,
        signature.operand_count,
        result_name,
    )?;
    let text = |identity| module.identities.text(identity);
    let source = &checked.source;
    let expected_scalar = signature
        .operand
        .scalar_type()
        .ok_or("numeric primitive has no scalar operand")?;
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
        source.parameter_types.len() == signature.operand_count
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
        boundary.scalar_parameters.len() == signature.operand_count
            && boundary
                .scalar_parameters
                .iter()
                .all(|&carrier| carrier == expected_scalar),
        "scalar parameter formats"
    );
    require!(
        boundary.parameter_order.len() == signature.operand_count
            && boundary
                .parameter_order
                .iter()
                .all(|kind| *kind == terminal_psi::BoundaryParameterKind::Scalar)
            && boundary.structural_parameters.is_empty(),
        "parameter telescope"
    );
    require!(
        matches!(boundary.result, BoundaryMachineResult::Scalar(result) if result.scalar_type == expected_result && result.qualifications == semantic_vocabulary::ScalarQualificationSetId::ZERO),
        "scalar result format"
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
        "full canonical result contract"
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

fn coordinates(
    declaration: &str,
    format: &str,
    parameter_count: usize,
    result: &str,
) -> Result<[String; 6], &'static str> {
    use language_core::sealed_intrinsics::{
        SealedDeclarationSite, SealedIntrinsic, SealedIntrinsicCatalog,
    };
    use language_semantics::type_identity::encoding::{
        CoordinateTag, atom, bare, byte_atom, compound,
    };
    use language_semantics::type_identity::vocabulary::{Label, NameForm, OwnerForm, TypeForm};
    let (file, bytes) = language_core::sealed_intrinsics::float_operations_source();
    if !language_core::float_operations::CANONICAL_FLOAT_OPERATIONS.contains(&declaration)
        || SealedIntrinsicCatalog::classify(SealedDeclarationSite {
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
                &vec![parameter; parameter_count].join(","),
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
        declared(result),
    ])
}
