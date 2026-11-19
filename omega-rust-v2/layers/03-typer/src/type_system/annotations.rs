use crate::contracts::effect_clauses;
use crate::declarations::resolved_source_file::ResolvedSourceFile;
use crate::evaluation::numeric_literals;
use crate::type_system::type_catalog::TypeCatalog;
use crate::{TypeError, ValueType};
use omega_parser::NodeKind;
use omega_resolver::{DeclarationId, Symbol};

impl<'a> ResolvedSourceFile<'a> {
    pub(crate) fn universe_level(
        &self,
        node_index: usize,
        types: &TypeCatalog<'_>,
    ) -> Result<ValueType, TypeError> {
        let source = &self.file;
        if let Some(Symbol::Binding {
            span,
            kind: NodeKind::KindedParam,
        }) = self.symbol(node_index)
            && let Some(value) = self.parameters.get(span)
            && types.parameter_carrier(value) == Some(&ValueType::LevelKind)
        {
            return Ok(value.clone());
        }
        if source.nodes[node_index].kind == NodeKind::Literal
            && let (
                ValueType::Exact {
                    value,
                    fractional_origin: None,
                },
                None,
            ) = numeric_literals::literal(source, node_index)?
            && value.is_integer()
            && let Some(level) = value.to_integer().to_biguint()
        {
            return Ok(ValueType::Level(level));
        }
        Err(TypeError::at(
            source,
            node_index,
            "expected a universe level binder or a nonnegative level literal",
        ))
    }
    pub fn annotation(
        &self,
        node_index: usize,
        types: &TypeCatalog<'_>,
    ) -> Result<ValueType, TypeError> {
        self.annotation_for(node_index, types, None)
    }
    pub fn annotation_for(
        &self,
        node_index: usize,
        types: &TypeCatalog<'_>,
        candidate: Option<&DeclarationId>,
    ) -> Result<ValueType, TypeError> {
        let raw = self.raw_type(node_index, candidate, types)?;
        types
            .normalize_at(raw, &self.file, node_index)
            .map_err(|reason| TypeError::at(&self.file, node_index, reason))
    }
    pub(crate) fn raw_type(
        &self,
        node_index: usize,
        candidate: Option<&DeclarationId>,
        types: &TypeCatalog<'_>,
    ) -> Result<ValueType, TypeError> {
        use NodeKind::*;
        let source = &self.file;
        let node = &source.nodes[node_index];
        match node.kind {
            FunctionType => {
                let [input, out] = *node.children.as_slice() else {
                    return Err(TypeError::unsupported(source, node_index));
                };
                let codomain = self.raw_type(out, candidate, types)?;
                if source.nodes[input].kind == Param {
                    let name = source.nodes[input]
                        .words()
                        .next()
                        .ok_or_else(|| TypeError::unsupported(source, input))?;
                    Ok(ValueType::Function(
                        Box::new(self.raw_type(self.single_child(input)?, candidate, types)?),
                        Box::new(codomain.bind_subject(&self.binder_site(name.span))),
                    ))
                } else {
                    Ok(ValueType::Function(Box::new(self.raw_type(input, candidate, types)?), Box::new(codomain)))
                }
            }
            FamilyApplication => {
                let [head, args] = *node.children.as_slice() else {
                    return Err(TypeError::unsupported(source, node_index));
                };
                let (family, mut ty) = match self.symbol(head) {
                    Some(Symbol::Declaration(id)) if id.kind == MathLet => {
                        let signature = types
                            .mathematical
                            .get(id)
                            .ok_or_else(|| TypeError::at(source, head, "mathematical family signature is unresolved"))?;
                        (ValueType::MathematicalDefinition(Box::new(id.clone())), signature.clone())
                    }
                    Some(Symbol::Binding { span, .. }) => {
                        if let Some(param) = self.parameters.get(span) {
                            (
                                param.clone(),
                                types
                                    .parameter_carrier(param)
                                    .cloned()
                                    .ok_or_else(|| TypeError::at(source, head, "untyped mathematical family"))?,
                            )
                        } else {
                            let annotation = self
                                .binding_annotations
                                .get(span)
                                .ok_or_else(|| TypeError::unsupported(source, head))?;
                            (
                                self.term_subject(head, &ValueType::NoResult, None),
                                self.raw_type(*annotation, candidate, types)?,
                            )
                        }
                    }
                    _ => return Err(TypeError::at(source, head, "expected a mathematical family")),
                };
                let mut subjects = Vec::new();
                for &arg in &source.nodes[args].children {
                    let ValueType::Function(input, out) = ty else {
                        return Err(TypeError::at(source, arg, "excess type-family arguments"));
                    };
                    let subject = self.mathematical_subject(arg, &input, types, None)?;
                    ty = out.apply_subject(&subject);
                    subjects.push(subject);
                }
                if !matches!(ty, ValueType::Universe { .. }) {
                    return Err(TypeError::at(source, node_index, "type family must return a universe"));
                }
                Ok(ValueType::FamilyApplication {
                    family: Box::new(family),
                    subjects,
                })
            }
            MachineType => {
                let mut params = Vec::new();
                let mut result = ValueType::NoResult;
                let mut closed = [false; 2];
                for &part in &node.children {
                    match source.nodes[part].kind {
                        Params => {
                            for &p in &source.nodes[part].children {
                                params.push(self.raw_type(self.single_child(p)?, candidate, types)?);
                            }
                        }
                        Result => result = self.raw_type(self.single_child(part)?, candidate, types)?,
                        Reaches => closed[0] = true,
                        Crashes => closed[1] = true,
                        Requires | Ensures | Blocks | Suspends => {}
                        Terminates if source.nodes[part].children.is_empty() => {}
                        _ => return Err(TypeError::unsupported(source, part)),
                    }
                }
                // Only the bound on a static callable may leave these effects open.
                // Its own runtime parameters and result still need closed contracts.
                let binder_bound = node
                    .parent
                    .is_some_and(|p| matches!(source.nodes[p].kind, ConstParam | KindedParam));
                if !binder_bound && closed != [true, true] {
                    let missing = match closed {
                        [true, false] => "an explicit crash ceiling",
                        [false, true] => "an explicit reach ceiling",
                        _ => "explicit reach and crash ceilings",
                    };
                    return Err(TypeError::at(source, node_index, format!("runtime callable requires {missing}")));
                }
                Ok(ValueType::Callable(Box::new(crate::CallableType {
                    requirement: None,
                    parameters: params,
                    result,
                    contract: crate::Site::at(source, node_index),
                    closed_runtime: closed == [true, true],
                    pause: effect_clauses::DeclaredPauseBounds::read(self, node_index)?.declared(),
                    has_guarantee: source.child(node_index, Ensures).is_some(),
                    reach: effect_clauses::reach(self, types, node_index)?,
                    terminates: source.child(node_index, Terminates).is_some(),
                    crash_ceiling: effect_clauses::crashes(self, node_index)?,
                })))
            }
            Lifetime => {
                let Some(Symbol::Binding { span, kind: LifetimeParam }) = self.symbol(node_index) else {
                    return Err(TypeError::at(source, node_index, "unresolved lifetime"));
                };
                self.parameters
                    .get(span)
                    .cloned()
                    .ok_or_else(|| TypeError::unsupported(source, node_index))
            }
            ReferenceType if source.child(node_index, Lifetime).is_some() => {
                let [region, target] = *node.children.as_slice() else {
                    return Err(TypeError::unsupported(source, node_index));
                };
                let reference = ValueType::Reference(self.access(node_index), Box::new(self.raw_type(target, candidate, types)?));
                Ok(ValueType::AtLifetime(Box::new(reference), Box::new(self.raw_type(region, candidate, types)?)))
            }
            QualifiedType => {
                let (base, atoms) = node
                    .children
                    .split_first()
                    .ok_or_else(|| TypeError::unsupported(source, node_index))?;
                let carrier = self.raw_type(*base, candidate, types)?;
                let domains = atoms
                    .iter()
                    .map(|&d| self.domain_application(d, &carrier, types))
                    .collect::<std::result::Result<_, _>>()?;
                Ok(ValueType::Qualified(Box::new(carrier), domains))
            }
            ReferenceType | SliceType => {
                let inner = Box::new(self.raw_type(self.single_child(node_index)?, candidate, types)?);
                Ok(if node.kind == ReferenceType {
                    ValueType::Reference(self.access(node_index), inner)
                } else {
                    ValueType::Slice(inner)
                })
            }
            ArrayType => {
                let [element, count] = *node.children.as_slice() else {
                    return Err(TypeError::unsupported(source, node_index));
                };
                if let Some(Symbol::Binding { span, kind: ConstParam }) = self.symbol(count) {
                    let parameter = self
                        .parameters
                        .get(span)
                        .ok_or_else(|| TypeError::unsupported(source, count))?;
                    if types.parameter_carrier(parameter) != ValueType::integer("u64").as_ref() {
                        return Err(TypeError::at(source, count, "array extent needs a static u64"));
                    }
                    return Ok(ValueType::IndexedArray(
                        Box::new(self.raw_type(element, candidate, types)?),
                        Box::new(parameter.clone()),
                    ));
                }
                if let Some(term) = self.open_index(types, count, &ValueType::integer("u64").unwrap())? {
                    return Ok(ValueType::IndexedArray(Box::new(self.raw_type(element, candidate, types)?), Box::new(term)));
                }
                let size = self.static_size(types, count)?;
                Ok(ValueType::Array(Box::new(self.raw_type(element, candidate, types)?), size))
            }
            PathType | PathExpression => {
                let symbol = match self.symbol(node_index) {
                    Some(Symbol::ByOverload(options)) => options
                        .iter()
                        .find(|o| Some(&o.candidate) == candidate)
                        .and_then(|o| o.target.as_deref()),
                    other => other,
                };
                if matches!(symbol,Some(Symbol::Builtin(name)) if name=="core::Squash") {
                    let section = self.single_child(node_index)?;
                    let argument = self.single_child(section)?;
                    return Ok(ValueType::Squash(Box::new(self.raw_type(argument, candidate, types)?)));
                }
                if let Some(Symbol::Builtin(name)) = symbol
                    && matches!(name.as_str(), "core::Type" | "core::Strict")
                {
                    let section = self
                        .single_child(node_index)
                        .map_err(|_| TypeError::at(source, node_index, "universe requires one level"))?;
                    let level = self.universe_level(self.single_child(section)?, types)?;
                    return Ok(ValueType::Universe {
                        strict: name == "core::Strict",
                        level: Box::new(level),
                    });
                }
                if let Some(Symbol::Builtin(name)) = symbol
                    && matches!(name.as_str(), "u" | "i")
                    && node.children.len() == 1
                {
                    let arg = self.single_child(node.children[0])?;
                    if let Some(Symbol::Binding {
                        span,
                        kind: ConstParam | KindedParam,
                    }) = self.symbol(arg)
                    {
                        let width = self
                            .parameters
                            .get(span)
                            .ok_or_else(|| TypeError::unsupported(source, arg))?;
                        if types.parameter_carrier(width) != ValueType::integer("u64").as_ref() {
                            return Err(TypeError::at(source, arg, "integer width requires a static u64"));
                        }
                        return Ok(ValueType::IndexedInteger {
                            signed: name == "i",
                            width: Box::new(width.clone()),
                        });
                    }
                    let required = ValueType::integer("u64").unwrap();
                    if let Some(width) = self.open_index(types, arg, &required)? {
                        return Ok(ValueType::IndexedInteger {
                            signed: name == "i",
                            width: Box::new(width),
                        });
                    }
                    let (number, suffix) = self.closed_number(types, arg, &required)?;
                    if suffix.as_ref().is_some_and(|actual| actual != &required) {
                        return Err(TypeError::at(source, arg, "integer width requires a static u64"));
                    }
                    let width = numeric_literals::array_length(source, arg, number, suffix)?;
                    if width == 0 {
                        return Err(TypeError::at(source, arg, "integer width must be positive"));
                    }
                    return Ok(ValueType::Integer {
                        signed: name == "i",
                        width: Some(width),
                    });
                }
                if let Some(Symbol::Declaration(id)) = symbol
                    && id.kind == Data
                {
                    return self.data_application(node_index, id, types, candidate);
                }
                if let Some(Symbol::Declaration(id)) = symbol
                    && id.kind == Domain
                {
                    let last = node.words().next_back().unwrap().span.get_end_offset();
                    let args = node
                        .children
                        .iter()
                        .filter(|&&section| source.span(section).get_start_offset() >= last)
                        .flat_map(|&a| source.nodes[a].children.iter().copied())
                        .enumerate()
                        .map(|(i, a)| self.raw_argument(a, types, id, types.domains.index_binder(id, i), candidate))
                        .collect::<std::result::Result<Vec<_>, _>>()?;
                    if let Some(carrier) = types.domains.known_carrier(id) {
                        return Ok(ValueType::Qualified(
                            Box::new(self.explicit_domain_carrier(node_index, carrier, types, candidate)?),
                            vec![crate::DomainApplication {
                                declaration: id.clone(),
                                arguments: args,
                            }],
                        ));
                    }
                    return Ok(ValueType::Applied(Box::new(id.clone()), args));
                }
                let mut args = Vec::new();
                for &section in &node.children {
                    if !matches!(source.nodes[section].kind, TypeArgs | ExpressionArgs) {
                        return Err(TypeError::unsupported(source, section));
                    }
                    for (i, &a) in source.nodes[section].children.iter().enumerate() {
                        args.push(match symbol {
                            Some(Symbol::Declaration(owner)) => self.raw_argument(a, types, owner, i, candidate)?,
                            _ => self.raw_type(a, candidate, types)?,
                        });
                    }
                }
                match symbol {
                    Some(Symbol::Declaration(id)) if id.kind == Data => Ok(if args.is_empty() {
                        ValueType::Nominal(Box::new(id.clone()))
                    } else {
                        ValueType::Applied(Box::new(id.clone()), args)
                    }),
                    Some(Symbol::Binding {
                        span,
                        kind: KindedParam | ConstParam,
                    }) if args.is_empty()
                        && self
                            .parameters
                            .get(span)
                            .and_then(|p| types.parameter_carrier(p))
                            .is_some_and(|t| matches!(t, ValueType::Universe { .. })) =>
                    {
                        Ok(self.parameters[span].clone())
                    }
                    Some(Symbol::Binding {
                        span,
                        kind: TypeParam | SomeParam,
                    }) if args.is_empty() => self
                        .parameters
                        .get(span)
                        .cloned()
                        .ok_or_else(|| TypeError::at(source, node_index, "unknown type parameter")),
                    Some(Symbol::Builtin(name)) if args.is_empty() => match name.as_str() {
                        "core::Level" if node.parent.is_some_and(|p| source.nodes[p].kind == KindedParam) => Ok(ValueType::LevelKind),
                        "core::Level" => Err(TypeError::at(source, node_index, "core::Level is a universe binder kind")),
                        "addr" => Ok(ValueType::Address),
                        "bool" => Ok(ValueType::Bool),
                        "f32" => Ok(ValueType::Float32),
                        "f64" => Ok(ValueType::Float64),
                        _ if name.starts_with(['u', 'i']) => {
                            ValueType::integer(name).ok_or_else(|| TypeError::at(source, node_index, format!("invalid integer carrier {name}")))
                        }
                        _ => Err(TypeError::unsupported(source, node_index)),
                    },
                    _ => Err(TypeError::at(source, node_index, "expected a type declaration or type parameter")),
                }
            }
            // Keep annotation formation exhaustive. Every new container must choose how
            // to form its children, including a runtime callable nested inside it.
            File | Fragment | Use | Const | Data | Field | Case | CaseField | StableNumber | Attributes | Attribute | DroppedBy | Domain | Access | Trait
            | Restriction | Conformance | SelectionRow | Hypothesis | Machine | AsmMachine | OperatorToken | MathLet | Path | GenericParams | TypeParam
            | KindedParam | ConstParam | MachineParam | SomeParam | LifetimeParam | Params | Param | Result | Body | Requires | Ensures | Reaches | Crashes
            | Blocks | Suspends | Terminates | Invokes | Satisfies | Registers | Clobbers | TypeArgs | Let | Assign | Discard | ExpressionStatement | Tail
            | AsmBlock | Instruction | AsmTransition | Match | Transition | Arm | WildcardPattern | BindingPattern | RecordPattern | FieldPattern
            | RestPattern | TuplePattern | ParenPattern | ValuePattern | Literal | ExpressionArgs | Paren | Tuple | RecordLiteral | FieldInit | Omission
            | ArrayLiteral | Selection | Call | Args | FieldAccess | Index | Range | Borrow | Unary | Move | CallMarker | Cast | CastEntry | Binary
            | Unfinished => Err(TypeError::unsupported(source, node_index)),
        }
    }
}
