use crate::contracts::effect_clauses;
use crate::declarations::resolved_source_file::ResolvedSourceFile;
use crate::evaluation::numeric_literals;
use crate::type_system::type_catalog::TypeCatalog;
use crate::{TypeError, ValueType};
use omega_parser::{NodeKind, TerminationClause};
use omega_resolver::Symbol;

impl<'a> ResolvedSourceFile<'a> {
    pub(crate) fn universe_level(
        &self,
        node_index: usize,
        types: &TypeCatalog<'_>,
    ) -> Result<ValueType, TypeError> {
        let source = self.file;
        if let Some(Symbol::Binding {
            span,
            kind: NodeKind::KindedParam,
            ..
        }) = self.name_in_role(node_index, false)
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
    fn integer_type(
        &self,
        types: &TypeCatalog<'_>,
        node: usize,
        section: usize,
        signed: bool,
    ) -> Result<ValueType, TypeError> {
        let arg = self.single_child(section)?;
        let width = if let Some(width) = self.open_size(types, arg)? {
            width
        } else {
            let required = ValueType::integer("u64").unwrap();
            let (number, suffix) = self.closed_number(types, arg, &required)?;
            if matches!(number, ValueType::StaticFormula { .. }) {
                number
            } else {
                if suffix.as_ref().is_some_and(|actual| actual != &required) {
                    return Err(TypeError::at(self.file, arg, "integer width requires a static u64"));
                }
                ValueType::static_size(numeric_literals::array_length(self.file, arg, number, suffix)?)
            }
        };
        let formed = ValueType::with_width(signed, width);
        formed
            .validate_closed_indices()
            .map_err(|message| TypeError::at(self.file, arg, message))?;
        if matches!(formed, ValueType::IndexedInteger { .. }) {
            types.formations.index(self, node, formed.clone());
        }
        Ok(formed)
    }
    pub fn annotation(
        &self,
        node_index: usize,
        types: &TypeCatalog<'_>,
    ) -> Result<ValueType, TypeError> {
        let raw = self.raw_type(node_index, types)?;
        types
            .normalize_at(raw, self, node_index)
            .map_err(|reason| TypeError::at(self.file, node_index, reason))
    }
    pub(crate) fn raw_type(
        &self,
        node_index: usize,
        types: &TypeCatalog<'_>,
    ) -> Result<ValueType, TypeError> {
        use NodeKind::*;
        let source = self.file;
        let node = &source.nodes[node_index];
        match node.kind {
            FunctionType => {
                let [input, out] = *node.children.as_slice() else {
                    return Err(TypeError::unsupported(source, node_index));
                };
                let codomain = self.raw_type(out, types)?;
                if source.nodes[input].kind == Param {
                    let name = source.nodes[input]
                        .words()
                        .next()
                        .ok_or_else(|| TypeError::unsupported(source, input))?;
                    Ok(ValueType::Function(
                        Box::new(self.raw_type(self.single_child(input)?, types)?),
                        Box::new(codomain.bind_subject(&self.binder_site(name.span))),
                    ))
                } else {
                    Ok(ValueType::Function(Box::new(self.raw_type(input, types)?), Box::new(codomain)))
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
                    Some(Symbol::Binding { span, binder_node, .. }) => {
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
                                .parameter_annotation(*binder_node)
                                .ok_or_else(|| TypeError::unsupported(source, head))?;
                            (self.term_subject(head, &ValueType::NoResult, None), self.raw_type(annotation, types)?)
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
                let params = source
                    .parameters(node_index)
                    .map(|(_, ty)| self.raw_type(ty, types))
                    .collect::<std::result::Result<_, _>>()?;
                let result = source
                    .result_annotation(node_index)
                    .map_or(Ok(ValueType::NoResult), |ty| self.raw_type(ty, types))?;
                let mut closed = [false; 2];
                for &part in &node.children {
                    match source.nodes[part].kind {
                        Params | Result => {}
                        Reaches => closed[0] = true,
                        Crashes => closed[1] = true,
                        Requires | Ensures | Blocks | Suspends => {}
                        Terminates if source.nodes[part].termination_clause() == Some(TerminationClause::Bare) => {}
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
                    pause: crate::DeclaredPauseBounds::read(self, node_index)?.declared(),
                    has_guarantee: source.child(node_index, Ensures).is_some(),
                    requires: source.child(node_index, Requires).is_some(),
                    reach: effect_clauses::reach(self, types, node_index)?,
                    terminates: source.child(node_index, Terminates).is_some(),
                    crash_ceiling: effect_clauses::crashes(self, node_index)?,
                })))
            }
            Lifetime => {
                let Some(Symbol::Binding { span, kind: LifetimeParam, .. }) = self.symbol(node_index) else {
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
                let reference = ValueType::Reference(self.access(node_index), Box::new(self.raw_type(target, types)?));
                Ok(ValueType::AtLifetime(Box::new(reference), Box::new(self.raw_type(region, types)?)))
            }
            QualifiedType => {
                let (base, atoms) = node
                    .children
                    .split_first()
                    .ok_or_else(|| TypeError::unsupported(source, node_index))?;
                let carrier = self.raw_type(*base, types)?;
                let domains = atoms
                    .iter()
                    .map(|&d| self.domain_application(d, &carrier, types))
                    .collect::<std::result::Result<_, _>>()?;
                Ok(ValueType::Qualified(Box::new(carrier), domains))
            }
            ReferenceType | SliceType => {
                let inner = Box::new(self.raw_type(self.single_child(node_index)?, types)?);
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
                let size = match self.open_size(types, count)? {
                    Some(term) => term,
                    None => self.closed_size(types, count)?,
                };
                Ok(ValueType::array_with_extent(self.raw_type(element, types)?, size))
            }
            PathType | PathExpression => {
                let symbol = self.name_in_role(node_index, true);
                let sections = self.file.reference_sections(node_index);
                if matches!(symbol,Some(Symbol::Builtin(name)) if name=="core::Squash") {
                    let [section] = *sections else {
                        return Err(TypeError::unsupported(source, node_index));
                    };
                    let argument = self.single_child(section)?;
                    return Ok(ValueType::Squash(Box::new(self.raw_type(argument, types)?)));
                }
                if let Some(Symbol::Builtin(name)) = symbol
                    && matches!(name.as_str(), "core::Type" | "core::Strict")
                {
                    let [section] = *sections else {
                        return Err(TypeError::at(source, node_index, "universe requires one level"));
                    };
                    let level = self.universe_level(self.single_child(section)?, types)?;
                    return Ok(ValueType::Universe {
                        strict: name == "core::Strict",
                        level: Box::new(level),
                    });
                }
                if let Some(Symbol::Builtin(name)) = symbol
                    && matches!(name.as_str(), "u" | "i")
                    && let [section] = *sections
                {
                    return self.integer_type(types, node_index, section, name == "i");
                }
                if let Some(Symbol::Declaration(schema)) = symbol
                    && schema.kind == Trait
                {
                    return Ok(ValueType::Evidence(Box::new(self.trait_application(node_index, schema, types)?)));
                }
                if let Some(Symbol::Declaration(id)) = symbol
                    && id.kind == Data
                {
                    return self.data_application(node_index, id, types);
                }
                if symbol.is_some_and(Symbol::names_domain) {
                    let (id, written) = self.domain_reference(node_index, None, types)?;
                    if let Some(declared) = types.domains.known_carrier(id) {
                        let carrier = written.unwrap_or_else(|| declared.clone());
                        let application = self.indexed_domain(node_index, id, &carrier, types)?;
                        return Ok(ValueType::Qualified(Box::new(carrier), vec![application]));
                    }
                    let args = self
                        .arguments_for(node_index, id)
                        .iter()
                        .copied()
                        .enumerate()
                        .map(|(i, a)| self.raw_argument(a, types, id, types.domains.index_binder(id, i)))
                        .collect::<std::result::Result<Vec<_>, _>>()?;
                    return Ok(ValueType::Applied(Box::new(id.clone()), args));
                }
                if !sections.is_empty() {
                    return Err(TypeError::at(source, node_index, "generic arguments require an applicable type declaration"));
                }
                match symbol {
                    Some(Symbol::Binding {
                        span,
                        kind: KindedParam | ConstParam,
                        ..
                    }) if self
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
                        ..
                    }) => self
                        .parameters
                        .get(span)
                        .cloned()
                        .ok_or_else(|| TypeError::at(source, node_index, "unknown type parameter")),
                    Some(Symbol::Builtin(name)) => match name.as_str() {
                        "core::Level" if node.parent.is_some_and(|p| source.nodes[p].kind == KindedParam) => Ok(ValueType::LevelKind),
                        "core::Level" => Err(TypeError::at(source, node_index, "core::Level is a universe binder kind")),
                        _ => ValueType::builtin_scalar(name).ok_or_else(|| {
                            if name.starts_with(['u', 'i']) {
                                TypeError::at(source, node_index, format!("invalid integer carrier {name}"))
                            } else {
                                TypeError::unsupported(source, node_index)
                            }
                        }),
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
