//! Elaborate resolved Omega source into typed declarations, selections and proof obligations.
#![forbid(unsafe_code)]
mod bodies;
mod callback_bounds;
mod collections;
mod conformance;
mod constants;
pub use collections::collection_contracts;
mod cleanup;
mod dependent;
mod domains;
mod effects;
mod hypotheses;
mod standard;
pub use standard::{resolve_source_package, type_source_package};
mod static_program;
pub use static_program::IntegerRoutine;
mod deferred_indices;
pub use deferred_indices::StaticTemplate;
mod index_terms;
mod integers;
mod invocations;
pub use index_terms::IntegerIndex;
mod model;
mod numeric;
mod properties;
mod refinements;
mod types;
pub use model::*;
use omega_lexer::SourceSpan;
use omega_parser::NodeKind;
use omega_resolver::{DeclarationId, IndexedSource, ResolvedStage, StageSource, Symbol};
use std::collections::{HashMap, HashSet};
use types::Types;

type Names<'a> = HashMap<SourceSpan, &'a Symbol>;
struct Signature {
    params: Vec<(SourceSpan, ValueType)>,
    result: ValueType,
    requires: bool,
    terminates: bool,
    site: Site,
    satisfies: Vec<RequirementSlot>,
    public: bool,
    crash_ceiling: Option<Vec<CrashCause>>,
    pause: effects::Envelope,
    reach: Option<Vec<ServiceApplication>>,
}
#[derive(Default)]
struct Catalog<'a> {
    types: Types<'a>,
    assembly: Option<&'a AssemblyTypes>,
    ranking_views: &'a [RankingView],
    overloads: HashMap<(&'a str, &'a str), Vec<&'a DeclarationId>>,
    signatures: HashMap<&'a DeclarationId, Signature>,
    data_properties: HashMap<&'a DeclarationId, DataProperties>,
    fields: HashMap<&'a DeclarationId, HashMap<String, ValueType>>,
    field_order: HashMap<&'a DeclarationId, Vec<String>>,
    cases: HashMap<&'a DeclarationId, Vec<&'a DeclarationId>>,
    case_owner: HashMap<&'a DeclarationId, &'a DeclarationId>,
    implementations: HashMap<&'a DeclarationId, RequirementSlot>,
    requirements: HashMap<&'a DeclarationId, Vec<&'a DeclarationId>>,
    refinements: HashMap<&'a DeclarationId, Vec<TraitRefinement>>,
    trait_parents: HashMap<&'a DeclarationId, RequirementSlot>,
    token_bindings: HashMap<&'a str, Vec<&'a omega_resolver::OperatorDeclaration>>,
    predicates: HashSet<&'a DeclarationId>,
    hypotheses: HashMap<Site, LogicalHypothesis>,
    assumptions: HashMap<DeclarationId, Vec<DeclarationId>>,

    constants: HashMap<&'a DeclarationId, ValueType>,
}
struct Unit<'a> {
    file: IndexedSource<'a>,
    names: Names<'a>,
    parameters: HashMap<SourceSpan, ValueType>,
    hypotheses: HashMap<SourceSpan, Site>,
    binding_annotations: HashMap<SourceSpan, usize>,
    definitions: HashMap<SourceSpan, &'a DeclarationId>,
}
impl<'a> Unit<'a> {
    fn declaration(
        &self,
        n: usize,
    ) -> Result<&'a DeclarationId, TypeError> {
        self.definitions
            .get(&self.file.span(n))
            .copied()
            .ok_or_else(|| TypeError::at(&self.file, n, "missing L2 definition"))
    }
    fn single_child(
        &self,
        n: usize,
    ) -> Result<usize, TypeError> {
        match self.file.nodes[n].children.as_slice() {
            [child] => Ok(*child),
            _ => Err(TypeError::unsupported(&self.file, n)),
        }
    }
    fn index_mathematical(
        &self,
        n: usize,
        types: &mut types::Types<'a>,
    ) -> Result<(), TypeError> {
        use NodeKind::*;
        let f = &self.file;
        let node = &f.nodes[n];
        let id = self.declaration(n)?;
        let mut parameters = Vec::new();
        let mut result = None;
        let equal = node
            .leaves
            .iter()
            .find(|l| l.text == "=")
            .map(|l| l.span.get_start_offset());
        for &part in &node.children {
            if equal.is_some_and(|offset| f.span(part).get_start_offset() > offset) {
                continue;
            }
            match f.nodes[part].kind {
                GenericParams => {}
                Params => {
                    for &p in &f.nodes[part].children {
                        let binder = f.nodes[p]
                            .words()
                            .next()
                            .ok_or_else(|| TypeError::unsupported(f, p))?;
                        parameters.push((self.binder_site(binder.span), self.annotation(self.single_child(p)?, types)?));
                    }
                }
                _ => result = Some(self.annotation(part, types)?),
            }
        }
        let ty = parameters
            .into_iter()
            .rev()
            .fold(result.ok_or_else(|| TypeError::unsupported(f, n))?, |out, (binder, p)| {
                ValueType::Function(Box::new(p), Box::new(out.bind_subject(&binder)))
            });
        types.mathematical.insert(id, ty);

        Ok(())
    }
    fn collect(
        &self,
        catalog: &mut Catalog<'a>,
        sealed: bool,
    ) -> Result<(), TypeError> {
        use NodeKind::*;
        let f = &self.file;
        for (n, node) in f.nodes.iter().enumerate().skip(1) {
            if node.parent != Some(0) && !matches!(node.kind, Data | Case | Machine | AsmMachine) {
                continue;
            }
            if f.nodes[0].kind == Fragment && matches!(node.kind, Let | Assign | Discard | ExpressionStatement | Tail | Transition | ConstParam) {
                continue;
            }
            match f.nodes[n].kind {
                Use => {}
                Domain => {
                    let id = self.declaration(n)?;
                    catalog
                        .types
                        .domains
                        .expanded(
                            &DomainApplication {
                                declaration: id.clone(),
                                arguments: catalog
                                    .types
                                    .formals(id)
                                    .into_iter()
                                    .take(catalog.types.domains.index_arity(id))
                                    .collect(),
                            },
                            catalog.types.domains.carrier(id),
                            true,
                        )
                        .map_err(|m| TypeError::at(f, n, m))?;
                }
                Trait => {
                    let id = self.declaration(n)?;
                    let requirements = catalog.requirements.entry(id).or_default();
                    if node.leaves.iter().any(|l| l.text == "=") {
                        catalog.refinements.entry(id).or_default();
                    }
                    for &part in &node.children {
                        match f.nodes[part].kind {
                            GenericParams => {}
                            PathType => {
                                let Some(Symbol::Declaration(parent)) = self.symbol(part) else {
                                    return Err(TypeError::at(f, part, "unresolved parent trait"));
                                };
                                let arguments = f
                                    .child(part, TypeArgs)
                                    .into_iter()
                                    .flat_map(|a| f.nodes[a].children.iter().copied())
                                    .enumerate()
                                    .map(|(index, arg)| self.argument(arg, &catalog.types, parent, index))
                                    .collect::<std::result::Result<Vec<_>, _>>()?;
                                if parent.kind != Trait || arguments.len() != catalog.types.arities[parent] {
                                    return Err(TypeError::at(f, part, "invalid parent trait application"));
                                }
                                catalog.trait_parents.insert(
                                    id,
                                    RequirementSlot {
                                        declaration: parent.clone(),
                                        trait_arguments: arguments,
                                    },
                                );
                            }
                            Restriction => catalog
                                .refinements
                                .entry(id)
                                .or_default()
                                .push(refinements::read(self, &catalog.types, id, part)?),
                            Machine => requirements.push(self.declaration(part)?),
                            _ => return Err(TypeError::unsupported(f, part)),
                        }
                    }
                }
                Conformance => {
                    let head = f
                        .child(n, PathType)
                        .ok_or_else(|| TypeError::unsupported(f, n))?;
                    let Some(Symbol::Declaration(schema)) = self.symbol(head) else {
                        return Err(TypeError::unsupported(f, head));
                    };
                    let args = f
                        .child(head, TypeArgs)
                        .into_iter()
                        .flat_map(|a| &f.nodes[a].children)
                        .enumerate()
                        .map(|(i, &a)| self.argument(a, &catalog.types, schema, i))
                        .collect::<std::result::Result<_, _>>()?;
                    catalog.implementations.insert(
                        self.declaration(n)?,
                        RequirementSlot {
                            declaration: schema.clone(),
                            trait_arguments: args,
                        },
                    );
                    for &part in &node.children {
                        if !matches!(f.nodes[part].kind, GenericParams | PathType | SelectionRow | Machine) {
                            return Err(TypeError::unsupported(f, part));
                        }
                    }
                }
                MathLet => {}
                Const => {
                    if node.children.len() == 2 {
                        continue;
                    }
                    let [_, annotation, _] = *node.children.as_slice() else {
                        return Err(TypeError::unsupported(f, n));
                    };
                    catalog
                        .constants
                        .insert(self.declaration(n)?, self.annotation(annotation, &catalog.types)?);
                }
                Data | Case => {
                    if node.leaves.iter().any(|w| w.text == "=") {
                        self.annotation(*node.children.last().unwrap(), &catalog.types)?;
                        continue;
                    }
                    let id = self.declaration(n)?;
                    if node.kind == Case {
                        let parent = self.declaration(node.parent.unwrap())?;
                        catalog.cases.entry(parent).or_default().push(id);
                        catalog.case_owner.insert(id, parent);
                    }
                    let fields = catalog.fields.entry(id).or_default();
                    let order = catalog.field_order.entry(id).or_default();
                    for &field in &node.children {
                        match f.nodes[field].kind {
                            GenericParams | StableNumber | Case | Data | DroppedBy => continue,
                            Requires => {
                                catalog.predicates.insert(id);
                                continue;
                            }
                            Attributes if node.kind == Data => {
                                catalog
                                    .data_properties
                                    .insert(id, properties::read(self, field)?);
                                continue;
                            }
                            Field | CaseField => {}
                            _ => return Err(TypeError::unsupported(f, field)),
                        }
                        let name = f.nodes[field]
                            .words()
                            .next()
                            .ok_or_else(|| TypeError::unsupported(f, field))?;
                        let mut annotation = None;
                        for &i in &f.nodes[field].children {
                            match f.nodes[i].kind {
                                StableNumber => continue,
                                Data => annotation = Some(catalog.types.open_data(self.declaration(i)?)),
                                _ => annotation = Some(self.annotation(i, &catalog.types)?),
                            }
                        }
                        order.push(name.text.into());
                        fields.insert(name.text.into(), annotation.ok_or_else(|| TypeError::unsupported(f, field))?);
                    }
                    if node.kind == Data && !fields.is_empty() && node.children.iter().any(|&c| f.nodes[c].kind == Case) {
                        return Err(TypeError::at(f, n, "data cannot contain both fields and cases"));
                    }
                }
                Machine | AsmMachine => {
                    if node.kind == AsmMachine && catalog.assembly.is_none() {
                        return Err(TypeError::at(f, n, "assembly typing requires an admitted target"));
                    }
                    if f.nodes[n]
                        .words()
                        .any(|w| !matches!(w.text, "public" | "machine" | "asm"))
                    {
                        return Err(TypeError::unsupported(f, n));
                    }
                    let mut params = Vec::new();
                    let mut result = ValueType::NoResult;
                    for &section in &f.nodes[n].children {
                        match f.nodes[section].kind {
                            Path | Body | GenericParams | Requires | Ensures | OperatorToken | Blocks | Suspends | Reaches => {}
                            Crashes | Hypothesis => {}
                            Terminates | Invokes => {}
                            Registers | Clobbers | Instruction | AsmTransition | AsmMachine if node.kind == AsmMachine => {}
                            Satisfies => {}
                            Params => {
                                for &p in &f.nodes[section].children {
                                    let name = f.nodes[p]
                                        .words()
                                        .next()
                                        .ok_or_else(|| TypeError::unsupported(f, p))?;
                                    params.push((name.span, self.annotation(self.single_child(p)?, &catalog.types)?));
                                }
                            }
                            Result => result = self.annotation(self.single_child(section)?, &catalog.types)?,
                            _ => return Err(TypeError::unsupported(f, section)),
                        }
                    }
                    let id = self.declaration(n)?;
                    let family = catalog.overloads.entry((&id.package, &id.path)).or_default();
                    for &previous in family
                        .iter()
                        .filter(|previous| previous.enclosing_machines == id.enclosing_machines)
                    {
                        let signature = &catalog.signatures[previous];
                        if signature.result.domains() == result.domains() && types::same_operands(&params, id, &signature.params, previous) {
                            return Err(TypeError::at(f, n, format!("duplicate typed operand signature for {}", id.path)));
                        }
                    }
                    family.push(id);
                    if node.kind != AsmMachine && !sealed && f.child(n, Body).is_none() && !node.parent.is_some_and(|p| f.nodes[p].kind == Trait) {
                        return Err(TypeError::at(f, n, "bodyless machine has no authorized supply"));
                    }
                    catalog.signatures.insert(
                        id,
                        Signature {
                            params,
                            result,
                            site: Site::at(f, n),
                            satisfies: node
                                .children
                                .iter()
                                .filter(|&&c| f.nodes[c].kind == Satisfies)
                                .map(|&c| callback_bounds::slot(self, self.single_child(c)?, &catalog.types))
                                .collect::<std::result::Result<_, _>>()?,
                            requires: f.child(n, Requires).is_some() || f.child(n, Hypothesis).is_some(),
                            terminates: f.child(n, Terminates).is_some(),
                            public: node.words().any(|w| w.text == "public")
                                || node
                                    .parent
                                    .is_some_and(|p| f.nodes[p].kind == Trait && f.nodes[p].words().any(|w| w.text == "public")),
                            crash_ceiling: effects::crashes(self, n)?,
                            pause: effects::Envelope::read(self, n)?,
                            reach: effects::reach(self, &catalog.types, n)?,
                        },
                    );
                }
                _ => return Err(TypeError::unsupported(f, n)),
            }
        }
        Ok(())
    }
}

/// The parsed files and resolution result must come from the same admitted stage.
pub fn type_stage(
    sources: &[StageSource<'_>],
    resolved: &ResolvedStage,
) -> Result<TypedStage, TypeError> {
    type_admitted(sources, resolved, false, &TypingEnvironment::default())
}

pub fn type_stage_for_target(
    sources: &[StageSource<'_>],
    resolved: &ResolvedStage,
    target: &AssemblyTypes,
) -> Result<TypedStage, TypeError> {
    type_admitted(
        sources,
        resolved,
        false,
        &TypingEnvironment {
            target: Some(target),
            ..Default::default()
        },
    )
}

pub fn type_stage_with_evaluator(
    sources: &[StageSource<'_>],
    resolved: &ResolvedStage,
    target: Option<&AssemblyTypes>,
    evaluator: &dyn StaticIntegerEvaluator,
) -> Result<TypedStage, TypeError> {
    type_admitted(
        sources,
        resolved,
        false,
        &TypingEnvironment {
            target,
            integer_evaluator: Some(evaluator),
            ..Default::default()
        },
    )
}
pub fn type_stage_in_environment(
    sources: &[StageSource<'_>],
    resolved: &ResolvedStage,
    environment: &TypingEnvironment<'_>,
) -> Result<TypedStage, TypeError> {
    type_admitted(sources, resolved, false, environment)
}
fn type_admitted(
    sources: &[StageSource<'_>],
    resolved: &ResolvedStage,
    sealed: bool,
    environment: &TypingEnvironment<'_>,
) -> Result<TypedStage, TypeError> {
    let mut units = Vec::new();
    let mut by_path = HashMap::new();
    for src in sources {
        let f = IndexedSource::read(src).map_err(|e| TypeError {
            package: e.package,
            file: e.file,
            span: e.span,
            message: e.message,
        })?;
        if !sealed && f.package == collections::PACKAGE {
            return Err(TypeError::at(&f, 0, "package identity reserved for the toolchain"));
        }
        by_path.insert((f.package, f.path), units.len());
        let binding_annotations = f
            .nodes
            .iter()
            .filter(|node| node.kind == NodeKind::Param)
            .filter_map(|p| Some((p.words().next()?.span, *p.children.first()?)))
            .collect();
        units.push(Unit {
            binding_annotations,
            file: f,
            names: HashMap::new(),
            parameters: HashMap::new(),
            definitions: HashMap::new(),
            hypotheses: HashMap::new(),
        });
    }
    for u in &resolved.uses {
        if let Some(&i) = by_path.get(&(u.package.as_str(), u.file.as_str())) {
            units[i].names.insert(u.span, &u.target);
        }
    }
    for (id, site) in resolved.declarations.iter().zip(&resolved.definition_sites) {
        if let Some(&i) = by_path.get(&(id.package.as_str(), site.file.as_str())) {
            units[i].definitions.insert(site.span, id);
        }
    }
    units.sort_by_key(|u| (u.file.package, u.file.path));
    let mut catalog = Catalog {
        assembly: environment.target,
        ranking_views: environment.ranking_views,
        ..Default::default()
    };
    catalog.types.specializations = deferred_indices::Specializations::build(&units, environment.specialized_indices)?;
    catalog.types.operator_tokens = resolved
        .operator_declarations
        .iter()
        .map(|o| o.token.as_str())
        .collect();
    catalog.types.evaluator = environment.integer_evaluator;
    if units.iter().any(|unit| {
        unit.file
            .nodes
            .iter()
            .any(|n| matches!(n.kind, NodeKind::TypeArgs | NodeKind::ExpressionArgs | NodeKind::ArrayType))
    }) {
        catalog.types.integer_program = static_program::IntegerProgram::build(&units, environment.evaluated_integer_calls)?;
    }
    for unit in &mut units {
        unit.index_types(&mut catalog.types)?;
    }
    for unit in &units {
        unit.index_scopes(&mut catalog.types)?;
    }
    for unit in &units {
        unit.index_const_carriers(&mut catalog.types)?;
    }
    let mut output = TypedStage::default();
    for unit in &units {
        for (n, node) in unit.file.nodes.iter().enumerate() {
            if node.kind == NodeKind::PathType
                && matches!(unit.symbol(n),Some(Symbol::Builtin(s)) if matches!(s.as_str(),"u"|"i"))
                && let Some(section) = unit.file.child(n, NodeKind::TypeArgs)
            {
                let arg = unit.single_child(section)?;
                let width = match unit.symbol(arg) {
                    Some(Symbol::Binding {
                        span,
                        kind: NodeKind::ConstParam | NodeKind::KindedParam,
                    }) => unit.parameters.get(span).cloned(),
                    _ => unit.open_index(&catalog.types, arg, &ValueType::integer("u64").unwrap())?,
                };
                let Some(width) = width else {
                    continue;
                };
                catalog.types.positive_widths.insert(width.clone());
                output.obligations.push(TypingObligation {
                    site: Site::at(&unit.file, n),
                    kind: ObligationKind::PositiveWidth(width.clone()),
                });
            }
        }
    }
    for unit in &units {
        unit.index_aliases(&mut catalog.types)?;
    }
    catalog.types.flatten_plain_aliases();
    for unit in &units {
        unit.index_domains(&mut catalog.types)?;
    }
    for unit in &units {
        unit.index_equations(&mut catalog.types)?;
    }
    hypotheses::index_interfaces(&units, &mut catalog.types)?;
    for unit in &mut units {
        hypotheses::index(unit, &mut catalog, &mut output)?;
    }
    for unit in &units {
        unit.collect(&mut catalog, sealed && unit.file.package == collections::PACKAGE)?;
    }
    hypotheses::assumptions(&units, &mut catalog, &mut output)?;
    callback_bounds::finish(&units, &mut catalog)?;
    for binding in &resolved.operator_declarations {
        let sig = catalog
            .signatures
            .get(&binding.machine)
            .ok_or_else(|| TypeError {
                package: binding.machine.package.clone(),
                file: String::new(),
                span: SourceSpan::new(0, 0),
                message: "operator has no typed signature".into(),
            })?;
        let arity = sig.params.len();
        let valid = match binding.token.as_str() {
            "!" | "~" => arity == 1,
            "-" => matches!(arity, 1 | 2),
            "[..]" => arity >= 2,
            _ => arity == 2,
        };
        let signature_site = || {
            let unit = units
                .iter()
                .find(|u| u.definitions.values().any(|&d| d == &binding.machine))
                .unwrap();
            let span = unit
                .definitions
                .iter()
                .find_map(|(s, &d)| (d == &binding.machine).then_some(*s))
                .unwrap();
            (unit.file.path.to_owned(), span)
        };
        let family = catalog
            .token_bindings
            .entry(binding.token.as_str())
            .or_default();
        let primitive = binding.home.is_none()
            && sig
                .params
                .iter()
                .all(|(_, t)| t.numeric() || t == &ValueType::Bool);
        let duplicate = family.iter().any(|b| {
            b.home == binding.home
                && b.machine.package == binding.machine.package
                && catalog.signatures[&b.machine]
                    .params
                    .iter()
                    .map(|(_, t)| t)
                    .eq(sig.params.iter().map(|(_, t)| t))
        });
        if !valid || duplicate || primitive {
            let (file, span) = signature_site();
            return Err(TypeError {
                package: binding.machine.package.clone(),
                file,
                span,
                message: if !valid {
                    "incorrect operator arity"
                } else if primitive {
                    "cannot bind a compiler-owned primitive operator family"
                } else {
                    "duplicate normalized operator binding"
                }
                .into(),
            });
        }
        family.push(binding);
    }
    constants::infer(&units, &mut catalog, &mut output)?;
    for unit in &units {
        if unit.file.nodes[0].kind == NodeKind::Fragment {
            bodies::check_fragment(unit, &catalog, &mut output)?;
        }
        for (n, node) in unit.file.nodes.iter().enumerate().skip(1) {
            use NodeKind::*;
            if node.kind == Data && catalog.types.multiplicities.get(unit.declaration(n)?) == Some(&Multiplicity::Copy) {
                let id = unit.declaration(n)?;
                for shape in std::iter::once(id).chain(catalog.cases.get(id).into_iter().flatten().copied()) {
                    if let Some(fields) = catalog.fields.get(shape)
                        && let Some(name) = catalog.field_order[shape]
                            .iter()
                            .find(|name| catalog.types.multiplicity(&fields[*name]) != Multiplicity::Copy)
                    {
                        return Err(TypeError::at(&unit.file, n, format!("copy data contains non-copy field {name}")));
                    }
                }
            }
            if node.kind == Data
                && let Some(properties) = catalog.data_properties.get(unit.declaration(n)?)
            {
                output
                    .data_properties
                    .push((unit.declaration(n)?.clone(), properties.clone()));
            }
            if matches!(node.kind, Machine | AsmMachine) {
                let id = unit.declaration(n)?;
                let signature = &catalog.signatures[id];
                conformance::check_satisfaction(unit, &catalog, n, &mut output)?;
                for index in 0..catalog.types.arities[id] {
                    if let Some(ValueType::Callable(bound)) = catalog.types.bound(id, index) {
                        output.callable_bounds.push((id.clone(), index, *bound.clone()));
                    }
                }
                output.signatures.push(TypedSignature {
                    site: Site::at(&unit.file, n),
                    machine: id.clone(),
                    parameters: signature.params.clone(),
                    result: signature.result.clone(),
                });
            }
            if node.kind == MachineType {
                bodies::check_callable(unit, &catalog, n, &mut output)?;
            }
            if node.kind == Conformance {
                conformance::check(unit, &catalog, n, &mut output)?;
            }
            if node.kind == Const {
                let id = unit.declaration(n)?;
                output
                    .constants
                    .push((id.clone(), catalog.constants[id].clone()));
                if node.children.len() == 2 {
                    continue;
                }
            }
            if matches!(node.kind, Machine | AsmMachine | MathLet | Data | Domain | Const) {
                bodies::check_declaration(unit, &catalog, n, &mut output)?;
            }
        }
    }
    let mut open_indices: Vec<_> = catalog
        .types
        .open_indices
        .borrow()
        .iter()
        .map(|(site, (n, carrier))| (site.clone(), *n, carrier.clone()))
        .collect();
    open_indices.sort_by_cached_key(|(site, _, _)| (site.package.clone(), site.file.clone(), site.span.get_start_offset()));
    for (site, n, carrier) in open_indices {
        let unit = units
            .iter()
            .find(|u| u.file.package == site.package && u.file.path == site.file)
            .unwrap();
        bodies::check_static_argument(unit, &catalog, n, &carrier, &mut output)?;
    }
    let mut evaluated: Vec<_> = catalog
        .types
        .evaluations
        .borrow()
        .iter()
        .map(|(request, (n, value))| (request.clone(), *n, value.clone()))
        .collect();
    evaluated.sort_by_cached_key(|(request, _, _)| {
        (
            request.expression.package.clone(),
            request.expression.file.clone(),
            request.expression.span.get_start_offset(),
            request.carrier.to_string(),
        )
    });
    for (request, n, value) in evaluated {
        let unit = units
            .iter()
            .find(|u| u.file.package == request.expression.package && u.file.path == request.expression.file)
            .unwrap();
        bodies::check_static_argument(unit, &catalog, n, &request.carrier, &mut output)?;
        output.evaluated_indices.push((request, value));
    }
    // Recheck every authored intermediate at a closed call, even when a canonical
    // rewrite or a surrounding operation makes the final index representable.
    for call in &output.calls {
        if let Some(formations) = catalog.types.formations.borrow().get(&call.machine) {
            let mut ordered: Vec<_> = formations.iter().collect();
            ordered.sort_by_key(|(site, _)| (&site.package, &site.file, site.span.get_start_offset(), site.span.get_end_offset()));
            for (_, term) in ordered {
                term.substitute(&call.machine, &call.type_arguments)
                    .validate_closed_indices()
                    .map_err(|message| TypeError {
                        package: call.site.package.clone(),
                        file: call.site.file.clone(),
                        span: call.site.span,
                        message,
                    })?;
            }
        }
    }
    effects::validate(&catalog, &mut output)?;
    invocations::compose(&units, &catalog, environment.boundary_slots, &mut output)?;
    effects::guarded_calls(&units, &catalog, &mut output)?;
    cleanup::check(&units, &catalog, &mut output)?;
    refinements::finish(&catalog, &mut output)?;
    cleanup::access_policies(&units, &catalog, environment, &mut output)?;
    Ok(output)
}
