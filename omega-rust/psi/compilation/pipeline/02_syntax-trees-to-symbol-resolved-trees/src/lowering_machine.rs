/*
    Machine definitions.

    `lower_machine_into` sets the lowerer's current-machine context, then lowers
    the ranking view and termination plan, trait conformances, external
    binding identity, and every state. `lower_generic_conformance_bounds` is
    shared with trait definitions. `token_bindings` rejects duplicate direct
    operator-token bindings once every selection has settled.
*/

use crate::lowering::expression::expression_table;
use crate::tables::syntax_expressions;
use crate::tables::syntax_items;
use syntax_trees::identifier::JoinedSyntaxPath;
mod token_bindings {
    /*
        Semantic-home and owner-local duplicate checks for direct machine token
        bindings.

        One declaration binds at most one fixed token, and distinct operand shapes
        may share a token; a second direct binding of the same token for the same
        owner and operand shape rejects at that second declaration
        ([expressions: machine token binding](../../../../../../../wiki/spec/language/expressions.md)).
        The owner is the attached data declaration, or the declaring module for a
        free machine. Shapes compare by resolved symbol identity, so the check runs
        after every selection has settled and a same-leaf type from another module
        never collides.

        A closed direct family is published only by its semantic-home owner
        ([operator families](../../../../../../../wiki/spec/language/expressions.md#operator-families)).
        Use sites select by operand types alone, so every binding that survives
        this module must carry its home in its operand tuple: an attached binding
        (`machine + Vec2::add`) must take `Vec2` somewhere in its telescope, and a
        free binding must name at least one declared type or domain. A telescope
        of bare compiler-owned primitives has no declaration-owned home, so
        `machine + add(left: u8, right: u8)` would inject into `u8`'s closed
        family and rejects here rather than becoming a candidate at every `u8 +`.
        That primitive-family rule binds every introducer -- direct machines,
        `operator` declarations and `boundary` requirement slots -- because each
        becomes a candidate for the token at a use site, and a provider-supplied
        slot replaces the primitive meaning just as a checked body would. Only
        the core semantic-home owner, the toolchain sources, publishes primitive
        operator meanings; compiler policy qualifiers such as `Wrapping` are not
        declared domains and supply no home.

        Ownership is package-scoped, not merely program-local: a closed direct
        family is published only by its semantic-home owner's package
        ([operator families](../../../../../../../wiki/spec/language/expressions.md#operator-families)),
        so a `machine <token>` binding whose home is owned by another package
        rejects at its own declaration rather than entering that family's
        candidate set to collide at a use. The home for an attached binding is
        the attached declaration; for a domain-homed binding the domain; for a
        free binding every declared type or domain the operand tuple names.
        Package identity comes from each declaration's checked source owner, so
        symbols without one (toolchain, source-free, or unmanaged inputs) cannot
        prove a foreign owner and keep the owner-local rule.

        A binding attached to a domain (`machine + Quantity::Additive::add`) is a
        domain-family meaning ([domains: semantic roles and operators](../../../../../../../wiki/spec/language/domains.md#semantic-roles-and-operators)):
        it participates only where an operand binding selects that domain, so its
        semantic home is the domain's carrier (the data or builtin type the domain
        classifies), not the domain symbol itself. The domain is its owner for the
        duplicate check, and declaring such a binding gives the domain its
        denotation role exactly as a domain-homed `operator` declaration did, so
        implicit weakening and result-dispatch keep treating it as semantic.
    */

    use crate::tables::body_expressions;
    use std::collections::{HashMap, HashSet};

    use arena::HandleSpan;
    use diagnostics::Diagnostic;
    use language_semantics::ReferenceAccess;
    use source::SourceSpan;
    use source::Span;
    use symbol_resolved_trees::SymbolResolvedTrees;
    use symbol_resolved_trees::data::TypeParameter;
    use symbol_resolved_trees::machine::Machine;
    use symbol_resolved_trees::name::DiagnosticName;
    use symbol_resolved_trees::signature::StateParameter;
    use symbol_resolved_trees::types::TypeConstraint;
    use symbol_resolved_trees::types::TypeReference;
    use symbols::{JoinedPath, SymbolHandle, SymbolKind};
    use syntax_trees::operator_spelling::OperatorSpelling;

    /*
        Reject every direct machine whose operand tuple omits its semantic home,
        then every spelling-bearing declaration whose fixed token, owner, and
        normalized operand shape repeat an earlier direct machine binding. Each
        rejection reports at its own declaration; a duplicate names the binding it
        repeats.

        `machine <token>` and `operator <token>` are one binding space while both
        introducers parse: the second declaration of an already-bound
        token/owner/shape triple rejects regardless of which introducer spelled
        it. Operator declarations are checked only against machine bindings here;
        an `operator` repeating another `operator` still rejects at the typed
        stage, which owns the same-name and spelling-overlap rules for the family
        it keeps while the introducer is being retired. `boundary` signatures are
        requirement slots, not supply, but they occupy the same token space at a
        use site, so a boundary row repeating a machine binding rejects here too.

        The semantic-home rule stays machine-only: attached `operator` spells like
        `Slice::index<T>(items: &[T], ...)` attach to a namespace that never joins
        the operand tuple, and trait-homed operators participate through
        conformance, not operand selection. Whether the operator form keeps that
        looser admission or retires behind the machine rule is an
        OPERATOR-MACHINE-SUPPLY question, not one this check settles. The one
        home rule every form answers is the primitive-family rule
        ([`TokenBinding::primitive_family_binding`]): no authored declaration of
        any introducer may bind a token over operands that name no declared type
        or domain.
    */
    pub(crate) fn reject_duplicate_direct_token_bindings(
        program: &SymbolResolvedTrees,
    ) -> Result<(), Vec<Diagnostic>> {
        // A retained forest also contains declarations excluded from this
        // activation. Only the selected frontier supplies token bindings; an
        // inactive Build declaration cannot reacquire a home in Product scope.
        let machine_bindings = program
            .machines
            .iter()
            .filter(|machine| program.symbols.symbol_source_is_selected(machine.symbol))
            .filter_map(|machine| {
                machine
                    .spelling
                    .map(|spelling| machine_token_binding(program, machine, spelling))
            })
            .collect::<Vec<_>>();
        let mut operator_bindings = program
            .operators
            .iter()
            .filter(|operator| program.symbols.symbol_source_is_selected(operator.symbol))
            .filter_map(|operator| {
                operator
                    .spelling
                    .map(|spelling| operator_token_binding(program, operator, None, spelling))
            })
            .collect::<Vec<_>>();
        for domain in program
            .domain_definitions
            .iter()
            .filter(|domain| program.symbols.symbol_source_is_selected(domain.symbol))
        {
            operator_bindings.extend(
                program
                    .operator_definitions(domain.operators)
                    .iter()
                    .filter(|operator| program.symbols.symbol_source_is_selected(operator.symbol))
                    .filter_map(|operator| {
                        operator.spelling.map(|spelling| {
                            operator_token_binding(program, operator, Some(domain), spelling)
                        })
                    }),
            );
        }
        let mut bindings: Vec<TokenBinding<'_>> = Vec::new();
        let mut diagnostics = Vec::new();
        for binding in machine_bindings {
            if let Some(diagnostic) = binding.missing_semantic_home(program) {
                diagnostics.push(diagnostic);
                continue;
            }
            if let Some(diagnostic) = binding.foreign_semantic_home(program) {
                diagnostics.push(diagnostic);
                continue;
            }
            if let Some(diagnostic) = binding.primitive_family_binding(program) {
                diagnostics.push(diagnostic);
                continue;
            }
            if let Some(earlier) = bindings
                .iter()
                .find(|earlier| earlier.repeats(program, &binding))
            {
                diagnostics.push(duplicate_diagnostic(&binding, earlier));
                continue;
            }
            bindings.push(binding);
        }
        for binding in &operator_bindings {
            if let Some(diagnostic) = binding.primitive_family_binding(program) {
                diagnostics.push(diagnostic);
                continue;
            }
            if let Some(earlier) = bindings
                .iter()
                .find(|earlier| earlier.repeats(program, binding))
            {
                diagnostics.push(duplicate_diagnostic(binding, earlier));
            }
        }
        if diagnostics.is_empty() {
            Ok(())
        } else {
            Err(diagnostics)
        }
    }

    /* Build the binding identity of a `machine <token>` declaration. */
    fn machine_token_binding<'program>(
        program: &'program SymbolResolvedTrees,
        machine: &'program Machine,
        spelling: OperatorSpelling,
    ) -> TokenBinding<'program> {
        TokenBinding {
            machine: Some(machine),
            symbol: machine.symbol,
            operand_types: entry_operand_types(program, machine),
            name: machine.name.as_str().to_owned(),
            name_span: machine.name.source_span(),
            owner_spelling: machine
                .attached_data
                .as_ref()
                .map_or_else(String::new, |attached| attached.as_str().to_owned()),
            spelling,
            owner: binding_owner(program, machine),
            operand_shape: ShapeNormalizer::new(program, machine.type_parameters)
                .operand_shape(entry_parameters(program, machine)),
            operand_display: ShapeNormalizer::for_display(program, machine.type_parameters)
                .operand_shape(entry_parameters(program, machine)),
        }
    }

    fn duplicate_diagnostic(binding: &TokenBinding, earlier: &TokenBinding) -> Diagnostic {
        Diagnostic::error(format!(
            "`{}` binds the fixed operator token `{}` already bound by `{}` for the \
         same owner and operand shape ({}); a second spelling needs a distinct \
         operand shape or a separate owner",
            binding.name,
            binding.spelling.symbol(),
            earlier.name,
            binding.operand_display
        ))
        .with_source_span(binding.name_span)
    }

    struct TokenBinding<'program> {
        /*
            The declaring `machine` when this binding came from that introducer;
            `operator`-form bindings carry `None` and skip the semantic-home rule
            per the module contract above.
        */
        machine: Option<&'program Machine>,
        /*
            The declaring machine or operator symbol, whose source custody decides
            whether it may publish a primitive operator meaning.
        */
        symbol: SymbolHandle,
        /* The declaration's operand types in telescope order. */
        operand_types: Vec<&'program TypeReference>,
        /*
            The declaration's display path (`Vec2::add`, or the leaf for a
            domain-homed operator).
        */
        name: String,
        name_span: SourceSpan,
        /*
            The authored owner path (`Vec2`, `Quantity::Additive`, empty for a
            free binding). Breaks the tie between two owners whose paths resolved
            to no declaration symbol.
        */
        owner_spelling: String,
        spelling: OperatorSpelling,
        owner: BindingOwner,
        /*
            Duplicate-detection key: resolved symbol identities and binder
            positions, never shown to the author.
        */
        operand_shape: String,
        /* The same operand shape in authored spellings, for diagnostics. */
        operand_display: String,
    }

    impl TokenBinding<'_> {
        fn repeats(&self, program: &SymbolResolvedTrees, other: &Self) -> bool {
            self.spelling == other.spelling
                && self.same_owner(program, other)
                && self.operand_shape == other.operand_shape
        }

        fn same_owner(&self, program: &SymbolResolvedTrees, other: &Self) -> bool {
            match (self.owner, other.owner) {
                (BindingOwner::AttachedData(left), BindingOwner::AttachedData(right))
                    if !(left.is_valid() && right.is_valid()) =>
                {
                    left == right
                        && self.owner_spelling == other.owner_spelling
                        && program
                            .symbols
                            .same_source_package(self.name_span, other.name_span)
                }
                _ => self.owner == other.owner,
            }
        }

        /*
            Whether the declaration comes from a toolchain source; `None` for
            source-free trees, which cannot prove custody either way.
        */
        fn is_toolchain_declaration(&self, program: &SymbolResolvedTrees) -> Option<bool> {
            program
                .symbols
                .symbol_source_origin(self.symbol)
                .map(|origin| origin == source::SourceOrigin::Toolchain)
        }

        /*
            The rejection for an authored binding whose operands name no declared
            type or domain, or `None` otherwise. Such a binding joins a primitive
            operator family (`u32 -`, `i32 ==`), which only the core semantic-home
            owner publishes; its spelled path cannot supply a home, since use sites
            select by operand types alone. A domain-homed binding participates only
            through operands its domain qualifies and keeps its own carrier rule.
            Toolchain declarations are the core owner; source-free trees cannot
            prove custody and are left to the remaining rules.
        */
        fn primitive_family_binding(&self, program: &SymbolResolvedTrees) -> Option<Diagnostic> {
            if matches!(self.owner, BindingOwner::Domain { .. })
                || self.is_toolchain_declaration(program)?
            {
                return None;
            }
            if self
                .operand_types
                .iter()
                .any(|type_reference| names_declaration(program, type_reference))
            {
                return None;
            }
            Some(
                Diagnostic::error(format!(
                    "`{}` binds the fixed operator token `{}` over operands that name no declared \
                 type or domain ({}); that joins a primitive operator family, which only the \
                 core semantic-home owner may publish -- give the binding a data or domain \
                 home among its operands",
                    self.name,
                    self.spelling.symbol(),
                    self.operand_display,
                ))
                .with_source_span(self.name_span),
            )
        }

        /*
            The rejection for a binding whose operand tuple carries no semantic
            home, or `None` when some operand names it. Only `machine <token>`
            declarations answer this rule; `machine` is `None` on an
            `operator`-form binding, which is never checked for a home.
        */
        fn missing_semantic_home(&self, program: &SymbolResolvedTrees) -> Option<Diagnostic> {
            let machine = self.machine?;
            if machine.supply_mode == language_semantics::MachineSupplyMode::TopLevelRequirement {
                return None;
            }
            let operand_types = &self.operand_types;
            let message = match self.owner {
                BindingOwner::AttachedData(home) => {
                    if operand_types
                        .iter()
                        .any(|type_reference| names_symbol(program, type_reference, home))
                    {
                        return None;
                    }
                    format!(
                        "`{}` binds the fixed operator token `{}` but no operand names its semantic \
                     home `{}` ({}); a closed family's direct token bindings belong to the owner \
                     of a participating operand",
                        self.name,
                        self.spelling.symbol(),
                        self.owner_spelling,
                        self.operand_shape
                    )
                }
                BindingOwner::Domain { domain, carrier } => {
                    if operand_types.iter().any(|type_reference| {
                        (carrier.is_valid() && names_symbol(program, type_reference, carrier))
                            || qualified_by_domain(program, type_reference, domain)
                    }) {
                        return None;
                    }
                    format!(
                        "`{}` binds the fixed operator token `{}` but no operand names the carrier of \
                     its home domain `{}` ({}); a domain-family binding participates only through \
                     operands the domain classifies",
                        self.name,
                        self.spelling.symbol(),
                        self.owner_spelling,
                        self.operand_shape
                    )
                }
                BindingOwner::Module(_) => return None,
            };
            Some(Diagnostic::error(message).with_source_span(self.name_span))
        }

        /*
            The rejection for a binding whose semantic home is owned by another
            package, or `None` when every home resolves inside the binding's own
            package. Only `machine <token>` declarations answer this rule (the
            same machine-only scope as [`TokenBinding::missing_semantic_home`]):
            `operator`-form and `boundary` slots occupy the token space but the
            introducer's retirement owns their admission change.

            A home owned by another package rejects the whole binding -- a
            package that owns one operand still injects the binding into the
            foreign operand's closed family. Declarations without a reconciled
            package identity cannot prove a foreign owner and are left to the
            owner-local rules.
        */
        fn foreign_semantic_home(&self, program: &SymbolResolvedTrees) -> Option<Diagnostic> {
            let machine = self.machine?;
            if machine.supply_mode == language_semantics::MachineSupplyMode::TopLevelRequirement {
                return None;
            }
            let binding_package = program.symbols.symbol_package_identity(machine.symbol)?;
            let foreign = |home: SymbolHandle| {
                program
                    .symbols
                    .symbol_package_identity(home)
                    .is_some_and(|owner| owner != binding_package)
            };
            let (home, participant) = match self.owner {
                BindingOwner::AttachedData(home) => {
                    (home.is_valid() && foreign(home)).then_some((home, "attached declaration"))
                }
                BindingOwner::Domain { domain, .. } => {
                    (domain.is_valid() && foreign(domain)).then_some((domain, "home domain"))
                }
                BindingOwner::Module(_) => self
                    .operand_types
                    .iter()
                    .flat_map(|type_reference| declared_operand_symbols(program, type_reference))
                    .find(|symbol| foreign(*symbol))
                    .map(|symbol| (symbol, "operand declaration")),
            }?;
            Some(
                Diagnostic::error(format!(
                    "`{}` binds the fixed operator token `{}` into a closed family through the \
                 {participant} `{}`, which another package owns; only the semantic-home owner's \
                 package may publish a closed direct family's token bindings",
                    self.name,
                    self.spelling.symbol(),
                    program.symbols.display_path(home, "::"),
                ))
                .with_source_span(self.name_span),
            )
        }
    }

    /* The entry state's parameter types in telescope order. */
    fn entry_operand_types<'program>(
        program: &'program SymbolResolvedTrees,
        machine: &Machine,
    ) -> Vec<&'program TypeReference> {
        entry_parameters(program, machine)
            .iter()
            .map(|parameter| &parameter.type_reference)
            .collect()
    }

    /* The entry state's parameters, empty when the machine has no entry state. */
    fn entry_parameters<'program>(
        program: &'program SymbolResolvedTrees,
        machine: &Machine,
    ) -> &'program [StateParameter] {
        let Some(entry) = program.machine_state_handles(machine.states).first() else {
            return &[];
        };
        program.state_parameters(program.machine_state(*entry).parameters)
    }

    /*
        Build the binding identity of a spelling-bearing `operator`-form
        declaration: a classic `operator`, a `boundary operator`/`boundary
        machine` requirement slot (both lower to the same operator record), or —
        when `domain` is given — a member already homed into that domain's
        operator family by `normalize_domain_operator_homes`.

        The owner mirrors `binding_owner` for machines: a domain member takes its
        domain (and carrier) as owner; a root `operator` with a qualified name
        resolves its `Owner::` prefix through the same source-visible top-level
        lookup a machine's `attached_data` uses, and a bare `operator + f` is a
        free binding owned by its declaring module. Root operators whose owner
        path names a domain were moved by `normalize_domain_operator_homes`
        before this check runs, so the data-kind lookup is the whole residual
        rule: trait and builtin owners, like unresolvable ones, stay invalid
        symbols compared by their authored spelling.
    */
    fn operator_token_binding<'program>(
        program: &'program SymbolResolvedTrees,
        operator: &'program symbol_resolved_trees::operator::OperatorDefinition,
        domain: Option<&'program symbol_resolved_trees::domain::DomainDefinition>,
        spelling: OperatorSpelling,
    ) -> TokenBinding<'program> {
        let members = program.operator_path_members(operator.name);
        let leaf = members.last();
        let name = members.joined_path();
        let name_span = members.first().map_or_else(SourceSpan::default, |first| {
            let first = first.source_span();
            let last = leaf.expect("nonempty members").source_span();
            SourceSpan::new(first.source_id, Span::new(first.span.start, last.span.end))
        });
        let (owner_spelling, owner) = match domain {
            Some(domain) => (
                domain.name.as_str().to_owned(),
                BindingOwner::Domain {
                    domain: domain.symbol,
                    carrier: carrier_symbol(program, &domain.target_type),
                },
            ),
            None => {
                let owner_members = &members[..members.len().saturating_sub(1)];
                if owner_members.is_empty() {
                    (
                        String::new(),
                        BindingOwner::Module(program.symbols.symbol_module(operator.symbol)),
                    )
                } else {
                    let owner_spelling = owner_members.joined_path();
                    let owner = program
                        .symbols
                        .find_top_level_by_name_and_kinds_from_source(
                            &owner_spelling,
                            &[SymbolKind::Data],
                            owner_members
                                .first()
                                .map_or(name_span, |member| member.source_span()),
                        )
                        .unwrap_or_else(SymbolHandle::invalid);
                    (owner_spelling, BindingOwner::AttachedData(owner))
                }
            }
        };
        TokenBinding {
            machine: None,
            symbol: operator.symbol,
            operand_types: program
                .state_parameters(operator.parameters)
                .iter()
                .map(|parameter| &parameter.type_reference)
                .collect(),
            name,
            name_span,
            owner_spelling,
            spelling,
            owner,
            operand_shape: ShapeNormalizer::new(program, operator.type_parameters)
                .operand_shape(program.state_parameters(operator.parameters)),
            operand_display: ShapeNormalizer::for_display(program, operator.type_parameters)
                .operand_shape(program.state_parameters(operator.parameters)),
        }
    }

    /*
        The declaration's own generic binder symbols. Each renders as its
        first-occurrence position in the operand telescope (see
        [`ShapeNormalizer`]), so renaming or reordering binders without changing
        their occurrence relationships — `index<T>(&[T], u64) -> T` and
        `index<U>(&[U], u64) -> U`, or `combine<T,U>(T,U)` and
        `combine<A,B>(B,A)` — renders one shape and the declarations collide as
        duplicates. Binders owned by another declaration — the attached data's
        `T` inside `Vec2<T>` — are not in this set and keep their symbol
        identity, so `Vec2<T>` and `Vec2<U>` under one owner stay distinct.
    */
    fn own_binders(
        program: &SymbolResolvedTrees,
        type_parameters: HandleSpan<TypeParameter>,
    ) -> HashSet<SymbolHandle> {
        program
            .data_type_parameters(type_parameters)
            .iter()
            .map(|parameter| parameter.symbol)
            .collect()
    }

    /*
        One name position of an operand's resolved type reference.

        [`name_positions`] owns the inventory; a rule states which positions it
        reads and how deep its reading reaches.
    */
    enum TypePosition<'a> {
        /* A `Named` type or a generic application's base (`Vec2` in `Vec2<T>`). */
        Declaration { symbol: SymbolHandle },
        /*
            A dynamic-trait slot (`dyn Trait<..>`): a requirement the operand
            satisfies, kept distinct from a declaration the operand itself names.
        */
        Trait { symbol: SymbolHandle },
        /*
            A domain constraint's spelling (`i32 in Degrees`). A constraint is
            still a name here; [`domain_named`] is the one place that joins a
            spelling to the declarations it selects.
        */
        Domain { name: &'a DiagnosticName },
    }

    /*
        How deep a rule's reading reaches. The operand-telescope rules ask
        different questions, so their reading depths genuinely differ.
    */
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum ReadingScope {
        /* Every argument, including an instantiated `dyn` requirement's. */
        EveryArgument,
        /*
            Generic application arguments; a `dyn` slot's arguments stay with the
            rule that reads that requirement.
        */
        GenericArguments,
        /*
            The reference, constrained-base, array and slice chain, with no
            application arguments.
        */
        Chain,
        /*
            References and constrained bases only, without elements: a carrier is a
            named declaration, not an array or slice element.
        */
        HeadChain,
    }

    /*
        Every name position of one operand's resolved type reference, in operand
        order.

        Every operand-telescope rule in this module reads the same closed
        inventory: a `Named` slot, a generic application's base and arguments, a
        dynamic-trait slot and arguments, a domain constraint's spelling, and the
        reference, constrained-base, array and slice chains. This is the one owner
        of that inventory; a new position is one edit here rather than five
        recursions kept in step by comment. Two facts are structural, not scope: a
        constraint reports its spelling without resolving it, and a constraint's
        own arguments parameterize the indexed family rather than joining the
        operand tuple, so they are never positions.
    */
    fn name_positions<'program>(
        program: &'program SymbolResolvedTrees,
        reference: &'program TypeReference,
        scope: ReadingScope,
    ) -> Vec<TypePosition<'program>> {
        let mut positions = Vec::new();
        extend_name_positions(program, reference, scope, &mut positions);
        positions
    }

    /* Append every position `scope` reaches, in operand order. */
    fn extend_name_positions<'program>(
        program: &'program SymbolResolvedTrees,
        reference: &'program TypeReference,
        scope: ReadingScope,
        positions: &mut Vec<TypePosition<'program>>,
    ) {
        match reference {
            TypeReference::AliasUse { expansion, .. } => {
                extend_name_positions(program, expansion, scope, positions);
            }
            TypeReference::Named { symbol, .. } => {
                positions.push(TypePosition::Declaration { symbol: *symbol });
            }
            TypeReference::Generic(generic) => {
                positions.push(TypePosition::Declaration {
                    symbol: generic.base_symbol,
                });
                if matches!(
                    scope,
                    ReadingScope::EveryArgument | ReadingScope::GenericArguments
                ) {
                    for argument in program.child_type_references(generic.arguments) {
                        extend_name_positions(program, argument, scope, positions);
                    }
                }
            }
            TypeReference::DynamicTrait {
                symbol, arguments, ..
            } => {
                positions.push(TypePosition::Trait { symbol: *symbol });
                if scope == ReadingScope::EveryArgument {
                    for argument in program.child_type_references(*arguments) {
                        extend_name_positions(program, argument, scope, positions);
                    }
                }
            }
            TypeReference::Reference(inner) => extend_name_positions(
                program,
                program.child_type_reference(inner.referee),
                scope,
                positions,
            ),
            TypeReference::Constrained(constrained) => {
                for constraint in program
                    .tables
                    .types
                    .constraints
                    .span_or_empty(constrained.constraints)
                {
                    if let TypeConstraint::Domain(domain) = constraint {
                        positions.push(TypePosition::Domain { name: &domain.name });
                    }
                }
                extend_name_positions(
                    program,
                    program.child_type_reference(constrained.base_type),
                    scope,
                    positions,
                );
            }
            TypeReference::FixedArray(fixed_array) if scope != ReadingScope::HeadChain => {
                extend_name_positions(
                    program,
                    program.child_type_reference(fixed_array.element_type),
                    scope,
                    positions,
                );
            }
            TypeReference::Slice(slice) if scope != ReadingScope::HeadChain => {
                extend_name_positions(
                    program,
                    program.child_type_reference(slice.element_type),
                    scope,
                    positions,
                )
            }
            TypeReference::FixedArray(_)
            | TypeReference::Slice(_)
            | TypeReference::ConstExpression(_)
            | TypeReference::Unit => {}
        }
    }

    /*
        Whether `symbol` is an authored declaration that can own a closed
        family.
    */
    fn is_declaration(program: &SymbolResolvedTrees, symbol: SymbolHandle) -> bool {
        symbol.is_valid()
            && matches!(
                program.symbols.get(symbol).kind,
                SymbolKind::Data | SymbolKind::Domain | SymbolKind::Trait
            )
    }

    /*
        Whether `type_reference` names the declaration `home` anywhere in its
        structure: directly, behind a reference, as a generic base, as the
        constrained carrier, or as an array or slice element. Every argument
        position is read, including an instantiated `dyn` requirement's.
    */
    fn names_symbol(
        program: &SymbolResolvedTrees,
        type_reference: &TypeReference,
        home: SymbolHandle,
    ) -> bool {
        name_positions(program, type_reference, ReadingScope::EveryArgument)
            .into_iter()
            .any(|position| match position {
                TypePosition::Declaration { symbol } | TypePosition::Trait { symbol } => {
                    symbol == home
                }
                TypePosition::Domain { .. } => false,
            })
    }

    /*
        Whether a constraint selects the exact home domain, behind references,
        constrained bases, arrays and slices. A same-spelled constraint in another
        checked package or scope does not participate in this family's home.
        An application's arguments are not constraint positions.
    */
    fn qualified_by_domain(
        program: &SymbolResolvedTrees,
        type_reference: &TypeReference,
        domain: SymbolHandle,
    ) -> bool {
        name_positions(program, type_reference, ReadingScope::Chain)
            .into_iter()
            .any(|position| match position {
                TypePosition::Domain { name } => domain_named(program, name) == Some(domain),
                TypePosition::Declaration { .. } | TypePosition::Trait { .. } => false,
            })
    }

    /*
        Whether `type_reference` names any authored declaration or a declared
        domain constraint. Builtin primitives resolve to `BuiltinType` symbols and
        generic binders to `TypeParameter` symbols; neither is a declaration that
        can own a family, so a telescope of only those has no home. Generic
        application arguments are read; a `dyn` slot's arguments are left to the
        rule that reads that requirement.
    */
    fn names_declaration(program: &SymbolResolvedTrees, type_reference: &TypeReference) -> bool {
        name_positions(program, type_reference, ReadingScope::GenericArguments)
            .into_iter()
            .any(|position| match position {
                TypePosition::Declaration { symbol } | TypePosition::Trait { symbol } => {
                    is_declaration(program, symbol)
                }
                TypePosition::Domain { name } => domain_named(program, name).is_some(),
            })
    }

    /*
        Every declaration-owned symbol a type reference participates in, over the
        walk [`names_declaration`] reads: named data/domain/trait declarations,
        the declarations behind references, generic bases and arguments, array and
        slice elements, constrained bases, and the declared domains named by
        domain constraints. Domain constraint arguments parameterize the indexed
        family rather than joining it, so they are not homes.
    */
    fn declared_operand_symbols(
        program: &SymbolResolvedTrees,
        type_reference: &TypeReference,
    ) -> Vec<SymbolHandle> {
        let mut symbols = Vec::new();
        for position in name_positions(program, type_reference, ReadingScope::GenericArguments) {
            match position {
                TypePosition::Declaration { symbol } | TypePosition::Trait { symbol } => {
                    if is_declaration(program, symbol) {
                        symbols.push(symbol);
                    }
                }
                TypePosition::Domain { name } => symbols.extend(domain_named(program, name)),
            }
        }
        symbols
    }

    /*
        Resolve a token's domain home with the membership pool. A signature-free
        unique hit does not return before module-local preference. The same
        symbol reached by lookup and by the exposure scan is one home. Two
        different symbols stay unresolved even when they print the same.
    */
    fn domain_named(program: &SymbolResolvedTrees, name: &DiagnosticName) -> Option<SymbolHandle> {
        match crate::symbols::membership_domain_lookup(
            &program.symbols,
            name.as_str(),
            name.source_span(),
        ) {
            crate::symbols::DomainMembership::Unique(symbol) => Some(symbol),
            crate::symbols::DomainMembership::NotFound
            | crate::symbols::DomainMembership::Ambiguous { .. }
            | crate::symbols::DomainMembership::Contested => None,
        }
    }

    /*
        Two unsourced `Quantity` rows are different symbols. A shared display
        path does not choose one of them.
    */
    #[test]
    fn domain_home_lookup_keeps_unsourced_ambiguity() {
        use symbols::{SymbolNameRef, SymbolTableBuilder};

        for count in [1, 2] {
            let mut builder = SymbolTableBuilder::new();
            let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
            let domains = builder.insert_children(
                root,
                (0..count).map(|_| (SymbolKind::Domain, SymbolNameRef::Static("Quantity"))),
            );
            let program = SymbolResolvedTrees {
                symbols: builder.finish(),
                ..Default::default()
            };
            assert_eq!(
                domain_named(&program, &DiagnosticName::generated_static("Quantity")),
                (count == 1).then_some(domains.start()),
            );
        }
    }

    /*
        Ordinary lookup and the exposure scan can both name one domain. That
        repeated handle is one candidate.
    */
    #[test]
    fn one_domain_symbol_reached_through_two_routes_is_one_candidate() {
        use std::path::PathBuf;
        use std::sync::Arc;

        use source::{SourceMap, SourceSpan, Span};
        use symbols::{SymbolKind, SymbolLookup, SymbolNameRef, SymbolTableBuilder};

        let mut sources = SourceMap::default();
        let local = sources
            .add(PathBuf::from("pkg/local.omg"), "Quantity".to_owned())
            .source_id;
        let sources = Arc::new(sources);
        let reference = SourceSpan::new(local, Span::new(0, 8));
        let mut builder = SymbolTableBuilder::with_sources(Some(sources));
        let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let quantity = builder.insert_children(
            root,
            [(
                SymbolKind::Domain,
                SymbolNameRef::OwnedSource {
                    value: "Quantity",
                    source_span: reference,
                },
            )],
        );
        let quantity = SymbolTableBuilder::child_handles(quantity)
            .next()
            .expect("one domain");
        let mut symbols = builder.finish();
        symbols
            .register_source_module(local, [("here", reference)])
            .expect("one module");
        assert_eq!(
            symbols.lookup_top_level_by_name_and_kinds_from_source_matching(
                "Quantity",
                &[SymbolKind::Domain],
                reference,
                |_| true,
            ),
            SymbolLookup::Unique(quantity),
        );
        assert!(symbols.domain_name_reaches(quantity, "Quantity", reference));
        assert!(matches!(
            crate::symbols::membership_domain_lookup(&symbols, "Quantity", reference),
            crate::symbols::DomainMembership::Unique(symbol) if symbol == quantity
        ));
    }

    /*
        A signature-free lookup can see one exact `Quantity` and stop. The
        membership lookup still scans the module and prefers the local leaf
        `Widget::Quantity`. Token homes have to take that local domain.
    */
    #[test]
    fn signature_free_unique_domain_does_not_skip_module_local_preference() {
        use std::path::PathBuf;
        use std::sync::Arc;

        use source::{SourceMap, SourceSpan, Span};
        use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};

        let mut sources = SourceMap::default();
        let local = sources
            .add(PathBuf::from("pkg/local.omg"), "Quantity".to_owned())
            .source_id;
        let sources = Arc::new(sources);
        let reference = SourceSpan::new(local, Span::new(0, 8));
        let mut builder = SymbolTableBuilder::with_sources(Some(sources));
        let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let mut handles = SymbolTableBuilder::child_handles(builder.insert_children(
            root,
            [
                (SymbolKind::Domain, SymbolNameRef::Static("Quantity")),
                (
                    SymbolKind::Domain,
                    SymbolNameRef::OwnedSource {
                        value: "Widget::Quantity",
                        source_span: reference,
                    },
                ),
            ],
        ));
        let signature_free = handles.next().expect("exact Quantity");
        let module_local = handles.next().expect("module-local leaf");
        let mut symbols = builder.finish();
        symbols
            .register_source_module(local, [("here", reference)])
            .expect("one module");
        let program = SymbolResolvedTrees {
            symbols,
            ..Default::default()
        };
        let selected = domain_named(&program, &DiagnosticName::from_str("Quantity", reference));
        assert_eq!(
            selected.map(|symbol| program.symbols.display_path(symbol, "::")),
            Some(program.symbols.display_path(module_local, "::")),
            "signature-free hit was {}",
            program.symbols.display_path(signature_free, "::"),
        );
    }

    /* The declaration set a direct binding is validated against. */
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum BindingOwner {
        /* `machine + Vec2::add(...)`: the exact attached data declaration. */
        AttachedData(SymbolHandle),
        /*
            `machine + Quantity::Additive::add(...)`: the attached domain, whose
            carrier type is the binding's semantic home.
        */
        Domain {
            domain: SymbolHandle,
            carrier: SymbolHandle,
        },
        /* A free machine: its declaring module (invalid for root scope). */
        Module(SymbolHandle),
    }

    fn binding_owner(program: &SymbolResolvedTrees, machine: &Machine) -> BindingOwner {
        if machine.attached_data.is_none() {
            return BindingOwner::Module(program.symbols.symbol_module(machine.symbol));
        }
        if let Some(domain) = attached_domain(program, machine) {
            return BindingOwner::Domain {
                domain: domain.symbol,
                carrier: carrier_symbol(program, &domain.target_type),
            };
        }
        BindingOwner::AttachedData(machine.attached_data_symbol)
    }

    /*
        The domain a machine's attached path names, if any. Attachment symbols are
        assigned against data declarations only, so a domain-attached machine
        carries an invalid attachment symbol. Resolve its authored path at that
        declaration's source occurrence, including checked-owner and scope custody.
    */
    fn attached_domain<'program>(
        program: &'program SymbolResolvedTrees,
        machine: &Machine,
    ) -> Option<&'program symbol_resolved_trees::domain::DomainDefinition> {
        if machine.attached_data_symbol.is_valid() {
            return None;
        }
        let domain = domain_named(program, machine.attached_data.as_ref()?)?;
        program.domain_definition_by_symbol(domain)
    }

    /*
        The declaration a domain classifies: the named base of its target type,
        behind references and constrained bases. An array or slice target has no
        carrier declaration -- the domain's own spelling qualifies an operand
        instead ([`qualified_by_domain`]).
    */
    fn carrier_symbol(
        program: &SymbolResolvedTrees,
        type_reference: &TypeReference,
    ) -> SymbolHandle {
        name_positions(program, type_reference, ReadingScope::HeadChain)
            .into_iter()
            .find_map(|position| match position {
                TypePosition::Declaration { symbol } => Some(symbol),
                TypePosition::Trait { .. } | TypePosition::Domain { .. } => None,
            })
            .unwrap_or(SymbolHandle::invalid())
    }

    /*
        Give every domain that owns a token-bearing machine its denotation role,
        as `normalize_domain_operator_homes` does for domain-homed `operator`
        declarations. Runs once attached symbols are assigned and before the
        selections that classify predicate-only domains read the roles.
    */
    pub(crate) fn mark_token_bound_domain_homes(program: &mut SymbolResolvedTrees) {
        let homes = program
            .machines
            .iter()
            .filter(|machine| {
                machine.spelling.is_some()
                    && program.symbols.symbol_source_is_selected(machine.symbol)
            })
            .filter_map(|machine| attached_domain(program, machine).map(|domain| domain.symbol))
            .collect::<Vec<_>>();
        if homes.is_empty() {
            return;
        }
        program.domain_definitions.for_each_mut(|domain| {
            if homes.contains(&domain.symbol) {
                domain.semantic_roles.denotation_dimension = Some(domain.semantic_id);
            }
        });
    }

    /*
        Renders the complete operand telescope by resolved identity: symbols
        where resolution assigned them, authored spelling only for the remaining
        unsymbolled leaves, and the declaration's own generic binders as
        first-occurrence positions across the telescope. Reference lifetimes are
        borrow-region tags and never distinguish operand shapes.
    */
    struct ShapeNormalizer<'program> {
        program: &'program SymbolResolvedTrees,
        /*
            The declaration's own binder symbols; every other symbol keeps its
            resolved identity.
        */
        own_binders: HashSet<SymbolHandle>,
        /*
            Own binder symbol → its first-occurrence position in the telescope,
            assigned during the single rendering pass.
        */
        ordinals: HashMap<SymbolHandle, usize>,
        /*
            Render authored spellings instead of identities: a diagnostic names
            `Wrapped`, not the arena slot that tells two same-spelled types apart.
        */
        display: bool,
    }

    impl<'program> ShapeNormalizer<'program> {
        fn new(
            program: &'program SymbolResolvedTrees,
            type_parameters: HandleSpan<TypeParameter>,
        ) -> Self {
            Self {
                program,
                own_binders: own_binders(program, type_parameters),
                ordinals: HashMap::new(),
                display: false,
            }
        }

        fn for_display(
            program: &'program SymbolResolvedTrees,
            type_parameters: HandleSpan<TypeParameter>,
        ) -> Self {
            Self {
                display: true,
                ..Self::new(program, type_parameters)
            }
        }

        fn operand_shape(&mut self, parameters: &[StateParameter]) -> String {
            parameters
                .iter()
                .map(|parameter| self.parameter_shape(parameter))
                .collect::<Vec<_>>()
                .join(", ")
        }

        fn parameter_shape(&mut self, parameter: &StateParameter) -> String {
            let mut shape = String::new();
            if parameter.is_mutable {
                shape.push_str("mut ");
            }
            if parameter.is_const {
                shape.push_str("const ");
            }
            shape.push_str(&self.type_shape(&parameter.type_reference));
            shape
        }

        /*
            Render `symbol` as the position where it first occurs in the
            telescope when the declaring signature owns it, else by resolved
            identity or authored spelling.
        */
        fn binder_or_symbol(&mut self, symbol: SymbolHandle, spelling: &str) -> String {
            if self.display {
                return spelling.to_owned();
            }
            if self.own_binders.contains(&symbol) {
                let ordinal = self.ordinals.len();
                return format!("binder{}", *self.ordinals.entry(symbol).or_insert(ordinal));
            }
            symbol_or_spelling(symbol, spelling)
        }

        fn type_shape(&mut self, type_reference: &TypeReference) -> String {
            match type_reference {
                TypeReference::AliasUse { expansion, .. } => self.type_shape(expansion),
                TypeReference::Reference(reference) => {
                    let access = match reference.access {
                        ReferenceAccess::Shared => "&",
                        ReferenceAccess::Mutable => "&mut ",
                        ReferenceAccess::WriteOnly => "&write ",
                    };
                    let referee = self.program.child_type_reference(reference.referee);
                    format!("{access}{}", self.type_shape(referee))
                }
                TypeReference::Constrained(constrained) => {
                    let constraints = self
                        .program
                        .tables
                        .types
                        .constraints
                        .span_or_empty(constrained.constraints)
                        .iter()
                        .map(|constraint| match constraint {
                            TypeConstraint::Domain(domain) if !domain.arguments.is_empty() => {
                                let arguments = self
                                    .program
                                    .child_type_references(domain.arguments)
                                    .iter()
                                    .map(|argument| self.type_shape(argument))
                                    .collect::<Vec<_>>()
                                    .join(", ");
                                format!("{}<{arguments}>", domain.name.as_str())
                            }
                            _ => constraint.display_name(),
                        })
                        .collect::<Vec<_>>()
                        .join(", ");
                    let base_type = self.program.child_type_reference(constrained.base_type);
                    format!("{}[{constraints}]", self.type_shape(base_type))
                }
                TypeReference::FixedArray(fixed_array) => {
                    let element_type = self.program.child_type_reference(fixed_array.element_type);
                    format!(
                        "[{}; {}]",
                        self.type_shape(element_type),
                        fixed_array.length
                    )
                }
                TypeReference::Slice(slice) => {
                    let element_type = self.program.child_type_reference(slice.element_type);
                    format!("[{}]", self.type_shape(element_type))
                }
                TypeReference::Generic(generic) => {
                    let arguments = self
                        .program
                        .child_type_references(generic.arguments)
                        .iter()
                        .map(|argument| self.type_shape(argument))
                        .collect::<Vec<_>>()
                        .join(", ");
                    format!(
                        "{}<{arguments}>",
                        self.binder_or_symbol(generic.base_symbol, generic.base_name.as_str())
                    )
                }
                TypeReference::ConstExpression(expression) => format!(
                    "const({})",
                    body_expressions(self.program).display_name(expression.expression)
                ),
                TypeReference::DynamicTrait {
                    symbol,
                    name,
                    arguments,
                    ..
                } => format!(
                    "dyn {}<{}>",
                    self.binder_or_symbol(*symbol, name.as_str()),
                    self.program
                        .child_type_references(*arguments)
                        .iter()
                        .map(|argument| self.type_shape(argument))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                TypeReference::Named { symbol, name, .. } => {
                    self.binder_or_symbol(*symbol, name.as_str())
                }
                TypeReference::Unit => "()".to_owned(),
            }
        }
    }

    fn symbol_or_spelling(symbol: SymbolHandle, spelling: &str) -> String {
        if symbol.is_valid() {
            format!("{spelling}#{}", symbol.arena_index())
        } else {
            spelling.to_owned()
        }
    }
}
mod type_equations {
    /*
        Preserve the source classifier's type-role equations for generic body
        checking. Application preparation still discharges every requires equation;
        current callers never create assumptions for a declaration's body.
    */

    use crate::lowering::type_reference;
    use crate::resolution::lowerer::Lowerer;
    use arena::HandleSpan;
    use diagnostics::Diagnostic;
    use symbol_resolved_trees::machine::DeclaredTypeEquation;
    use symbol_resolved_trees::types::{
        ReferenceTypeReference, ReferenceTypeReferenceStorage, SliceTypeReference,
        SliceTypeReferenceStorage, TypeReference,
    };
    use syntax_trees::expression::{ExpressionHandle, ExpressionNode};
    use syntax_trees::identifier::JoinedSyntaxPath;
    use syntax_trees::{SyntaxTrees, item::Machine};

    pub(super) struct MachineTypeEquations {
        pub(super) definitions: HandleSpan<DeclaredTypeEquation>,
        pub(super) requires_argument_recovery: bool,
        pub(super) declares_equations: bool,
    }

    pub(super) fn lower(
        lowerer: &mut Lowerer,
        syntax: &SyntaxTrees,
        machine: &Machine,
    ) -> Result<MachineTypeEquations, Diagnostic> {
        let parameters = syntax.items.type_parameters(machine.type_parameters);
        let equations =
            crate::preparation::machine_equations::machine_type_equations(syntax, machine);
        let declares_equations = !equations.is_empty();
        let requires_argument_recovery = equations
            .iter()
            .any(crate::preparation::type_equations::TypeEquation::requires_argument_recovery);
        let mut lowered = Vec::with_capacity(equations.len());
        for equation in equations {
            equation.validate_kind("machine")?;
            let Some((parameter, definition)) = equation.structural_definition() else {
                continue;
            };
            lowered.push(DeclaredTypeEquation {
                parameter: TypeReference::Named {
                    supplied_operand_owner: Default::default(),
                    symbol: symbols::SymbolHandle::invalid(),
                    name: crate::lowering::name::lower_name(syntax, &parameters[parameter].name),
                },
                definition: lower_type_operand(lowerer, syntax, definition)?,
            });
        }
        Ok(MachineTypeEquations {
            definitions: lowerer
                .symbol_resolved_trees
                .tables
                .declarations
                .declared_type_equations
                .insert_many(lowered),
            requires_argument_recovery,
            declares_equations,
        })
    }

    /*
        These expression forms have type role only after the equation classifier
        selected an opposite type binder. Their children join ordinary type storage.
    */
    fn lower_type_operand(
        lowerer: &mut Lowerer,
        syntax: &SyntaxTrees,
        expression: ExpressionHandle,
    ) -> Result<TypeReference, Diagnostic> {
        Ok(match syntax.expressions.expression(expression) {
            ExpressionNode::TypeExpression(reference) => {
                type_reference::lower_type_reference_handle(lowerer, syntax, *reference)?
            }
            ExpressionNode::Name {
                members: path,
                application: path_application,
            } if path_application.is_none() => {
                let members = syntax.expressions.identifier_path_members(path.to_owned());
                TypeReference::Named {
                    supplied_operand_owner: Default::default(),
                    symbol: symbols::SymbolHandle::invalid(),
                    name: symbol_resolved_trees::name::DiagnosticName::new(
                        members.joined_path_in(syntax),
                        syntax.expressions.source_span(expression),
                    ),
                }
            }
            ExpressionNode::Borrow(borrow) => {
                let referee = lower_type_operand(lowerer, syntax, borrow.target)?;
                let referee = lowerer.declaration_child_type_references().append(referee);
                TypeReference::Reference(ReferenceTypeReference {
                    storage: ReferenceTypeReferenceStorage {
                        supplied_operand_owner: Default::default(),
                        referee,
                        access: borrow.access,
                        lifetime: None,
                    },
                })
            }
            ExpressionNode::ArrayLiteral(elements) => {
                let [element] = syntax.expressions.expression_handles(*elements) else {
                    return Err(Diagnostic::error(
                        "type equation requires exactly one slice element type",
                    )
                    .with_source_span(syntax.expressions.source_span(expression)));
                };
                let element = lower_type_operand(lowerer, syntax, *element)?;
                let element_type = lowerer.declaration_child_type_references().append(element);
                TypeReference::Slice(SliceTypeReference {
                    storage: SliceTypeReferenceStorage {
                        supplied_operand_owner: Default::default(),
                        element_type,
                    },
                })
            }

            _ => {
                return Err(Diagnostic::error(
                    "structural type equation lost its classified type operand",
                )
                .with_source_span(syntax.expressions.source_span(expression)));
            }
        })
    }
}

pub(crate) use token_bindings::{
    mark_token_bound_domain_homes, reject_duplicate_direct_token_bindings,
};

use crate::lowering::data::lower_type_parameters;
use crate::lowering::expression::lower_expression_into_table;
use crate::lowering::name::lower_name;
use crate::lowering::name::lower_names;
use crate::lowering::state::{
    lower_machine_signature_contracts, lower_service_reach_names, lower_signature_invokes,
    lower_state_body, lower_state_node,
};
use crate::lowering::type_reference::lower_child_type_references;
use crate::resolution::lowerer::Lowerer;
use arena::{Handle, HandleSpan};
use diagnostics::Diagnostic;
use language_semantics::RankingViewId;
use language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure;
use language_semantics::declaration_selection::CompilerDerivedSelectionPartition;
use std::mem::replace;
use symbol_resolved_trees::expression::ExpressionHandle;
use symbol_resolved_trees::machine::{
    GenericConformanceBound, GenericDataMachineOrigin, Machine, MachineBodyStatus, MachineStorage,
    TraitConformance, TraitDefaultOrigin,
};
use symbol_resolved_trees::name::DiagnosticName;
use symbol_resolved_trees::state::State;
use symbols::SymbolHandle;
use syntax::expression::BinaryOperator;
use syntax::expression::ExpressionNode;
use syntax::identifier::Identifier;
use syntax::item::ExternalBinding;
use syntax::item::Item;
use syntax::item::SatisfiesClause;
use syntax::item::StateHandle;
use syntax_trees::item::CapabilityContractKind;
use syntax_trees::{self as syntax, SyntaxTrees};

/*
    Prepared syntax bound to the one declaration roster that indexed it.
    Compiler-derived machines can rewrite a template's name/attachment; retain
    that normalized owner rather than rediscovering it by an authored spelling.
*/
#[derive(Debug, Clone, compiler_storage::Record)]
pub(crate) struct PendingMachineBody {
    pub(crate) machine_ordinal: usize,
    source: syntax::item::Machine,
    states: Vec<(Handle<State>, StateHandle)>,
}

impl PendingMachineBody {
    pub(crate) fn same_syntax(
        &self,
        other: &Self,
        left: &syntax::SyntaxTextOwner,
        right: &syntax::SyntaxTextOwner,
    ) -> bool {
        use syntax::SyntaxEquivalent;
        let Self {
            machine_ordinal,
            source,
            states,
        } = self;
        *machine_ordinal == other.machine_ordinal
            && *states == other.states
            && source.same_syntax(&other.source, left, right)
    }
}

/*
    Structural exposure is decided before supply classification erases whether
    an external leaf used the boundary introducer. Keep it separate from `public`:
    exported contracts do not make a private declaration source-nameable.
*/
pub(crate) fn machine_interface_exposure(
    machine: &syntax::item::Machine,
) -> AuthoredDeclarationSelectionExposure {
    use language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure;
    if machine.is_public
        || (machine.boundary
            && !crate::lowering::operator::is_token_bearing_boundary_signature(machine))
    {
        AuthoredDeclarationSelectionExposure::PublicInterface
    } else {
        AuthoredDeclarationSelectionExposure::PrivateImplementation
    }
}

pub(crate) fn lower_machine_into(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    machine: &syntax::item::Machine,
) -> Result<(), Diagnostic> {
    lower_lexical_machine_into(lowerer, syntax_trees, machine, Handle::invalid())
}

fn lower_lexical_machine_into(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    machine: &syntax::item::Machine,
    lexical_parent: Handle<Machine>,
) -> Result<(), Diagnostic> {
    lowerer.with_authored_expression_exposure(machine_interface_exposure(machine), |lowerer| {
        lower_lexical_machine_parts(lowerer, syntax_trees, machine, lexical_parent)
    })
}

fn lower_lexical_machine_parts(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    machine: &syntax::item::Machine,
    lexical_parent: Handle<Machine>,
) -> Result<(), Diagnostic> {
    if lexical_parent.is_valid()
        && (machine.is_public
            || machine.attached_data.is_some()
            || machine.boundary
            || machine.spelling.is_some())
    {
        return Err(Diagnostic::error(
            "a nested machine is a private independent lexical declaration",
        ));
    }
    let compiler_provided = machine.target.is_none()
        && !machine.target_family_interface
        && crate::lowering::operator::is_bare_bodyless_signature(machine);
    if compiler_provided {
        crate::lowering::operator::require_catalog_signature(lowerer, syntax_trees, machine)?;
    }
    let compiler_selection_partition = compiler_selection_partition(
        lowerer.symbol_resolved_trees.machines.len(),
        syntax_trees,
        machine,
    );
    lowerer.current_compiler_selection_partition = compiler_selection_partition;
    lowerer.current_machine_is_boundary = machine.boundary;
    lowerer.current_machine_root_index = Some(lowerer.symbol_resolved_trees.machines.len());
    lowerer.current_machine_name = Some(machine.name.as_str(syntax_trees).to_owned());
    lowerer.current_evidence_term_names = syntax_items(syntax_trees)
        .capability_contracts(machine.contracts)
        .iter()
        .filter_map(|contract| contract.binding.as_ref())
        .map(|binding| binding.as_str(syntax_trees).to_owned())
        .collect();
    let interface_exposure = machine_interface_exposure(machine);
    let states = lowerer.with_authored_expression_exposure(
        AuthoredDeclarationSelectionExposure::PrivateImplementation,
        |lowerer| {
            lower_machine_states(
                lowerer,
                syntax_trees,
                machine.states,
                interface_exposure == AuthoredDeclarationSelectionExposure::PublicInterface,
            )
        },
    )?;
    let body_status = if lowerer.defer_machine_bodies && !machine.bodyless {
        lowerer.pending_machine_bodies.push(PendingMachineBody {
            machine_ordinal: lowerer.symbol_resolved_trees.machines.len(),
            source: machine.clone(),
            states: lowerer
                .symbol_resolved_trees
                .machine_state_handles(states)
                .iter()
                .copied()
                .zip(
                    syntax_items(syntax_trees)
                        .state_handles(machine.states)
                        .iter()
                        .copied(),
                )
                .collect(),
        });
        MachineBodyStatus::Deferred
    } else {
        MachineBodyStatus::Complete
    };
    lowerer.current_machine_is_boundary = false;
    lowerer.current_machine_root_index = None;
    lowerer.current_machine_name = None;
    lowerer.current_state_name = None;
    lowerer.current_evidence_term_names.clear();
    let type_parameters = lower_type_parameters(lowerer, syntax_trees, machine.type_parameters)?;
    let type_equations::MachineTypeEquations {
        definitions: declared_type_equations,
        requires_argument_recovery,
        declares_equations,
    } = type_equations::lower(lowerer, syntax_trees, machine)?;

    let sibling_target = machine
        .target
        .as_ref()
        .map(|name| lower_name(syntax_trees, name));
    let satisfies = lower_machine_trait_conformances(lowerer, syntax_trees, machine.satisfies)?;
    let inferred_float_binding = if is_sealed_float_provider_member(lowerer, syntax_trees, machine)
    {
        let binding = lowerer
            .symbol_resolved_trees
            .external_bindings
            .intern(language_semantics::ExternalBindingIdentity::CompilerIntrinsic);
        for edge in lowerer
            .symbol_resolved_trees
            .tables
            .declarations
            .machine_trait_conformances
            .span_mut_or_empty(satisfies)
        {
            edge.external_binding = Some(binding);
        }
        Some(binding)
    } else {
        None
    };
    let conformance_bounds =
        lower_generic_conformance_bounds(lowerer, syntax_trees, &machine.conformance_bounds)?;
    let ranking_subjects =
        lower_ranking_expressions(lowerer, syntax_trees, machine.ranking_subjects)?;
    let ranking_view = lower_machine_ranking_view(lowerer, syntax_trees, machine.ranking_view);
    let ranking_view_arguments =
        lower_ranking_expressions(lowerer, syntax_trees, machine.ranking_view_arguments)?;
    let ranking_range = if machine.ranking_range.is_valid() {
        lower_expression_into_table(lowerer, syntax_trees, machine.ranking_range)?
    } else {
        ExpressionHandle::invalid()
    };
    let service_reaches = lower_service_reach_names(syntax_trees, machine.service_reaches);
    let invokes = lower_signature_invokes(lowerer, syntax_trees, machine.invokes);
    let contracts =
        lower_machine_signature_contracts(lowerer, syntax_trees, machine.contracts, states)?;
    let machine_name = lower_name(syntax_trees, &machine.name);
    let target_symbol_name = match &sibling_target {
        Some(target) => DiagnosticName::new(
            format!("{}::{}", machine.name.as_str(syntax_trees), target.as_str()),
            machine.name.source_span(),
        ),
        None => DiagnosticName::default(),
    };
    let attached_data = machine
        .attached_data
        .as_ref()
        .map(|name| lower_name(syntax_trees, name));
    let termination_plan = build_termination_plan(syntax_trees, machine);

    let supply_mode = {
        let via_binding = syntax_items(syntax_trees)
            .satisfies_clauses(machine.satisfies)
            .iter()
            .find_map(|clause| clause.via.as_ref())
            .map(|value| external_binding_identity(syntax_trees, value));
        let has_via_expression = syntax_items(syntax_trees)
            .satisfies_clauses(machine.satisfies)
            .iter()
            .any(|clause| clause.via_expression.is_valid());
        if compiler_provided {
            language_semantics::MachineSupplyMode::CompilerProvided
        } else if machine.target_family_interface {
            language_semantics::MachineSupplyMode::Requirement
        } else if crate::lowering::operator::is_token_bearing_boundary_signature(machine) {
            language_semantics::MachineSupplyMode::TopLevelRequirement
        } else if let Some(binding) = inferred_float_binding {
            language_semantics::MachineSupplyMode::ExternalRealization {
                binding: Some(binding),
                mechanism: Some(language_semantics::ExternalBindingMechanism::CompilerIntrinsic),
            }
        } else if machine.bodyless && (via_binding.is_some() || has_via_expression) {
            let (binding, mechanism) = via_binding
                .map(|identity| {
                    let mechanism = identity.mechanism();
                    let binding = lowerer
                        .symbol_resolved_trees
                        .external_bindings
                        .intern(identity);
                    (Some(binding), Some(mechanism))
                })
                .unwrap_or((None, None));
            language_semantics::MachineSupplyMode::ExternalRealization { binding, mechanism }
        } else if machine.bodyless && machine.boundary {
            let authors_fact = syntax_items(syntax_trees)
                .capability_contracts(machine.contracts)
                .iter()
                .any(|contract| {
                    matches!(contract.kind, CapabilityContractKind::Ensures)
                        && !contract.facts.is_empty()
                });
            if compiler_owned_boundary(lowerer, syntax_trees, machine) {
                language_semantics::MachineSupplyMode::Boundary
            } else if authors_fact {
                language_semantics::MachineSupplyMode::AdmissionClaim
            } else {
                language_semantics::MachineSupplyMode::Boundary
            }
        } else if machine.boundary {
            language_semantics::MachineSupplyMode::Boundary
        } else {
            language_semantics::MachineSupplyMode::CheckedBody
        }
    };
    lowerer.pending_machine_service_reaches.push(
        crate::selection::service_reaches::PendingAuthoredServiceReach::Authored {
            keyword_source_spans: machine.service_reach_keyword_source_spans.clone(),
            authored: service_reaches,
        },
    );
    if let Some(sources) = &mut lowerer.equation_sources
        && !machine.type_parameters.is_empty()
    {
        sources.declarations.push((
            lowerer.symbol_resolved_trees.machines.len(),
            machine.clone(),
        ));
    }
    let declaration = lowerer.symbol_resolved_trees.machines.push(Machine {
        symbol: SymbolHandle::invalid(),
        lexical_parent,
        name: machine_name,
        generic_data_origin: GenericDataMachineOrigin {
            template_source: lower_name(syntax_trees, &machine.generic_data_template),
            ..Default::default()
        },
        trait_default_origin: TraitDefaultOrigin {
            trait_source: lower_name(syntax_trees, &machine.default_body_trait),
            ..Default::default()
        },
        attached_data,
        attached_data_symbol: SymbolHandle::invalid(),
        attached_data_application: None,
        spelling: machine.spelling,
        is_public: machine.is_public,
        interface_exposure,
        supply_mode,
        body_is_present: !machine.bodyless,
        target: sibling_target,
        authored_target: machine
            .authored_target
            .as_ref()
            .map(|name| lower_name(syntax_trees, name)),
        target_family: SymbolHandle::invalid(),
        target_family_interface: machine.target_family_interface,
        target_family_contracts_prepared: false,
        target_symbol_name,
        has_structural_type_equations: requires_argument_recovery,
        structural_type_equations_pending: declares_equations
            && (lowerer.structural_type_equations_pending || !requires_argument_recovery),
        termination_plan,
        service_reach_row: language_semantics::ServiceReachRowId::NULL,
        service_reach_is_installation_bound: machine.service_reach_is_installation_bound,
        admitted_same_stack_bytes: machine.admitted_same_stack_bytes,
        declares_no_crash_route: machine.declares_no_crash_route,
        suspends_keyword_source_spans: machine.suspends_keyword_source_spans.clone(),
        blocks_keyword_source_spans: machine.blocks_keyword_source_spans.clone(),
        compiler_selection_partition,
        body_status,
        storage: MachineStorage {
            lifetime_parameters: lower_names(syntax_trees, &machine.lifetime_parameters),
            type_parameters,
            declared_type_equations,
            owned_data: HandleSpan::empty(),
            satisfies,
            conformance_bounds,
            ranking_subjects,
            ranking_view,
            ranking_view_arguments,
            ranking_range,
            invokes,
            suspends: machine.suspends,
            blocks: machine.blocks,
            contracts,
            states,
        },
    });
    lowerer.current_compiler_selection_partition = None;
    for child in machine.nested_machines.handles() {
        let Item::Machine(child) = syntax_trees.items.item(child) else {
            return Err(Diagnostic::error(
                "a machine lexical scope contains a non-machine declaration",
            ));
        };
        lower_lexical_machine_into(lowerer, syntax_trees, child, declaration)?;
    }
    Ok(())
}

/*
    Lower a selected implementation without recreating its declaration or its
    state parameters. Symbols for new locals are appended by the symbol owner.
*/
pub(crate) fn complete_machine_body(
    lowerer: &mut Lowerer,
    syntax: &SyntaxTrees,
    pending: &PendingMachineBody,
) -> Result<(), Diagnostic> {
    let inputs = crate::resolution::type_inputs::TypeInputScope::new(lowerer);
    with_pending_machine_scope(lowerer, syntax, pending, |lowerer| {
        for &(state_handle, syntax_state) in &pending.states {
            let state = lowerer
                .symbol_resolved_trees
                .machine_state(state_handle)
                .clone();
            let statements = lower_state_body(
                lowerer,
                syntax,
                &state,
                syntax.items.state(syntax_state).statements,
            )?;
            lowerer
                .symbol_resolved_trees
                .tables
                .declarations
                .machine_states
                .get_mut(state_handle)
                .statements = statements;
        }
        lowerer.symbol_resolved_trees.machines[pending.machine_ordinal].body_status =
            MachineBodyStatus::Complete;
        Ok(())
    })?;
    let owner = lowerer.symbol_resolved_trees.machines[pending.machine_ordinal].symbol;
    inputs.finish(lowerer, owner);
    lowerer.completed_machines.push(owner);
    Ok(())
}

pub(crate) fn lower_pending_lexical_inputs(
    lowerer: &mut Lowerer,
    syntax: &SyntaxTrees,
    demand: crate::lowering::state::LexicalInputDemand,
) -> Result<Vec<crate::lowering::state::StateLexicalInputs>, Diagnostic> {
    let pending = lowerer.pending_machine_bodies.clone();
    let mut inputs = Vec::new();
    for body in &pending {
        with_pending_machine_scope(lowerer, syntax, body, |lowerer| {
            let machine = lowerer.symbol_resolved_trees.machines[body.machine_ordinal].symbol;
            for &(state, syntax_state) in &body.states {
                inputs.push(crate::lowering::state::lower_state_lexical_inputs(
                    lowerer,
                    syntax,
                    machine,
                    state,
                    syntax.items.state(syntax_state).statements,
                    demand,
                )?);
            }
            Ok(())
        })?;
    }
    Ok(inputs)
}

fn with_pending_machine_scope<T>(
    lowerer: &mut Lowerer,
    syntax: &SyntaxTrees,
    pending: &PendingMachineBody,
    operation: impl FnOnce(&mut Lowerer) -> Result<T, Diagnostic>,
) -> Result<T, Diagnostic> {
    let declaration = lowerer
        .symbol_resolved_trees
        .machines
        .iter()
        .nth(pending.machine_ordinal)
        .ok_or_else(|| Diagnostic::error("deferred body lost its indexed machine"))?;
    if declaration.body_status != MachineBodyStatus::Deferred {
        return Err(Diagnostic::error(
            "machine body completion requires a pending implementation",
        ));
    }
    let partition = declaration.compiler_selection_partition;
    let previous_defer = replace(&mut lowerer.defer_machine_bodies, false);
    let previous_partition = replace(&mut lowerer.current_compiler_selection_partition, partition);
    let previous_boundary = replace(
        &mut lowerer.current_machine_is_boundary,
        pending.source.boundary,
    );
    let previous_root = lowerer
        .current_machine_root_index
        .replace(pending.machine_ordinal);
    let previous_name = lowerer
        .current_machine_name
        .replace(pending.source.name.as_str(syntax).to_owned());
    let evidence_names = syntax_items(syntax)
        .capability_contracts(pending.source.contracts)
        .iter()
        .filter_map(|contract| contract.binding.as_ref())
        .map(|binding| binding.as_str(syntax).to_owned())
        .collect();
    let previous_evidence = replace(&mut lowerer.current_evidence_term_names, evidence_names);
    let result = lowerer.with_authored_expression_exposure(
        AuthoredDeclarationSelectionExposure::PrivateImplementation,
        operation,
    );
    lowerer.defer_machine_bodies = previous_defer;
    lowerer.current_compiler_selection_partition = previous_partition;
    lowerer.current_machine_is_boundary = previous_boundary;
    lowerer.current_machine_root_index = previous_root;
    lowerer.current_machine_name = previous_name;
    lowerer.current_evidence_term_names = previous_evidence;
    result
}

fn compiler_selection_partition(
    machine_ordinal: usize,
    syntax_trees: &SyntaxTrees,
    machine: &syntax::item::Machine,
) -> Option<CompilerDerivedSelectionPartition> {
    if machine.name.is_source_backed() {
        return None;
    }
    let ordinal = u64::try_from(machine_ordinal).ok()?;
    let [requirement] = syntax_trees.items.satisfies_clauses(machine.satisfies) else {
        return machine
            .attached_data
            .is_some()
            .then(|| CompilerDerivedSelectionPartition::from_compiler_ordinal(ordinal));
    };
    let requirement_name = requirement.requirement.as_ref()?;
    if requirement.trait_name.is_source_backed()
        || requirement_name.is_source_backed()
        || requirement.alias.is_some()
        || requirement.via.is_some()
        || requirement.via_expression.is_valid()
    {
        return None;
    }
    Some(CompilerDerivedSelectionPartition::from_compiler_ordinal(
        ordinal,
    ))
}

pub(crate) fn lower_generic_conformance_bounds(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    bounds: &[syntax::item::GenericConformanceBound],
) -> Result<Vec<GenericConformanceBound>, Diagnostic> {
    bounds
        .iter()
        .map(|bound| {
            Ok(GenericConformanceBound {
                binder: SymbolHandle::invalid(),
                binder_name: lower_name(syntax_trees, &bound.binder),
                trait_symbol: SymbolHandle::invalid(),
                trait_name: lower_name(syntax_trees, &bound.trait_name),
                lifetime_arguments: bound
                    .lifetime_arguments
                    .iter()
                    .map(|name| lower_name(syntax_trees, name))
                    .collect(),
                arguments: lower_child_type_references(lowerer, syntax_trees, bound.arguments)?,
            })
        })
        .collect()
}

/*
    Retain the authored promise and witness without guessing a subject's
    carrier from its spelling. Typing selects canonical default views after
    parameter types and collection observations have exact identities.
    Neither stage establishes the checked termination guarantee.
*/
fn build_termination_plan(
    syntax_trees: &SyntaxTrees,
    machine: &syntax::item::Machine,
) -> language_semantics::MachineTerminationPlan {
    use language_semantics::{
        MachineTerminationPlan, RankingWitness, TerminationGuarantee, TerminationInterface,
    };

    let interface = if machine.terminates_guarantee {
        TerminationInterface::Published(TerminationGuarantee::Terminates {
            premises: Vec::new(),
        })
    } else if machine.boundary || machine.bodyless {
        TerminationInterface::Published(TerminationGuarantee::NoGuarantee)
    } else {
        TerminationInterface::InternalDerived
    };

    let subjects = syntax_expressions(syntax_trees).expression_handles(machine.ranking_subjects);
    let implementation_witness = (!subjects.is_empty()).then(|| {
        let (ranking_view, view_path) = authored_ranking_view(syntax_trees, machine);
        RankingWitness {
            subjects: subjects
                .iter()
                .map(|subject| render_ranked_subject(syntax_trees, *subject))
                .collect(),
            ranking_view,
            view_path,
            view_arguments: syntax_expressions(syntax_trees)
                .expression_handles(machine.ranking_view_arguments)
                .iter()
                .map(|argument| render_ranked_subject(syntax_trees, *argument))
                .collect(),
            rank_range: machine.ranking_range.is_valid().then(|| {
                let ExpressionNode::Range(range) =
                    syntax_trees.expressions.expression(machine.ranking_range)
                else {
                    return language_semantics::RankRange::default();
                };
                language_semantics::RankRange {
                    floor: render_ranked_subject(syntax_trees, range.start),
                    ceiling: render_ranked_subject(syntax_trees, range.end),
                    ceiling_inclusive: range.end_inclusive,
                }
            }),
        }
    });

    MachineTerminationPlan {
        interface,
        checked_summary: TerminationGuarantee::NoGuarantee,
        implementation_witness,
    }
}

/*
    Render one ranked subject as source-like text for the private witness
    for diagnostics; retained expression handles carry semantic identity.
*/
fn render_ranked_subject(
    syntax_trees: &SyntaxTrees,
    subject: syntax::expression::ExpressionHandle,
) -> String {
    match syntax_trees.expressions.expression(subject) {
        ExpressionNode::Integer(literal) => literal.to_string(),
        ExpressionNode::Name {
            members: path,
            application: path_application,
        } if path_application.is_none() => syntax_expressions(syntax_trees)
            .identifier_path_members(path.to_owned())
            .joined_path_with_in(syntax_trees, "."),
        ExpressionNode::Member(member) => format!(
            "{}.{}",
            render_ranked_subject(syntax_trees, member.receiver),
            member.member.as_str(syntax_trees)
        ),
        ExpressionNode::Binary(binary) if matches!(binary.operator, BinaryOperator::Subtract) => {
            format!(
                "{} - {}",
                render_ranked_subject(syntax_trees, binary.left),
                render_ranked_subject(syntax_trees, binary.right)
            )
        }
        _ => "value".to_string(),
    }
}

/*
    Retain an authored `-> View` verbatim, with a fixed identity for canonical
    builtins. An empty view requests default selection in typing; an unknown
    explicit path stays nonempty and must not acquire a default instead.
*/
fn authored_ranking_view(
    syntax_trees: &SyntaxTrees,
    machine: &syntax::item::Machine,
) -> (RankingViewId, String) {
    use language_semantics::RankingViewId;

    let order = syntax_items(syntax_trees).identifier_path_members(machine.ranking_view);
    if !order.is_empty() {
        let path = order.joined_path_in(syntax_trees);
        let id = RankingViewId::canonical(&path).unwrap_or(RankingViewId::NULL);
        return (id, path);
    }

    (RankingViewId::NULL, String::new())
}

fn lower_machine_ranking_view(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    order: HandleSpan<Identifier>,
) -> HandleSpan<DiagnosticName> {
    crate::lowering::name::lower_name_members(
        syntax_trees,
        order,
        &mut lowerer
            .symbol_resolved_trees
            .tables
            .declarations
            .ranking_views,
    )
}

fn lower_ranking_expressions(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    source_expressions: HandleSpan<syntax::expression::ExpressionHandle>,
) -> Result<HandleSpan<ExpressionHandle>, Diagnostic> {
    let mut lowered = Vec::new();

    for expression in syntax_expressions(syntax_trees).expression_handles(source_expressions) {
        let expression = lower_expression_into_table(lowerer, syntax_trees, *expression)?;
        lowered.push(expression);
    }

    Ok(expression_table(lowerer).insert_expression_handles(lowered))
}

fn lower_machine_trait_conformances(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    satisfies: HandleSpan<SatisfiesClause>,
) -> Result<HandleSpan<TraitConformance>, Diagnostic> {
    let mut span = HandleSpan::empty();

    for clause in syntax_trees.items.satisfies_clauses(satisfies) {
        let arguments = lower_child_type_references(lowerer, syntax_trees, clause.arguments)?;
        let external_binding = clause.via.as_ref().map(|binding| {
            lowerer
                .symbol_resolved_trees
                .external_bindings
                .intern(external_binding_identity(syntax_trees, binding))
        });
        let via_expression = clause
            .via_expression
            .is_valid()
            .then(|| lower_expression_into_table(lowerer, syntax_trees, clause.via_expression))
            .transpose()?
            .unwrap_or_else(ExpressionHandle::invalid);
        lowerer
            .symbol_resolved_trees
            .tables
            .declarations
            .machine_trait_conformances
            .append_to_span(
                &mut span,
                TraitConformance {
                    symbol: SymbolHandle::invalid(),
                    name: lower_name(syntax_trees, &clause.trait_name),
                    lifetime_arguments: lower_names(syntax_trees, &clause.lifetime_arguments),
                    arguments,
                    requirement: clause
                        .requirement
                        .as_ref()
                        .map(|name| lower_name(syntax_trees, name)),
                    alias: clause
                        .alias
                        .as_ref()
                        .map(|name| lower_name(syntax_trees, name)),
                    external_binding,
                    via_expression,
                    external_binding_source_span: clause.via_keyword_source_span,
                },
            );
    }

    Ok(span)
}

fn external_binding_identity(
    syntax_trees: &syntax_trees::SyntaxTrees,
    binding: &ExternalBinding,
) -> language_semantics::ExternalBindingIdentity {
    use language_semantics::ExternalBindingIdentity;

    match binding {
        ExternalBinding::Syscall { number } => ExternalBindingIdentity::Syscall { number: *number },
        ExternalBinding::CompilerIntrinsic => ExternalBindingIdentity::CompilerIntrinsic,
        ExternalBinding::VtableSlot { index } => {
            ExternalBindingIdentity::VtableSlot { index: *index }
        }
        ExternalBinding::VtableField { field } => ExternalBindingIdentity::VtableField {
            field: field.as_str(syntax_trees).to_owned(),
        },
        ExternalBinding::TableFunction { field } => ExternalBindingIdentity::TableFunction {
            field: field.as_str(syntax_trees).to_owned(),
        },
    }
}

fn lower_machine_states(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    states: HandleSpan<StateHandle>,
    is_public: bool,
) -> Result<HandleSpan<Handle<State>>, Diagnostic> {
    let mut span = HandleSpan::empty();

    for (ordinal, state) in syntax_trees.items.state_handles(states).iter().enumerate() {
        let authored_type = syntax_trees.items.state(*state).return_type;
        let state = lower_state_node(
            lowerer,
            syntax_trees,
            syntax_trees.items.state(*state),
            is_public && ordinal == 0,
        )?;
        let state = lowerer
            .symbol_resolved_trees
            .tables
            .declarations
            .machine_states
            .append(state);
        if authored_type.is_valid()
            && let Some(sources) = &mut lowerer.equation_sources
        {
            sources.declared_types.push((
                crate::preparation::machine_equations::DeclaredTypeOrigin::Result(state),
                authored_type,
            ));
        }
        lowerer
            .symbol_resolved_trees
            .tables
            .declarations
            .machine_state_handles
            .append_to_span(&mut span, state);
    }

    Ok(span)
}

/*
    The sealed intrinsic catalog row this authored declaration matches, if
    any. Lowering reads the catalog before the machine has a symbol, from the
    same toolchain file and declaration path the symbol table later reports.
*/
fn sealed_intrinsic(
    lowerer: &Lowerer,
    syntax_trees: &SyntaxTrees,
    machine: &syntax::item::Machine,
) -> Option<language_core::sealed_intrinsics::SealedIntrinsic> {
    let file = lowerer
        .sources
        .as_ref()?
        .get(machine.name.source_span().source_id)?;
    symbols::sealed_intrinsic_at(file, machine.name.as_str(syntax_trees))
}

/*
    A core `NativeFloats` member declares implicit compiler-intrinsic supply
    only in its exact bodyless single-relation shape. Typed validation must
    still authenticate the exact public relation, complete scalar signature
    and numeric meaning before it can be consumed.
*/
fn is_sealed_float_provider_member(
    lowerer: &Lowerer,
    syntax_trees: &SyntaxTrees,
    machine: &syntax::item::Machine,
) -> bool {
    let [edge] = syntax_trees.items.satisfies_clauses(machine.satisfies) else {
        return false;
    };
    machine.bodyless
        && machine.boundary
        && machine.attached_data.is_none()
        && edge.via.is_none()
        && !edge.via_expression.is_valid()
        && sealed_intrinsic(lowerer, syntax_trees, machine)
            == Some(language_core::sealed_intrinsics::SealedIntrinsic::NativeFloatOperation)
}

/*
    Classify exact bundled compiler operations before deciding whether a
    bodyless guarantee is an authored admission claim. Typed checking and
    execution still validate the signature and discharge its operation duties.
*/
fn compiler_owned_boundary(
    lowerer: &Lowerer,
    syntax: &SyntaxTrees,
    machine: &syntax::item::Machine,
) -> bool {
    matches!(
        sealed_intrinsic(lowerer, syntax, machine),
        Some(language_core::sealed_intrinsics::SealedIntrinsic::StaticInterrupt(_))
    )
}
