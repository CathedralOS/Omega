/*
    Record each evaluated initializer on the declaration that owns it.

    The semantic evaluator establishes the result and builtin meanings. Resolution
    independently checks the retained scalar payload, source roots, and complete
    selected dependency/operator roster. Original roots still undergo ordinary
    declaration-source resolution; initializer implementation exposure never
    becomes public index exposure just because its constant is public.
*/

use super::initializer_dependencies::DependencyPurpose;
use crate::declaration_selection::{
    AuthoredDeclarationSelectionExposure as Exposure,
    AuthoredDeclarationSelectionIntrinsic as Intrinsic, AuthoredDeclarationSelectionKind as Kind,
    AuthoredDeclarationSelectionLateBinding as LateBinding,
    AuthoredDeclarationSelectionTarget as Target,
};
use crate::resolution::lowering::expression::lower_expression_into_table;
use crate::resolution::selections::call_identity::selected_declaration_source;
use crate::resolution::tables::body_expressions;
use crate::resolution::tables::syntax_expressions;
use crate::source_resolved::{SymbolResolvedTrees, expression::ExpressionHandle};
use diagnostics::Diagnostic;
use source_text::SourceSpan;
use symbols::SymbolKind;
use syntax_trees::identifier::JoinedSyntaxPath;
use syntax_trees::item::ConstInitializerCustody;
use syntax_trees::{SyntaxTrees, item::ConstDefinition, types::ConstArgumentOrigin};

mod resolved_payload {
    /*
        Independently encode a materialized initializer after declaration binding.

        Current and retained declarations share resolved identities here. No source
        header is reconstructed, and the receipt supplies neither fields nor types.
        The original declared carrier remains intact; only authenticated explicit
        storage representations project their semantic value for payload encoding.
    */

    use crate::source_resolved::SymbolResolvedTrees;
    use crate::source_resolved::data::{DataDefinition, DataField, DataMember};
    use crate::source_resolved::expression::{
        ExpressionHandle, ExpressionNode, TableStructLiteralField,
    };
    use crate::source_resolved::types::{FixedArrayLength, TypeReference};
    use application_identity::const_value::{
        CanonicalConstIdentity, DecodedCanonicalConstValue as Value,
    };
    use language_core::DataSupplyMode;
    use numeric_literals::EvaluatedConstant;
    use numeric_literals::FloatFormat;
    use symbols::{BuiltinTypeAtom, SymbolHandle, SymbolKind};

    #[derive(Clone, Copy)]
    struct Operand<'program> {
        reference: &'program TypeReference,
        frame: usize,
    }

    struct Binding<'program> {
        parameter: SymbolHandle,
        actual: Operand<'program>,
    }

    struct Encoder<'program> {
        program: &'program SymbolResolvedTrees,
        bindings: Vec<Binding<'program>>,
    }

    pub(super) fn validate(
        program: &SymbolResolvedTrees,
        destination: &TypeReference,
        materialized: ExpressionHandle,
        expected: &EvaluatedConstant,
    ) -> Result<(), String> {
        let mut encoder = Encoder {
            program,
            bindings: Vec::new(),
        };
        let operand = Operand {
            reference: destination,
            frame: 0,
        };
        let actual = if let ExpressionNode::UndeterminedNaN(format) =
            program.tables.bodies.expressions.expression(materialized)
        {
            let operand = encoder.unconstrained(operand)?;
            let TypeReference::Named { symbol, .. } = operand.reference else {
                return Err("undetermined NaN lost its declared floating carrier".into());
            };
            if float_format(program.symbols.builtin_type_atom(*symbol)) != Some(*format) {
                return Err("undetermined NaN conflicts with its declared floating carrier".into());
            }
            EvaluatedConstant::UndeterminedNaN(*format)
        } else {
            let value = encoder.value(operand, materialized, 0)?;
            let identity = CanonicalConstIdentity::from_declaration_value(&value)
                .ok_or("materialized initializer exceeds canonical declaration encoding limits")?;
            EvaluatedConstant::Encoded(identity.encoding)
        };
        if &actual != expected {
            return Err("materialized value drifted from its canonical result".into());
        }
        Ok(())
    }

    impl<'program> Encoder<'program> {
        fn resolve(&self, mut operand: Operand<'program>) -> Result<Operand<'program>, String> {
            for _ in 0..=self.bindings.len() {
                let TypeReference::Named { symbol, .. } = operand.reference else {
                    return Ok(operand);
                };
                let Some(binding) = self.bindings[..operand.frame]
                    .iter()
                    .rev()
                    .find(|binding| binding.parameter == *symbol)
                else {
                    return Ok(operand);
                };
                operand = binding.actual;
            }
            Err("materialized initializer has cyclic declaration arguments".into())
        }

        fn unconstrained(
            &self,
            mut operand: Operand<'program>,
        ) -> Result<Operand<'program>, String> {
            loop {
                operand = self.resolve(operand)?;
                operand.reference = match operand.reference {
                    TypeReference::Constrained(constraint) => {
                        self.program.child_type_reference(constraint.base_type)
                    }
                    TypeReference::AliasUse { expansion, .. } => expansion,
                    _ => return Ok(operand),
                };
            }
        }

        fn value(
            &mut self,
            operand: Operand<'program>,
            expression: ExpressionHandle,
            depth: usize,
        ) -> Result<Value, String> {
            if depth >= 64 {
                return Err("materialized initializer exceeds canonical declaration depth".into());
            }
            let operand = self.unconstrained(operand)?;
            let program = self.program;
            if let TypeReference::Generic(application) = operand.reference
                && let Some(definition) = program.data_definition_by_symbol(application.base_symbol)
                && is_layout_storage(program, definition)
            {
                let arguments = program.child_type_references(application.arguments);
                if arguments.len() != 2 || !application.lifetime_arguments.is_empty() {
                    return Err("explicit storage initializer lost its complete application".into());
                }
                return self.value(
                    Operand {
                        reference: &arguments[0],
                        frame: operand.frame,
                    },
                    expression,
                    depth + 1,
                );
            }
            let table = &program.tables.bodies.expressions;
            match operand.reference {
                TypeReference::FixedArray(array) => {
                    let ExpressionNode::ArrayLiteral(elements) = table.expression(expression)
                    else {
                        return Err("materialized initializer is not its declared array".into());
                    };
                    let length = self.array_length(&array.length, operand.frame)?;
                    let elements = table.expression_handles(*elements);
                    if elements.len() != length {
                        return Err("materialized initializer array length drifted".into());
                    }
                    let element = Operand {
                        reference: program.child_type_reference(array.element_type),
                        frame: operand.frame,
                    };
                    let type_name = format!("[{}; {length}]", self.label(element, depth + 1)?);
                    let values = elements
                        .iter()
                        .map(|element_expression| {
                            self.value(element, *element_expression, depth + 1)
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    Ok(Value::Array { type_name, values })
                }
                TypeReference::Named { symbol, .. }
                    if program.symbols.builtin_type_atom(*symbol).is_some() =>
                {
                    self.scalar(
                        program
                            .symbols
                            .builtin_type_atom(*symbol)
                            .ok_or("scalar initializer lost builtin identity")?,
                        expression,
                    )
                }
                TypeReference::Named { symbol, .. } => {
                    let definition = program
                        .data_definition_by_symbol(*symbol)
                        .ok_or("materialized initializer lost its selected data declaration")?;
                    self.nominal(
                        definition,
                        None,
                        operand.frame,
                        definition.name.as_str().to_owned(),
                        expression,
                        depth,
                    )
                }
                TypeReference::Generic(application) => {
                    let definition = program
                        .data_definition_by_symbol(application.base_symbol)
                        .ok_or(
                            "materialized initializer lost its selected generic data declaration",
                        )?;
                    let label = self.label(operand, depth + 1)?;
                    self.nominal(
                        definition,
                        Some(application),
                        operand.frame,
                        label,
                        expression,
                        depth,
                    )
                }
                _ => Err("materialized initializer has no canonical declaration carrier".into()),
            }
        }

        fn nominal(
            &mut self,
            definition: &'program DataDefinition,
            application: Option<&'program crate::source_resolved::types::GenericTypeReference>,
            frame: usize,
            label: String,
            expression: ExpressionHandle,
            depth: usize,
        ) -> Result<Value, String> {
            let program = self.program;
            let actual_type = match program.tables.bodies.expressions.expression(expression) {
                ExpressionNode::StructLiteral(literal) => literal.type_symbol,
                ExpressionNode::Name(path)
                    if program.symbols.get(path.symbol).kind == SymbolKind::Variant =>
                {
                    program.symbols.get(path.symbol).parent
                }
                _ => return Err("materialized initializer lost its nominal constructor".into()),
            };
            let (definition, application) = if actual_type == definition.symbol {
                (definition, application)
            } else if let Some(TypeReference::Generic(origin)) = &definition.generic_instance {
                if actual_type != origin.base_symbol {
                    return Err(
                        "materialized initializer selects a different nominal instance".into(),
                    );
                }
                (
                    program
                        .data_definition_by_symbol(origin.base_symbol)
                        .ok_or("materialized instance lost its template declaration")?,
                    Some(origin),
                )
            } else {
                return Err("materialized initializer selects a different nominal carrier".into());
            };
            if definition.supply_mode != DataSupplyMode::CheckedShape
                || definition.quotient.is_some()
            {
                return Err(
                    "materialized initializer has no ordinary source-owned constructor".into(),
                );
            }
            let start = self.bindings.len();
            let parameters = program.data_type_parameters(definition.type_parameters);
            if let Some(application) = application {
                let arguments = program.child_type_references(application.arguments);
                if arguments.len() != parameters.len()
                    || application.lifetime_arguments.len() != definition.lifetime_parameters.len()
                {
                    return Err(
                        "materialized initializer lost its complete generic application".into(),
                    );
                }
                for (parameter, argument) in parameters.iter().zip(arguments) {
                    self.bindings.push(Binding {
                        parameter: parameter.symbol,
                        actual: Operand {
                            reference: argument,
                            frame,
                        },
                    });
                }
            } else if !parameters.is_empty() || !definition.lifetime_parameters.is_empty() {
                return Err(
                    "materialized initializer requires a closed declaration application".into(),
                );
            }
            let bound_frame = self.bindings.len();
            let result = (|| {
                let table = &program.tables.bodies.expressions;
                let (actual_type, case, supplied) = match table.expression(expression) {
                    ExpressionNode::StructLiteral(literal) => (
                        literal.type_symbol,
                        literal.case_symbol,
                        table.struct_fields(literal.fields),
                    ),
                    ExpressionNode::Name(path)
                        if program.symbols.get(path.symbol).kind == SymbolKind::Variant =>
                    {
                        (
                            program.symbols.get(path.symbol).parent,
                            Some(path.symbol),
                            &[][..],
                        )
                    }
                    _ => {
                        return Err("materialized initializer lost its nominal constructor".into());
                    }
                };
                if actual_type != definition.symbol {
                    return Err(
                        "materialized initializer selects a different nominal carrier".into(),
                    );
                }
                let members = program.data_members(definition.members);
                if let Some(case) = case {
                    let variant = members
                        .iter()
                        .find_map(|member| match member {
                            DataMember::Variant(variant) if variant.symbol == case => Some(variant),
                            _ => None,
                        })
                        .ok_or(
                            "materialized initializer case belongs to a different declaration",
                        )?;
                    let fields = program
                        .data_payload_fields(variant.payload)
                        .iter()
                        .collect::<Vec<_>>();
                    Ok(Value::Variant {
                        type_name: label,
                        case_name: variant.name.as_str().to_owned(),
                        fields: self.fields(&fields, supplied, bound_frame, depth + 1)?,
                    })
                } else {
                    let fields = members
                        .iter()
                        .filter_map(|member| match member {
                            DataMember::Field(field) => Some(field),
                            _ => None,
                        })
                        .collect::<Vec<_>>();
                    if members
                        .iter()
                        .any(|member| matches!(member, DataMember::Variant(_)))
                    {
                        return Err("materialized initializer omitted its declared case".into());
                    }
                    Ok(Value::Record {
                        type_name: label,
                        fields: self.fields(&fields, supplied, bound_frame, depth + 1)?,
                    })
                }
            })();
            self.bindings.truncate(start);
            result
        }

        fn fields(
            &mut self,
            declared: &[&'program DataField],
            supplied: &'program [TableStructLiteralField],
            frame: usize,
            depth: usize,
        ) -> Result<Vec<(String, Value)>, String> {
            if declared.len() != supplied.len() {
                return Err("materialized initializer field roster drifted".into());
            }
            declared
                .iter()
                .map(|field| {
                    let mut matches = supplied
                        .iter()
                        .filter(|actual| actual.field_symbol == field.symbol);
                    let actual = matches.next().ok_or_else(|| {
                        format!("materialized initializer lost field `{}`", field.name)
                    })?;
                    if matches.next().is_some() {
                        return Err("materialized initializer repeats a declared field".into());
                    }
                    Ok((
                        field.name.as_str().to_owned(),
                        self.value(
                            Operand {
                                reference: &field.type_reference,
                                frame,
                            },
                            actual.value,
                            depth,
                        )?,
                    ))
                })
                .collect()
        }

        fn scalar(
            &self,
            atom: BuiltinTypeAtom,
            expression: ExpressionHandle,
        ) -> Result<Value, String> {
            let node = self
                .program
                .tables
                .bodies
                .expressions
                .expression(expression);
            if atom == BuiltinTypeAtom::Bool {
                return match node {
                    ExpressionNode::Boolean(value) => Ok(Value::Boolean(*value)),
                    _ => Err("materialized initializer conflicts with bool".into()),
                };
            }
            if let Some(format) = float_format(Some(atom)) {
                let literal = match node {
                    ExpressionNode::Float(literal) => literal.clone(),
                    ExpressionNode::Integer(literal) if literal.landing().is_none() => {
                        numeric_literals::FloatLiteral::parse(
                            &literal
                                .value_bignum()
                                .ok_or("materialized floating initializer lost its integer value")?
                                .to_string(),
                        )
                        .ok_or("materialized floating initializer is not exact")?
                    }
                    _ => {
                        return Err(
                            "materialized initializer conflicts with floating carrier".into()
                        );
                    }
                };
                if literal.landing().is_some_and(|actual| actual != format) {
                    return Err("materialized initializer conflicts with floating landing".into());
                }
                let bits = match format {
                    FloatFormat::F32
                        if literal.presented_bits().is_some() || !literal.value_f32().is_nan() =>
                    {
                        u64::from(literal.value_f32().to_bits())
                    }
                    FloatFormat::F64
                        if literal.presented_bits().is_some() || !literal.value_f64().is_nan() =>
                    {
                        literal.value_f64().to_bits()
                    }
                    _ => return Err(
                        "materialized floating initializer lacks determined representation bits"
                            .into(),
                    ),
                };
                return Ok(Value::Float { format, bits });
            }
            let integer_type = match atom {
                BuiltinTypeAtom::Integer(carrier) => {
                    numeric_literals::LandedIntegerType::Fixed(carrier)
                }
                BuiltinTypeAtom::Address => numeric_literals::LandedIntegerType::Addr,
                _ => {
                    return Err(
                        "materialized initializer has an unsupported builtin carrier".into(),
                    );
                }
            };
            let ExpressionNode::Integer(literal) = node else {
                return Err("materialized initializer conflicts with integer carrier".into());
            };
            if literal.landing().is_some_and(|landing| {
                landing.landed_type != integer_type
                    || landing.domain != numerics::arithmetic::ArithmeticDomain::Exact
            }) {
                return Err("materialized initializer conflicts with integer landing".into());
            }
            let value = literal
                .value_bignum()
                .ok_or("materialized initializer integer exceeds canonical storage")?;
            if !integer_type.value_carrier().admits(&value) {
                return Err(format!(
                    "materialized const value `{value}` does not fit `{integer_type}`"
                ));
            }
            Ok(Value::Integer {
                integer_type,
                value,
            })
        }

        fn array_length(&self, length: &FixedArrayLength, frame: usize) -> Result<usize, String> {
            match length {
                FixedArrayLength::Literal(length) => Ok(*length),
                FixedArrayLength::Expression(argument) => {
                    let TypeReference::ConstExpression(value) =
                        self.program.child_type_reference(*argument)
                    else {
                        return Err("materialized array extent is not a completed value".into());
                    };
                    let ExpressionNode::Integer(literal) = self
                        .program
                        .tables
                        .bodies
                        .expressions
                        .expression(value.expression)
                    else {
                        return Err(
                            "materialized array extent is not a completed integer literal".into(),
                        );
                    };
                    literal
                        .value_u64()
                        .and_then(|value| usize::try_from(value).ok())
                        .ok_or_else(|| {
                            "materialized array extent is not a nonnegative host-sized integer"
                                .into()
                        })
                }
                FixedArrayLength::ConstParameter { symbol, .. } => {
                    if symbol.is_valid()
                        && self.program.symbols.get(*symbol).kind == SymbolKind::Const
                    {
                        let (_, value) = self.declaration_integer(*symbol)?;
                        return usize::try_from(value).map_err(|_| {
                            "materialized array extent is not a nonnegative host-sized integer"
                                .into()
                        });
                    }
                    let binding = self.bindings[..frame]
                        .iter()
                        .rev()
                        .find(|binding| binding.parameter == *symbol)
                        .ok_or("materialized array extent has no exact declaration binding")?;
                    let argument = self.resolve(binding.actual)?;
                    let TypeReference::Named { symbol, name, .. } = argument.reference else {
                        return Err("materialized array extent is not a completed integer".into());
                    };
                    if symbol.is_valid()
                        && self.program.symbols.get(*symbol).kind == SymbolKind::Const
                    {
                        let (_, value) = self.declaration_integer(*symbol)?;
                        return usize::try_from(value).map_err(|_| {
                            "materialized array extent is not a nonnegative host-sized integer"
                                .into()
                        });
                    }
                    name.as_str().parse().map_err(|_| {
                        "materialized array extent is not a closed nonnegative integer".into()
                    })
                }
                _ => Err("materialized array extent is not complete".into()),
            }
        }

        /*
            An extent's selected declaration owns both its integer carrier and
            completed initializer. Never read a same-spelled caller constant or a
            provisional receipt encoding to determine geometry.
        */
        fn declaration_integer(
            &self,
            mut symbol: SymbolHandle,
        ) -> Result<(BuiltinTypeAtom, i128), String> {
            let mut active = Vec::new();
            let mut declared_carrier = None;
            loop {
                if active.contains(&symbol) {
                    return Err("materialized array extent has a cyclic constant dependency".into());
                }
                active.push(symbol);
                let declaration = self
                    .program
                    .const_declaration(symbol)
                    .ok_or("materialized array extent lost its exact constant declaration")?;
                let declared_type = declaration.declared_type.as_ref().ok_or(
                    "materialized array extent constant has no declared carrier before typing",
                )?;
                let carrier = self.unconstrained(Operand {
                    reference: declared_type,
                    frame: 0,
                })?;
                let TypeReference::Named {
                    symbol: carrier_symbol,
                    ..
                } = carrier.reference
                else {
                    return Err(
                        "materialized array extent constant must have an integer carrier".into(),
                    );
                };
                let atom = self
                    .program
                    .symbols
                    .builtin_type_atom(*carrier_symbol)
                    .filter(|atom| atom.is_fixed_integer())
                    .ok_or("materialized array extent constant must have an integer carrier")?;
                if declared_carrier.is_some_and(|expected| expected != atom) {
                    return Err(
                        "materialized array extent alias changes its declared integer carrier"
                            .into(),
                    );
                }
                declared_carrier = Some(atom);
                let table = &self.program.tables.bodies.expressions;
                if let ExpressionNode::Name(path) = table.expression(declaration.initializer)
                    && path.symbol.is_valid()
                    && self.program.symbols.get(path.symbol).kind == SymbolKind::Const
                {
                    symbol = path.symbol;
                    continue;
                }
                return match self.scalar(atom, declaration.initializer)? {
                    Value::Integer { value, .. } => value
                        .to_i128()
                        .map(|value| (atom, value))
                        .ok_or("array extent exceeds host indexing resources".to_owned()),
                    _ => {
                        Err("materialized array extent constant is not a completed integer".into())
                    }
                };
            }
        }

        fn label(&self, operand: Operand<'program>, depth: usize) -> Result<String, String> {
            if depth >= 64 {
                return Err("materialized carrier label exceeds canonical depth".into());
            }
            let operand = self.unconstrained(operand)?;
            match operand.reference {
                TypeReference::ConstExpression(recipe) => {
                    match self
                        .program
                        .tables
                        .bodies
                        .expressions
                        .expression(recipe.expression)
                    {
                        ExpressionNode::Integer(literal) => literal
                            .value_bignum()
                            .map(|value| value.to_string())
                            .ok_or_else(|| {
                                "materialized carrier argument has no integer value".into()
                            }),
                        _ => Err("materialized carrier argument is not a completed scalar".into()),
                    }
                }
                TypeReference::Named { symbol, name, .. } => {
                    Ok(self.program.data_definition_by_symbol(*symbol).map_or_else(
                        || name.as_str().to_owned(),
                        |definition| definition.name.as_str().to_owned(),
                    ))
                }
                TypeReference::FixedArray(array) => Ok(format!(
                    "[{}; {}]",
                    self.label(
                        Operand {
                            reference: self.program.child_type_reference(array.element_type),
                            frame: operand.frame
                        },
                        depth + 1
                    )?,
                    self.array_length(&array.length, operand.frame)?
                )),
                TypeReference::Generic(application) => {
                    let name = self
                        .program
                        .data_definition_by_symbol(application.base_symbol)
                        .map_or_else(
                            || self.program.symbols.name(application.base_symbol),
                            |definition| definition.name.as_str(),
                        );
                    let arguments = self
                        .program
                        .child_type_references(application.arguments)
                        .iter()
                        .map(|reference| {
                            self.label(
                                Operand {
                                    reference,
                                    frame: operand.frame,
                                },
                                depth + 1,
                            )
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    Ok(format!("{name}<{}>", arguments.join(", ")))
                }
                _ => Err("materialized initializer carrier has no canonical label".into()),
            }
        }
    }

    fn is_layout_storage(program: &SymbolResolvedTrees, definition: &DataDefinition) -> bool {
        program
            .symbols
            .symbol_source_span(definition.symbol)
            .is_some_and(|reference| {
                crate::resolution::declarations::declaration_selection::is_core_layout_storage_source(
                    &program.symbols,
                    definition.name.as_str(),
                    definition.supply_mode,
                    reference,
                )
            })
    }

    fn float_format(atom: Option<BuiltinTypeAtom>) -> Option<FloatFormat> {
        match atom {
            Some(BuiltinTypeAtom::F32) => Some(FloatFormat::F32),
            Some(BuiltinTypeAtom::F64) => Some(FloatFormat::F64),
            _ => None,
        }
    }
}

/*
    The three existing initializer inventories: original value use, complete
    declaration preparation, and the materialized value's realized selections.
*/
#[derive(Clone, Copy)]
pub enum InitializerCustodyPurpose {
    Authored,
    Declaration,
    Value,
}

/*
    Read a completed predecessor's original graph, never its source syntax or
    a guessed summary. The receiving evaluator still checks every new use.
*/
pub(crate) fn retained_constant_custody(
    program: &SymbolResolvedTrees,
    owner: &crate::source_resolved::constant::ConstDeclaration,
    purpose: InitializerCustodyPurpose,
) -> Result<ConstInitializerCustody, Diagnostic> {
    use super::initializer_dependencies::{DependencyPurpose, collect_declaration};
    let source = program
        .symbols
        .symbol_source_span(owner.symbol)
        .ok_or_else(|| {
            error(
                owner.initializer_source_span,
                "retained constant lost its declaration source",
            )
        })?;
    let mut custody = ConstInitializerCustody::default();
    if matches!(purpose, InitializerCustodyPurpose::Value) {
        append_retained_custody(
            program,
            owner,
            source,
            &mut custody,
            DependencyPurpose::DeclarationReadiness,
        )?;
        return Ok(custody);
    }
    let mut retained = ConstInitializerCustody::default();
    append_retained_custody(
        program,
        owner,
        source,
        &mut retained,
        DependencyPurpose::Authored,
    )?;
    let dependencies = collect_declaration(
        program,
        if owner.authored_initializer.is_valid() {
            owner.authored_initializer
        } else {
            owner.initializer
        },
        owner.declared_type.as_ref(),
        if matches!(purpose, InitializerCustodyPurpose::Authored) {
            DependencyPurpose::Authored
        } else {
            DependencyPurpose::DeclarationReadiness
        },
        None,
        Some(&retained.call_selections),
    )
    .map_err(|reason| error(source, &reason))?;
    if !dependencies.pending_calls.is_empty() {
        return Err(error(
            source,
            "retained initializer has unresolved source call choices",
        ));
    }
    custody.builtin_operators = dependencies.initializer_operators;
    custody.call_selections = dependencies.calls;
    for (reference, declaration_source) in dependencies.constants {
        let declaration = program
            .const_declaration_at_source_span(declaration_source)
            .ok_or_else(|| {
                error(
                    reference,
                    "retained constant dependency lost its declaration",
                )
            })?;
        if !declaration.is_public
            && !program
                .symbols
                .same_source_package(reference, declaration_source)
        {
            return Err(error(
                reference,
                "retained constant selects a private foreign declaration",
            ));
        }
        let origin = ConstArgumentOrigin {
            reference,
            declaration: declaration_source,
            initializer: declaration.initializer_source_span,
            evaluated_value: declaration
                .evaluated_value
                .clone()
                .ok_or_else(|| error(reference, "retained dependency has no completed value"))?,
        };
        if !custody.selections.contains(&origin) {
            custody.selections.push(origin);
        }
    }
    Ok(custody)
}

pub(crate) struct PendingInitializer {
    declaration: SourceSpan,
    materialized: ExpressionHandle,
    pub(crate) authored: ExpressionHandle,
    references: Vec<(SourceSpan, String)>,
    operators: Vec<SourceSpan>,
    receipt: syntax_trees::item::ConstInitializerNormalization,
}

/*
    The exact inventory validated before constant substitution. Substitution may
    inherit another declaration's realized rows; this owner rejoins the same
    root afterward without re-resolving or manufacturing occurrence identities.
*/
pub(crate) struct InitializerRootCustody {
    root: ExpressionHandle,
    reference: SourceSpan,
    declaration: SourceSpan,
    occurrences: Vec<crate::source_resolved::AuthoredDeclarationSelectionOccurrenceId>,
}

pub(crate) fn rejoin_after_substitution(
    program: &mut SymbolResolvedTrees,
    custody: &[InitializerRootCustody],
) -> Result<(), Diagnostic> {
    for owner in custody {
        replace_initializer_lanes(program, owner)?;
    }
    Ok(())
}

fn error(reference: SourceSpan, reason: &str) -> Diagnostic {
    Diagnostic::error(format!("constant initializer normalization: {reason}"))
        .with_source_span(reference)
}

pub(crate) fn retain(
    lowerer: &mut crate::resolution::lowering::lowerer::Lowerer,
    syntax: &SyntaxTrees,
    definition: &ConstDefinition,
    materialized: ExpressionHandle,
) -> Result<(), Diagnostic> {
    let Some(receipt) = &definition.preparation().normalization else {
        return Ok(());
    };
    let reference = syntax.expressions.source_span(definition.value());
    if receipt.authored_expression != definition.initializer
        || !syntax_expressions(syntax).contains_expression(receipt.authored_expression)
        || receipt.authored_expression == definition.value()
        || syntax.expressions.source_span(receipt.authored_expression) != reference
    {
        return Err(error(reference, "lost its exact authored expression"));
    }
    if definition
        .preparation()
        .pending_qualification
        .as_ref()
        .is_some_and(|obligation| {
            obligation.declaration != definition.name.source_span()
                || obligation.initializer != definition.value()
        })
        || definition
            .preparation()
            .qualification
            .as_ref()
            .is_some_and(|qualification| {
                qualification.destination != definition.type_reference
                    || qualification.initializer != definition.value()
            })
    {
        return Err(error(
            reference,
            "initializer lost its exact destination obligation",
        ));
    }
    let mut references = Vec::new();
    let mut operators = Vec::new();
    let mut pending = vec![receipt.authored_expression];
    let mut visited = Vec::new();
    while let Some(expression) = pending.pop() {
        if !syntax.expressions.contains_expression(expression) {
            return Err(error(reference, "authored expression has an invalid child"));
        }
        if visited.contains(&expression) {
            continue;
        }
        visited.push(expression);
        use syntax_trees::expression::{ExpressionNode, MatchPattern};
        match syntax.expressions.expression(expression) {
            ExpressionNode::Integer(_)
            | ExpressionNode::Boolean(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::String(_) => {}
            ExpressionNode::Name {
                members: path,
                application: path_application,
            } if path_application.is_none() => {
                let members = syntax.expressions.identifier_path_members(path.to_owned());
                let (Some(first), Some(last)) = (members.first(), members.last()) else {
                    return Err(error(reference, "authored dependency lost its name"));
                };
                let reference = SourceSpan::new(
                    first.source_span().source_id,
                    source_text::Span::new(
                        first.source_span().span.start,
                        last.source_span().span.end,
                    ),
                );
                let name = members.joined_path_in(syntax);
                let identifier = syntax_trees::identifier::Identifier::new(name.clone(), reference);
                if lowerer
                    .constant_selection
                    .as_ref()
                    .is_some_and(|selection| {
                        selection
                            .qualified_payload_free_case(syntax, &identifier)
                            .is_ok_and(|selected| selected.is_some())
                    })
                {
                    continue;
                }
                references.push((reference, name));
            }
            ExpressionNode::Binary(binary) => {
                operators.push(syntax.expressions.source_span(expression));
                pending.push(binary.right);
                pending.push(binary.left);
            }
            ExpressionNode::Member(member) => pending.push(member.receiver),
            ExpressionNode::Unary(unary) => {
                operators.push(syntax.expressions.source_span(expression));
                pending.push(unary.operand);
            }
            ExpressionNode::Cast(cast) => pending.push(cast.value),
            ExpressionNode::Call(call) => {
                let arguments = syntax.expressions.expression_handles(call.arguments);
                if arguments.len() != call.arguments.len() {
                    return Err(error(reference, "authored call has a stale argument span"));
                }
                pending.extend(arguments.iter().rev().copied());
            }
            ExpressionNode::ArrayLiteral(elements) => {
                pending.extend(
                    syntax_expressions(syntax)
                        .expression_handles(*elements)
                        .iter()
                        .rev()
                        .copied(),
                );
            }
            ExpressionNode::StructLiteral(literal) => {
                let fields = syntax.expressions.struct_fields(literal.fields);
                if fields.len() != literal.fields.len() {
                    return Err(error(
                        reference,
                        "authored constructor has a stale field span",
                    ));
                }
                pending.extend(fields.iter().rev().map(|field| field.value));
            }
            ExpressionNode::Match(dispatch) => {
                for arm in syntax.expressions.match_arms(dispatch.arms).iter().rev() {
                    pending.push(arm.value);
                    if let MatchPattern::Value(pattern) = arm.pattern {
                        pending.push(pattern);
                    }
                }
                pending.push(dispatch.subject);
            }
            _ => {
                return Err(error(
                    reference,
                    "authored expression is outside constant leaf evaluation",
                ));
            }
        }
    }
    let original = lowerer
        .with_authored_expression_exposure(Exposure::PrivateImplementation, |lowerer| {
            lower_expression_into_table(lowerer, syntax, receipt.authored_expression)
        })?;
    lowerer.pending_const_values.push(original);
    lowerer.pending_const_initializers.push(PendingInitializer {
        declaration: definition.name.source_span(),
        materialized,
        authored: original,
        references,
        operators,
        receipt: receipt.clone(),
    });
    Ok(())
}

/*
    Reconstruct the transitive roster from original references, not receipt
    membership. Even omitted or unused dependencies must rejoin current values.
    Rejoin the receipt to its exact retained owner before traversing geometry.
    A same-spelled declaration or a sibling instance cannot supply this type.
*/
fn declaration_custody(
    program: &SymbolResolvedTrees,
    record: &PendingInitializer,
    purpose: DependencyPurpose,
) -> Result<super::initializer_dependencies::ConstInitializerDependencies, Diagnostic> {
    let mut owners = program.const_declarations.iter().filter(|declaration| {
        program.symbols.symbol_source_span(declaration.symbol) == Some(record.declaration)
            && declaration.initializer == record.materialized
    });
    let owner = owners.next().ok_or_else(|| {
        error(
            record.declaration,
            "normalization lost its exact declaration owner",
        )
    })?;
    if owners.next().is_some() {
        return Err(error(
            record.declaration,
            "normalization has ambiguous declaration custody",
        ));
    }
    let dependencies = super::initializer_dependencies::collect_declaration(
        program,
        record.authored,
        owner.declared_type.as_ref(),
        purpose,
        None,
        Some(if purpose == DependencyPurpose::Authored {
            &record.receipt.authored_custody.call_selections
        } else {
            &record.receipt.declaration_custody.call_selections
        }),
    )
    .map_err(|reason| error(record.declaration, &reason))?;
    if !dependencies.pending_calls.is_empty() {
        return Err(error(
            record.declaration,
            "normalization has unresolved source call choices",
        ));
    }
    Ok(dependencies)
}

fn reconstruct(
    program: &SymbolResolvedTrees,
    records: &[PendingInitializer],
    root: usize,
    purpose: DependencyPurpose,
) -> Result<(Vec<ConstArgumentOrigin>, Vec<SourceSpan>), Diagnostic> {
    let mut origins = Vec::new();
    let mut operators = Vec::new();
    let mut pending = vec![root];
    let mut visited = Vec::new();
    while let Some(ordinal) = pending.pop() {
        if visited.contains(&ordinal) {
            continue;
        }
        visited.push(ordinal);
        let record = &records[ordinal];
        for operator in &record.operators {
            if !operators.contains(operator) {
                operators.push(*operator);
            }
        }
        let mut dependencies = Vec::new();
        for (reference, name) in &record.references {
            let selected = program
                .symbols
                .find_top_level_by_name_and_kinds_from_source(
                    name,
                    &[SymbolKind::Const],
                    *reference,
                )
                .ok_or_else(|| {
                    error(*reference, "dependency no longer selects an exact constant")
                })?;
            dependencies.push((*reference, selected));
        }
        let closure = declaration_custody(program, record, purpose)?;
        for operator in &closure.initializer_operators {
            if !operators.contains(operator) {
                operators.push(*operator);
            }
        }

        for (reference, declaration) in closure.constants {
            let selected = program
                .const_declarations
                .iter()
                .find(|candidate| {
                    program.symbols.symbol_source_span(candidate.symbol) == Some(declaration)
                })
                .ok_or_else(|| error(reference, "closure dependency lost its exact declaration"))?
                .symbol;
            if !dependencies.contains(&(reference, selected)) {
                dependencies.push((reference, selected));
            }
        }
        for (reference, selected) in &dependencies {
            let declaration = program
                .const_declaration(*selected)
                .ok_or_else(|| error(*reference, "selected dependency lost its declaration"))?;
            let source = program
                .symbols
                .symbol_source_span(*selected)
                .ok_or_else(|| error(*reference, "selected dependency lost its source"))?;
            if !declaration.is_public && !program.symbols.same_source_package(*reference, source) {
                return Err(error(
                    *reference,
                    "dependency selects a private foreign declaration",
                ));
            }
            let origin = ConstArgumentOrigin {
                reference: *reference,
                declaration: source,
                initializer: declaration.initializer_source_span,
                evaluated_value: declaration.evaluated_value.clone().ok_or_else(|| {
                    error(*reference, "dependency has no completed canonical value")
                })?,
            };
            if !origins.contains(&origin) {
                origins.push(origin);
            }
            if let Some(dependency) = records
                .iter()
                .position(|record| record.declaration == source)
            {
                if dependency == root {
                    return Err(error(*reference, "normalized initializer dependency cycle"));
                }
                pending.push(dependency);
            } else {
                let mut retained = ConstInitializerCustody::default();
                append_retained_custody(
                    program,
                    declaration,
                    records[root].declaration,
                    &mut retained,
                    purpose,
                )?;
                for origin in retained.selections {
                    if !origins.contains(&origin) {
                        origins.push(origin);
                    }
                }
                for operator in retained.builtin_operators {
                    if !operators.contains(&operator) {
                        operators.push(operator);
                    }
                }
            }
        }
    }
    Ok((origins, operators))
}

/*
    A seeded base already owns the complete transitive roster on its resolved
    initializer. Rejoin those exact selections rather than resolving its source
    again under an extension's namespace.
*/
fn append_retained_custody(
    program: &SymbolResolvedTrees,
    owner: &crate::source_resolved::constant::ConstDeclaration,
    root: SourceSpan,
    custody: &mut ConstInitializerCustody,
    purpose: DependencyPurpose,
) -> Result<(), Diagnostic> {
    let owner_source = program.symbols.symbol_source_span(owner.symbol);
    for occurrence in body_expressions(program).authored_selection_occurrences(
        if purpose == DependencyPurpose::Authored && owner.authored_initializer.is_valid() {
            owner.authored_initializer
        } else {
            owner.initializer
        },
    ) {
        let selection = program
            .authored_declaration_selections()
            .get(occurrence)
            .ok_or_else(|| error(root, "retained initializer lost its exact selection"))?;
        let reference = selection.source_span();
        match (selection.kind(), selection.target()) {
            (Kind::Call, Target::Resolved(selected))
                if matches!(
                    program.symbols.get(selected.selected_symbol()).kind,
                    SymbolKind::Machine | SymbolKind::State | SymbolKind::Operator
                ) =>
            {
                let declaration = selected_declaration_source(program, selected.selected_symbol())
                    .map_err(|reason| error(reference, &reason))?;
                if !custody.call_selections.contains(&(reference, declaration)) {
                    custody.call_selections.push((reference, declaration));
                }
            }
            (Kind::Operator, Target::Intrinsic(Intrinsic::BuiltinOperator)) => {
                if !custody.builtin_operators.contains(&reference) {
                    custody.builtin_operators.push(reference);
                }
            }
            (Kind::StaticPathSegment, Target::Resolved(selected))
                if matches!(
                    program.symbols.get(selected.selected_symbol()).kind,
                    SymbolKind::Module | SymbolKind::Data | SymbolKind::Variant
                ) => {}
            (Kind::StaticPathSegment, Target::Resolved(selected)) => {
                let declaration = program
                    .const_declaration(selected.selected_symbol())
                    .ok_or_else(|| {
                        error(
                            reference,
                            "retained initializer dependency is not a constant",
                        )
                    })?;
                let source = program
                    .symbols
                    .symbol_source_span(declaration.symbol)
                    .ok_or_else(|| {
                        error(reference, "retained initializer dependency lost its source")
                    })?;
                if source == root || Some(source) == owner_source {
                    return Err(error(reference, "normalized initializer dependency cycle"));
                }
                if !declaration.is_public && !program.symbols.same_source_package(reference, source)
                {
                    return Err(error(
                        reference,
                        "retained initializer selects a private foreign declaration",
                    ));
                }
                let origin = ConstArgumentOrigin {
                    reference,
                    declaration: source,
                    initializer: declaration.initializer_source_span,
                    evaluated_value: declaration.evaluated_value.clone().ok_or_else(|| {
                        error(
                            reference,
                            "retained dependency has no completed canonical value",
                        )
                    })?,
                };
                if !custody.selections.contains(&origin) {
                    custody.selections.push(origin);
                }
            }
            (
                Kind::StructLiteralType
                | Kind::StructLiteralCase
                | Kind::StructLiteralField
                | Kind::CaseReference,
                Target::Resolved(_),
            ) => {}
            _ => {
                return Err(error(
                    reference,
                    "retained initializer selection has no checked scalar meaning",
                ));
            }
        }
    }
    Ok(())
}

/*
    After ordinary authored selections have been recorded, close duplicate
    declaration-side operator obligations using the receipt's evaluated meaning.
    Instantiated partitions and unrelated source occurrences retain their own
    selection obligations.
*/
pub(crate) fn finalize_operator_obligations(
    program: &mut SymbolResolvedTrees,
    records: &[PendingInitializer],
) -> Result<(), Diagnostic> {
    let occurrences = program
        .authored_declaration_selections()
        .iter()
        .filter(|selection| {
            selection.kind() == Kind::Operator
                && selection.exposure() == Exposure::PrivateImplementation
                && selection.compiler_partition().is_none()
                && matches!(
                    selection.target(),
                    Target::LateBound(LateBinding::CheckedOperator)
                )
                && records.iter().any(|record| {
                    record
                        .receipt
                        .builtin_operators
                        .contains(&selection.source_span())
                })
        })
        .map(|selection| (selection.occurrence_id(), selection.source_span()))
        .collect::<Vec<_>>();
    for (occurrence, reference) in occurrences {
        program
            .finalize_intrinsic_authored_declaration_selection(
                occurrence,
                LateBinding::CheckedOperator,
                Intrinsic::BuiltinOperator,
            )
            .map_err(|reason| {
                error(
                    reference,
                    &format!("retained builtin selection finalization failed: {reason:?}"),
                )
            })?;
    }
    Ok(())
}

fn attach_custody(
    program: &mut SymbolResolvedTrees,
    root: ExpressionHandle,
    custody: &ConstInitializerCustody,
    available: &super::initializer_dependencies::ConstInitializerDependencies,
    declaration: SourceSpan,
) -> Result<InitializerRootCustody, Diagnostic> {
    let mut occurrences = Vec::new();
    for (reference, selected_source) in &custody.call_selections {
        let mut candidates = available.call_targets.iter().filter(|candidate| {
            candidate.reference == *reference && candidate.declaration == *selected_source
        });
        let candidate = candidates.next().ok_or_else(|| {
            error(
                *reference,
                "realized call has no exact source declaration dependency",
            )
        })?;
        if candidates.next().is_some() {
            return Err(error(
                *reference,
                "realized call has ambiguous source declaration custody",
            ));
        }
        let original = {
            let mut retained = program
                .authored_declaration_selections()
                .iter()
                .filter(|row| {
                    row.source_span() == *reference
                        && row.exposure() == Exposure::PrivateImplementation
                        && row.kind() == Kind::Call
                        && row.compiler_partition().is_none()
                });
            let original = retained.next().copied();
            if retained.next().is_some() {
                return Err(error(
                    *reference,
                    "initializer call has duplicate source occurrences",
                ));
            }
            original
        };
        let occurrence = if let Some(original) = original {
            match original.target() {
                Target::LateBound(LateBinding::CheckedCall) => program
                    .finalize_authored_declaration_selection(
                        original.occurrence_id(),
                        LateBinding::CheckedCall,
                        candidate.target,
                    )
                    .map_err(|reason| {
                        error(
                            *reference,
                            &format!("initializer call selection finalization failed: {reason:?}"),
                        )
                    })?,
                Target::Resolved(selected) if selected.selected_symbol() == candidate.target => {}
                _ => {
                    return Err(error(
                        *reference,
                        "initializer call conflicts with its retained source selection",
                    ));
                }
            }
            original.occurrence_id()
        } else {
            program
                .record_resolved_authored_declaration_selection(
                    *reference,
                    Exposure::PrivateImplementation,
                    Kind::Call,
                    candidate.target,
                )
                .map_err(
                    crate::resolution::constants::selections::const_selection_record_diagnostic,
                )?
        };
        occurrences.push(occurrence);
    }
    for origin in &custody.selections {
        let mut candidates = program.const_declarations.iter().filter(|candidate| {
            program.symbols.symbol_source_span(candidate.symbol) == Some(origin.declaration)
        });
        let selected = candidates
            .next()
            .ok_or_else(|| error(origin.reference, "selected declaration disappeared"))?
            .symbol;
        if candidates.next().is_some() {
            return Err(error(
                origin.reference,
                "selected declaration has ambiguous source custody",
            ));
        }
        occurrences.push(
            program
                .record_resolved_authored_declaration_selection(
                    origin.reference,
                    Exposure::PrivateImplementation,
                    Kind::StaticPathSegment,
                    selected,
                )
                .map_err(
                    crate::resolution::constants::selections::const_selection_record_diagnostic,
                )?,
        );
    }
    for operator in &custody.builtin_operators {
        let occurrence = program
            .record_late_bound_authored_declaration_selection(
                *operator,
                Exposure::PrivateImplementation,
                Kind::Operator,
                LateBinding::CheckedOperator,
            )
            .map_err(crate::resolution::constants::selections::const_selection_record_diagnostic)?;
        program
            .finalize_intrinsic_authored_declaration_selection(
                occurrence,
                LateBinding::CheckedOperator,
                Intrinsic::BuiltinOperator,
            )
            .map_err(|reason| {
                error(
                    declaration,
                    &format!("builtin selection finalization failed: {reason:?}"),
                )
            })?;
        occurrences.push(occurrence);
    }
    let owner = InitializerRootCustody {
        root,
        reference: program.tables.bodies.expressions.source_span(root),
        declaration,
        occurrences,
    };
    replace_initializer_lanes(program, &owner)?;
    Ok(owner)
}

fn replace_initializer_lanes(
    program: &mut SymbolResolvedTrees,
    owner: &InitializerRootCustody,
) -> Result<(), Diagnostic> {
    if !body_expressions(program).expression_is_valid(owner.root)
        || program.tables.bodies.expressions.source_span(owner.root) != owner.reference
    {
        return Err(error(
            owner.declaration,
            "initializer substitution changed its exact custody root",
        ));
    }
    let mut retained = Vec::new();
    for occurrence in body_expressions(program).authored_selection_occurrences(owner.root) {
        let selection = program
            .authored_declaration_selections()
            .get(occurrence)
            .ok_or_else(|| {
                error(
                    owner.declaration,
                    "initializer root lost an attached selection",
                )
            })?;
        let initializer_lane = matches!(selection.kind(), Kind::Call | Kind::Operator)
            || (selection.kind() == Kind::StaticPathSegment
                && matches!(selection.target(), Target::Resolved(selected)
                if program.symbols.get(selected.selected_symbol()).kind == SymbolKind::Const));
        if !initializer_lane {
            retained.push(occurrence);
        }
    }
    retained.extend(owner.occurrences.iter().copied());
    program
        .tables
        .bodies
        .expressions
        .replace_authored_selection_occurrences(owner.root, retained);
    Ok(())
}

fn same_custody(expected: &ConstInitializerCustody, actual: &ConstInitializerCustody) -> bool {
    expected.selections.len() == actual.selections.len()
        && expected
            .selections
            .iter()
            .all(|value| actual.selections.contains(value))
        && expected.builtin_operators.len() == actual.builtin_operators.len()
        && expected
            .builtin_operators
            .iter()
            .all(|value| actual.builtin_operators.contains(value))
        && expected.call_selections.len() == actual.call_selections.len()
        && expected
            .call_selections
            .iter()
            .all(|value| actual.call_selections.contains(value))
}

/*
    Describe a failed inventory comparison without printing constant payloads.
    Counts stay complete; only the first four source coordinates are retained.
*/
fn missing_dependency_prefix<Value: PartialEq, Description>(
    required: &[Value],
    available: &[Value],
    mut describe: impl FnMut(&Value) -> Description,
) -> (usize, Vec<Description>) {
    let mut count = 0usize;
    let mut prefix = Vec::new();
    for value in required {
        if !available.contains(value) {
            count += 1;
            if prefix.len() < 4 {
                prefix.push(describe(value));
            }
        }
    }
    (count, prefix)
}

pub(crate) fn finalize(
    program: &mut SymbolResolvedTrees,
    records: &[PendingInitializer],
) -> Result<Vec<InitializerRootCustody>, Diagnostic> {
    use super::initializer_dependencies::DependencyPurpose;
    use syntax_trees::item::ConstInitializerCustody;
    let mut roots = Vec::new();
    for (ordinal, record) in records.iter().enumerate() {
        let mut owners = program.const_declarations.iter().filter(|declaration| {
            program.symbols.symbol_source_span(declaration.symbol) == Some(record.declaration)
                && declaration.initializer == record.materialized
        });
        let owner = owners.next().ok_or_else(|| {
            error(
                record.declaration,
                "normalization lost its exact declaration owner",
            )
        })?;
        if owners.next().is_some() {
            return Err(error(
                record.declaration,
                "normalization has ambiguous declaration custody",
            ));
        }
        let declared_type = owner.declared_type.as_ref().ok_or_else(|| {
            error(
                record.declaration,
                "normalization lost its exact declared type",
            )
        })?;
        resolved_payload::validate(
            program,
            declared_type,
            record.materialized,
            &record.receipt.result_value,
        )
        .map_err(|reason| {
            error(
                record.declaration,
                &format!("materialized value is not canonical: {reason}"),
            )
        })?;
        let (authored_origins, authored_operators) =
            reconstruct(program, records, ordinal, DependencyPurpose::Authored)?;
        let authored_calls = declaration_custody(program, record, DependencyPurpose::Authored)?;
        let authored = ConstInitializerCustody {
            selections: authored_origins,
            builtin_operators: authored_operators,
            call_selections: authored_calls.calls.clone(),
        };
        let (origins, operators) = reconstruct(
            program,
            records,
            ordinal,
            DependencyPurpose::DeclarationReadiness,
        )?;
        let declaration_calls =
            declaration_custody(program, record, DependencyPurpose::DeclarationReadiness)?;
        let preparation = ConstInitializerCustody {
            selections: origins,
            builtin_operators: operators,
            call_selections: declaration_calls.calls.clone(),
        };
        if !same_custody(&authored, &record.receipt.authored_custody)
            || !same_custody(&preparation, &record.receipt.declaration_custody)
        {
            return Err(error(
                record.declaration,
                "authored or declaration readiness custody drifted",
            ));
        }
        let realized = ConstInitializerCustody {
            selections: record.receipt.selections.clone(),
            builtin_operators: record.receipt.builtin_operators.clone(),
            call_selections: record.receipt.call_selections.clone(),
        };
        if realized
            .selections
            .iter()
            .enumerate()
            .any(|(index, value)| realized.selections[..index].contains(value))
            || realized
                .builtin_operators
                .iter()
                .enumerate()
                .any(|(index, value)| realized.builtin_operators[..index].contains(value))
            || realized
                .call_selections
                .iter()
                .enumerate()
                .any(|(index, value)| realized.call_selections[..index].contains(value))
        {
            return Err(error(
                record.declaration,
                "realized custody contains duplicate occurrences",
            ));
        }
        if realized
            .selections
            .iter()
            .any(|value| !preparation.selections.contains(value))
            || realized
                .builtin_operators
                .iter()
                .any(|value| !preparation.builtin_operators.contains(value))
            || realized
                .call_selections
                .iter()
                .any(|value| !preparation.call_selections.contains(value))
        {
            let missing_selections = missing_dependency_prefix(
                &realized.selections,
                &preparation.selections,
                |origin| (origin.reference, origin.declaration, origin.initializer),
            );
            let missing_operators = missing_dependency_prefix(
                &realized.builtin_operators,
                &preparation.builtin_operators,
                |source| *source,
            );
            let missing_calls = missing_dependency_prefix(
                &realized.call_selections,
                &preparation.call_selections,
                |call| *call,
            );
            return Err(error(
                record.declaration,
                &format!(
                    "realized custody has no exact source preparation dependency: missing selections (count, source prefix) {missing_selections:?}; missing operators (count, source prefix) {missing_operators:?}; missing calls (count, source prefix) {missing_calls:?}"
                ),
            ));
        }
        roots.push(attach_custody(
            program,
            record.authored,
            &authored,
            &authored_calls,
            record.declaration,
        )?);
        roots.push(attach_custody(
            program,
            record.materialized,
            &realized,
            &declaration_calls,
            record.declaration,
        )?);
    }
    Ok(roots)
}

pub(crate) fn constant_initializer_owns_selection(
    syntax: &SyntaxTrees,
    reference: source_text::SourceSpan,
) -> bool {
    syntax.root_items().any(|item| {
        let syntax_trees::item::Item::Const(definition) = item else {
            return false;
        };
        definition
            .preparation()
            .normalization
            .as_ref()
            .is_some_and(|normalization| {
                normalization
                    .selections
                    .iter()
                    .any(|origin| origin.reference == reference)
                    || normalization.builtin_operators.contains(&reference)
                    || normalization
                        .call_selections
                        .iter()
                        .any(|selection| selection.0 == reference)
            })
    })
}
