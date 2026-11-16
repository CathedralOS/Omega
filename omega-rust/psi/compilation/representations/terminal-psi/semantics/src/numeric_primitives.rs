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

/// The exact Core declaration for each supported semantic operation. Consumers
/// authenticate its sealed source and full signature separately from this name.
pub fn declaration_path(meaning: NumericIntrinsic) -> Result<String, &'static str> {
    use language_semantics::numeric_intrinsics::NumericType;
    use numerics::arithmetic::ArithmeticDomain;
    let declaration = match meaning {
        NumericIntrinsic::FloatBinary { operation, .. } => {
            Ok(format!("Float::{}", operation.name()))
        }
        NumericIntrinsic::Conversion {
            source: NumericType::F32,
            target: NumericType::F64,
            domain: ArithmeticDomain::Exact,
        } => Ok("F64::from_f32".into()),
        NumericIntrinsic::Conversion {
            source: NumericType::F64,
            target: NumericType::F32,
            domain: ArithmeticDomain::Exact,
        } => Ok("F32::from_f64".into()),
        NumericIntrinsic::Conversion {
            source,
            target: NumericType::Integer(target),
            domain: ArithmeticDomain::Exact,
        } if matches!(source, NumericType::F32 | NumericType::F64) => {
            let source = if source == NumericType::F32 {
                "f32"
            } else {
                "f64"
            };
            Ok(format!(
                "{}::from_{source}",
                target.to_string().to_ascii_uppercase()
            ))
        }
        _ => Err("numeric primitive is outside the supported catalog"),
    }?;
    if !language_core::float_operations::CANONICAL_FLOAT_OPERATIONS.contains(&declaration.as_str())
    {
        return Err("numeric primitive is outside the authored Core operation roster");
    }
    Ok(declaration)
}

/// The sealed catalog retains token-bound declarations as machines and named
/// declarations as operators. Receiving reconstructs that actual source form.
pub fn declaration_kind(
    meaning: NumericIntrinsic,
) -> Result<semantic_vocabulary::BoundaryRequirementKind, &'static str> {
    declaration_path(meaning)?;
    Ok(match meaning {
        NumericIntrinsic::FloatBinary { .. } => {
            semantic_vocabulary::BoundaryRequirementKind::Machine
        }
        NumericIntrinsic::Conversion { .. } => {
            semantic_vocabulary::BoundaryRequirementKind::Operator
        }
        _ => return Err("numeric primitive has no supported declaration kind"),
    })
}

/// The full normal-result contract of the supported closed primitive. Inputs
/// occupy consecutive values starting at 1; the normal result follows them,
/// just as an ordinary scalar callable does. Unsupported operations refuse.
pub fn ensures(meaning: NumericIntrinsic) -> Result<Vec<Proposition>, &'static str> {
    if let Some(target) = integer_conversion_target(meaning) {
        use numerics::float_semantics::{FloatSemanticOperation, FloatSemanticValueKind};
        declaration_path(meaning)?;
        let (formals, result) = scalar_formals(meaning)?;
        let [source] = formals.as_slice() else {
            return Err("integer conversion has a different argument roster");
        };
        let operation = FloatSemanticOperation::from_source_identity(
            "FloatSemantics",
            &format!("to_{target}"),
            &[FloatSemanticValueKind::Meaning],
            FloatSemanticValueKind::Integer(target),
        )
        .ok_or("integer conversion has no complete catalog row")?;
        let relation = semantic_vocabulary::FloatIntegerConversion::new(
            source.clone(),
            result,
            operation.contract_identity(),
        )
        .map_err(|error| error.description())?;
        return Ok(vec![Proposition::FloatIntegerConversion(relation)]);
    }
    let semantic_operation = match meaning {
        NumericIntrinsic::FloatBinary { operation, .. }
            if matches!(
                operation,
                PrimitiveFloatBinaryOperation::Add
                    | PrimitiveFloatBinaryOperation::Subtract
                    | PrimitiveFloatBinaryOperation::Multiply
                    | PrimitiveFloatBinaryOperation::Divide
            ) =>
        {
            Some(operation.name())
        }
        NumericIntrinsic::Conversion { .. } => {
            declaration_path(meaning)?;
            Some("convert")
        }
        _ => None,
    };
    if let Some(name) = semantic_operation {
        use numerics::float_semantics::{
            FloatSemanticOperation,
            FloatSemanticValueKind::{Format, Meaning},
        };
        let (formals, result) = scalar_formals(meaning)?;
        let format = result
            .scalar_type()
            .as_ieee_float()
            .ok_or("numeric primitive has no float result")?;
        let parameters = std::iter::once(Format)
            .chain(std::iter::repeat_n(Meaning, formals.len()))
            .collect::<Vec<_>>();
        let operation = FloatSemanticOperation::from_source_identity(
            "FloatSemantics",
            name,
            &parameters,
            Meaning,
        )
        .ok_or("numeric primitive has no complete semantic catalog row")?;
        let clause = semantic_vocabulary::FloatMeaningClause::result_equation(
            result,
            operation.contract_identity(),
            format,
            formals,
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

fn scalar_formals(
    meaning: NumericIntrinsic,
) -> Result<(Vec<ScalarTerm>, ScalarTerm), &'static str> {
    let signature = meaning
        .signature()
        .ok_or("numeric primitive has no scalar signature")?;
    let operand = signature
        .operand
        .scalar_type()
        .ok_or("numeric primitive has no scalar operand")?;
    let count = u64::try_from(signature.operand_count)
        .map_err(|_| "numeric primitive has too many scalar formals")?;
    let result = count
        .checked_add(1)
        .and_then(ValueId::new)
        .ok_or("numeric primitive has no normal result coordinate")?;
    Ok((
        (1..=count)
            .map(|raw| {
                ScalarTerm::value(
                    ValueId::new(raw).expect("nonzero formal coordinate"),
                    operand,
                )
            })
            .collect(),
        ScalarTerm::value(result, result_scalar_type(meaning)?),
    ))
}

fn integer_conversion_target(
    meaning: NumericIntrinsic,
) -> Option<numerics::integer_carrier::FixedInteger> {
    use language_semantics::numeric_intrinsics::NumericType;
    use numerics::arithmetic::ArithmeticDomain;
    match meaning {
        NumericIntrinsic::Conversion {
            source: NumericType::F32 | NumericType::F64,
            target: NumericType::Integer(target),
            domain: ArithmeticDomain::Exact,
        } => Some(target),
        _ => None,
    }
}

/// Keep the converter's complete finite/toward-zero range requirement. Exact
/// carrier casts prove a separate, stronger predicate before this call.
pub fn requires(meaning: NumericIntrinsic) -> Result<Vec<Proposition>, &'static str> {
    let Some(target) = integer_conversion_target(meaning) else {
        return Ok(Vec::new());
    };
    let (formals, _) = scalar_formals(meaning)?;
    let [source] = formals.as_slice() else {
        return Err("integer conversion needs exactly one argument");
    };
    semantic_vocabulary::float_integer_preconditions(source.clone(), target)
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
    let expected_requires = requires(meaning)?;
    let declaration = declaration_path(meaning)?;
    let kind = declaration_kind(meaning)?;
    let signature = meaning
        .signature()
        .ok_or("numeric primitive has no scalar signature")?;
    let expected_operand = signature
        .operand
        .scalar_type()
        .ok_or("numeric primitive has no scalar operand")?;
    let format_name = match expected_operand {
        ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary32) => "f32",
        ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary64) => "f64",
        _ => return Err("numeric primitive has an unsupported operand carrier"),
    };
    let expected_result = result_scalar_type(meaning)?;
    let result_name = match expected_result {
        ScalarType::Boolean => "bool".to_owned(),
        ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary32) => "f32".to_owned(),
        ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary64) => "f64".to_owned(),
        ScalarType::Integer(integer) => {
            semantic_vocabulary::fixed_integer_type(integer)?.to_string()
        }
    };
    let [owner, state, semantic, callable, parameter, result] = coordinates(
        &declaration,
        kind,
        format_name,
        signature.operand_count,
        &result_name,
    )?;
    let text = |identity| module.identities.text(identity);
    let source = &checked.source;
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
    require!(boundary.kind == kind, "requirement kind");
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
                .all(|&carrier| carrier == expected_operand),
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
        boundary.crash_routes.is_empty() && boundary.scalar_requires == expected_requires,
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
    kind: semantic_vocabulary::BoundaryRequirementKind,
    format: &str,
    arity: usize,
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
    let callable = if kind == semantic_vocabulary::BoundaryRequirementKind::Operator {
        format!(
            "operator::{declaration}({})->{}",
            vec![ordinary(format); arity].join(","),
            ordinary(result)
        )
    } else {
        compound(
            CoordinateTag::NamedCallable,
            [
                atom(
                    CoordinateTag::Path,
                    &compound(
                        NameForm::Nominal,
                        [bare(OwnerForm::Toolchain), atom(Label::Path, declaration)],
                    ),
                ),
                atom(CoordinateTag::Parameters, &vec![parameter; arity].join(",")),
                atom(CoordinateTag::ResultDispatch, ""),
            ],
        )
    };
    Ok([
        exact(declaration),
        if kind == semantic_vocabulary::BoundaryRequirementKind::Operator {
            exact(declaration)
        } else {
            exact(&format!(
                "{declaration}::{}",
                declaration
                    .rsplit("::")
                    .next()
                    .ok_or("numeric declaration has no entry member")?
            ))
        },
        format!("toolchain::{declaration}"),
        callable,
        declared(format),
        declared(result),
    ])
}
