//! Parameter-rooted argument paths: which caller parameter an argument place
//! names, and which checked path its segments lend to the callee formal. Each
//! call lane keeps one admission table whose first admitting row decides. The
//! formal's access stays an `Option` because a nested reference has none: it
//! rejects the argument at the first row that reads it, never earlier, so a
//! row deciding without it keeps its outcome and its caller's trace phase.

use crate::execution::terminal_unit::calls::argument_paths::{
    fixed_byte_array_range_path, open_byte_range_path, projected_argument_path,
    projected_borrowed_path, projected_evaluated_path,
};
use crate::execution::terminal_unit::calls::argument_presentation::ArgumentPresentation;
use crate::execution::terminal_unit::calls::boundary_admission::fixed_byte_array_view_is_admitted;
use crate::execution::terminal_unit::types::{
    base_type_identity, byte_sequence_carrier, parameter_is_root,
};
use crate::execution::terminal_unit::{
    CheckedStructuralAccess, CheckedUnitStructuralParameterPlan, CheckedUnitStructuralPathSegment,
    MachineSupplyMode, Multiplicity, StateParameter, SymbolHandle, TypeReferenceHandle,
};
use CheckedStructuralAccess::{MutableBorrow, Owned, SharedBorrow, WriteOnlyBorrow};

type ArgumentPath = Vec<CheckedUnitStructuralPathSegment>;

/// The caller parameter an argument place is rooted at, with the dense index
/// of its structural plan.
#[derive(Clone, Copy)]
pub(crate) struct SourceParameter<'parameters> {
    pub(crate) declaration: &'parameters StateParameter,
    pub(crate) plan: &'parameters CheckedUnitStructuralParameterPlan,
    pub(crate) index: usize,
}

impl<'parameters> SourceParameter<'parameters> {
    pub(crate) fn resolve(
        declarations: &'parameters [StateParameter],
        plans: &'parameters [CheckedUnitStructuralParameterPlan],
        root: SymbolHandle,
    ) -> Option<Self> {
        let declaration = declarations
            .iter()
            .find(|parameter| parameter_is_root(parameter, root))?;
        let position =
            typed_trees::signature::state_parameter_position(declarations, declaration.symbol)?;
        let position = u32::try_from(position).unwrap_or(u32::MAX);
        let (index, plan) = plans
            .iter()
            .enumerate()
            .find(|(_, plan)| plan.position == position)?;
        Some(Self {
            declaration,
            plan,
            index,
        })
    }

    /// What the parameter lends to a formal of `target_access`.
    pub(crate) fn custody(self, target_access: Option<CheckedStructuralAccess>) -> ArgumentCustody {
        ArgumentCustody {
            access: self.plan.access,
            multiplicity: self.plan.multiplicity,
            unqualified: self.plan.qualifications.is_empty(),
            target_access,
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct ArgumentCustody {
    pub(crate) access: CheckedStructuralAccess,
    pub(crate) multiplicity: Multiplicity,
    pub(crate) unqualified: bool,
    pub(crate) target_access: Option<CheckedStructuralAccess>,
}

impl ArgumentCustody {
    fn lends_borrow(self) -> bool {
        matches!(self.access, MutableBorrow | SharedBorrow)
    }

    /// The custody that lends a fixed byte-array window or view.
    pub(crate) fn lends_unrestricted_borrow(self) -> bool {
        self.lends_borrow() && self.multiplicity == Multiplicity::Unrestricted && self.unqualified
    }
}

/// An argument place's segments below its root, classified once.
enum ArgumentPathShape {
    Whole,
    /// A fixed window closing any prefix, `[.., FixedRange]`.
    FixedRange,
    /// Statically selected storage, including exact case payloads.
    Static {
        fields: usize,
        fixed_indexes: usize,
        case_qualified: bool,
    },
    /// An unnormalized `Index` (a runtime element or an open-ended range)
    /// among field and fixed-index selectors.
    RuntimeIndexed,
    /// An interior range, which no argument path spells.
    Unsupported,
}

impl ArgumentPathShape {
    fn of(segments: &[facts::PlaceSegment]) -> Self {
        use facts::PlaceSegment::{Case, Field, FixedIndex, FixedRange, Index, OpaqueRange};
        match segments.last() {
            None => return Self::Whole,
            Some(FixedRange { .. }) => return Self::FixedRange,
            Some(_) => {}
        }
        let (mut fields, mut fixed_indexes, mut runtime_indexed) = (0, 0, false);
        let mut case_qualified = false;
        for segment in segments {
            match segment {
                Field { .. } => fields += 1,
                FixedIndex { .. } => fixed_indexes += 1,
                Index { .. } => runtime_indexed = true,
                Case { .. } => case_qualified = true,
                FixedRange { .. } | OpaqueRange => return Self::Unsupported,
            }
        }
        if runtime_indexed && case_qualified {
            // Runtime case selection needs the call's dynamic path evidence;
            // the ordinary runtime-index presentation has not retained it.
            Self::Unsupported
        } else if runtime_indexed {
            Self::RuntimeIndexed
        } else {
            Self::Static {
                fields,
                fixed_indexes,
                case_qualified,
            }
        }
    }
}

/// One parameter-rooted argument: the canonical place the caller names and
/// the callee formal it must present as.
#[derive(Clone, Copy)]
pub(crate) struct ParameterArgument<'argument> {
    pub(crate) program: &'argument dyn ::validation::CheckingInput,
    pub(crate) machine: SymbolHandle,
    pub(crate) state: SymbolHandle,
    pub(crate) statement_index: usize,
    pub(crate) place: &'argument validation::CanonicalPlace,
    pub(crate) target: TypeReferenceHandle,
    pub(crate) target_identity: typed_trees::type_identity::TypeIdentityId,
    pub(crate) selectors: Option<
        &'argument dyn Fn(
            typed_trees::expression::ExpressionHandle,
        ) -> Option<CheckedUnitStructuralPathSegment>,
    >,
}

/// The path an ordinary call lends from a caller parameter.
pub(crate) fn ordinary_parameter_path(
    argument: ParameterArgument<'_>,
    custody: ArgumentCustody,
    supply_mode: MachineSupplyMode,
) -> Option<ArgumentPath> {
    let checked_body = supply_mode == MachineSupplyMode::CheckedBody;
    let unrestricted = custody.multiplicity == Multiplicity::Unrestricted;
    let access = custody.access;
    match ArgumentPathShape::of(&argument.place.segments) {
        ArgumentPathShape::Whole => Some(Vec::new()),
        // A qualified parameter passes only whole.
        _ if !custody.unqualified => None,
        ArgumentPathShape::FixedRange
            if (checked_body || supply_mode.is_boundary_declaration())
                && custody.lends_unrestricted_borrow() =>
        {
            argument.fixed_range_path()
        }
        // A borrowed byte-view formal takes the boundary presentation rule; an
        // unrestricted field path may also lend a fixed byte array as the view.
        ArgumentPathShape::Static { fixed_indexes, .. }
            if checked_body
                && custody.lends_borrow()
                && matches!(custody.target_access?, MutableBorrow | SharedBorrow)
                && argument.target_is_byte_view() =>
        {
            argument.presented_projection_path(&[], unrestricted && fixed_indexes == 0)
        }
        // Static projection changes the selected referent, not loan
        // authority: a borrow lends the same or a weaker access at every
        // depth, and the caller still rejoins authored access and aliases.
        ArgumentPathShape::Static { .. }
            if checked_body
                && unrestricted
                && matches!(
                    (access, custody.target_access?),
                    (
                        MutableBorrow,
                        SharedBorrow | MutableBorrow | WriteOnlyBorrow
                    ) | (SharedBorrow, SharedBorrow)
                        | (WriteOnlyBorrow, WriteOnlyBorrow)
                ) =>
        {
            argument.slice_view_projection_path()
        }
        // An owned affine member moves into an owned formal. The exact path
        // walker retains a Case segment's declaration; checked permissions own
        // membership, and execution rejoins the stored discriminator. The guard
        // does not change the move or its claim transfer contract.
        // The caller's scalar or Unit result does not change this custody.
        ArgumentPathShape::Static { .. }
            if checked_body
                && custody.multiplicity == Multiplicity::Affine
                && access == Owned
                && custody.target_access? == Owned =>
        {
            argument.exact_projection_path()
        }
        // Other case-qualified presentations still need their own access and
        // ABI contracts; membership alone cannot authorize one.
        ArgumentPathShape::Static {
            case_qualified: true,
            ..
        } => None,
        ArgumentPathShape::Static {
            fields: 0,
            fixed_indexes: 1..=3,
            ..
        } => argument.exact_projection_path(),
        ArgumentPathShape::Static { fields: 0, .. }
            if checked_body
                && access == WriteOnlyBorrow
                && custody.target_access? == WriteOnlyBorrow =>
        {
            argument.exact_projection_path()
        }
        ArgumentPathShape::Static {
            fixed_indexes: 0, ..
        } => argument.exact_projection_path(),
        // A runtime index loans one element of a borrowed subtree without
        // transferring ownership, the same subloan the receiver lane admits;
        // a shared source cannot lend mutation.
        ArgumentPathShape::RuntimeIndexed
            if checked_body
                && match custody.target_access? {
                    SharedBorrow => {
                        custody.lends_borrow()
                            || (access == Owned && custody.multiplicity == Multiplicity::Affine)
                    }
                    MutableBorrow => access == MutableBorrow,
                    _ => false,
                } =>
        {
            argument.runtime_indexed_path(custody)
        }
        _ => None,
    }
}

/// The path a boundary-requirement call lends from a caller parameter: a
/// fixed byte window from an unrestricted borrow, and any other projection
/// through the formal's ABI presentation. The caller's access replay is the
/// remaining custody check; a runtime selector passes only when it folds to a
/// constant.
pub(crate) fn boundary_parameter_path(
    argument: ParameterArgument<'_>,
    custody: ArgumentCustody,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
) -> Option<ArgumentPath> {
    match ArgumentPathShape::of(&argument.place.segments) {
        ArgumentPathShape::Whole => Some(Vec::new()),
        // A qualified parameter passes only whole.
        _ if !custody.unqualified => None,
        ArgumentPathShape::FixedRange if custody.lends_unrestricted_borrow() => {
            argument.fixed_range_path()
        }
        ArgumentPathShape::Static {
            case_qualified: false,
            ..
        }
        | ArgumentPathShape::RuntimeIndexed => {
            argument.presented_projection_path(substitutions, true)
        }
        _ => None,
    }
}

impl ParameterArgument<'_> {
    /// Whole storage and projected storage use the same carrier presentation.
    /// This proves representation, not permission: the call's access and
    /// predicate checks still govern the loan. The separate fixed-array rule
    /// retains its unrestricted, unqualified custody requirement.
    pub(crate) fn whole_parameter_is_admitted(
        self,
        source: SourceParameter<'_>,
        byte_view: bool,
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    ) -> bool {
        source.plan.type_identity == self.target_identity
            || super::argument_presentation::classify(
                self.program,
                source.declaration.type_reference,
                self.target,
                self.target_identity,
                substitutions,
            )
            .is_some_and(|presentation| match presentation {
                ArgumentPresentation::FixedArraySlice => {
                    byte_view
                        && fixed_byte_array_view_is_admitted(
                            self.program,
                            source.declaration.type_reference,
                            self.target,
                        )
                }
                _ => true,
            })
    }

    fn target_is_byte_view(self) -> bool {
        byte_sequence_carrier(self.program, self.target, &[]).is_some_and(|carrier| {
            carrier.is_borrowed()
                || matches!(
                    carrier,
                    checked_trees::CheckedByteSequenceCarrier::BoundedOwned { .. }
                )
        })
    }

    fn fixed_range_path(self) -> Option<ArgumentPath> {
        let (program, place, target) = (self.program, self.place, self.target);
        fixed_byte_array_range_path(program, self.state, self.statement_index, place, target)
            .map(|(_, path)| path)
    }

    fn exact_projection_path(self) -> Option<ArgumentPath> {
        let (selected, path) = projected_argument_path(
            self.program,
            self.state,
            self.statement_index,
            self.place,
            None,
            None,
        )?;
        let (selected, path) = super::argument_paths::borrowed_argument_endpoint(
            self.program,
            selected,
            path,
            self.target,
        )?;
        (base_type_identity(self.program, selected, &[], &[])? == self.target_identity)
            .then_some(path)
    }

    /// A projection at the formal's identity, or a fixed array lent as the
    /// formal's borrowed slice view.
    fn slice_view_projection_path(self) -> Option<ArgumentPath> {
        let (program, target) = (self.program, self.target);
        let (projected, path) = projected_borrowed_path(
            program,
            self.machine,
            self.state,
            self.statement_index,
            self.place,
        )?;
        let (projected, path) =
            super::argument_paths::borrowed_argument_endpoint(program, projected, path, target)?;
        let presentation = super::argument_presentation::classify(
            program,
            projected,
            target,
            self.target_identity,
            &[],
        )?;
        (presentation != ArgumentPresentation::ByteSequence).then_some(path)
    }

    /// A projection the formal presents through the carrier presentation
    /// rule or, when `byte_view` allows, as a fixed byte array's byte view.
    fn presented_projection_path(
        self,
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
        byte_view: bool,
    ) -> Option<ArgumentPath> {
        let (program, target) = (self.program, self.target);
        let (projected, path) = projected_argument_path(
            program,
            self.state,
            self.statement_index,
            self.place,
            None,
            None,
        )?;
        let (projected, path) =
            super::argument_paths::borrowed_argument_endpoint(program, projected, path, target)?;
        super::argument_presentation::classify(
            program,
            projected,
            target,
            self.target_identity,
            substitutions,
        )
        .is_some_and(|presentation| match presentation {
            ArgumentPresentation::FixedArraySlice => {
                byte_view && fixed_byte_array_view_is_admitted(program, projected, target)
            }
            _ => true,
        })
        .then_some(path)
    }

    /// The open-ended spelling `x[..]`/`x[a..]` survives canonicalization as
    /// an `Index` carrying its range, since its end is not an authored
    /// constant; against a fixed byte array it is the fixed-range argument
    /// ending at the backing's literal extent. Any other `Index` keeps the
    /// runtime-selector route: `projected_borrowed_path` retains the selector
    /// only when the caller's published integer range proves it inside the
    /// enclosing fixed array's extent, and terminal verification replays it.
    fn runtime_indexed_path(self, custody: ArgumentCustody) -> Option<ArgumentPath> {
        let (program, state, statement_index, place) =
            (self.program, self.state, self.statement_index, self.place);
        if custody.lends_unrestricted_borrow()
            && let Some((_, path)) =
                open_byte_range_path(program, state, statement_index, place, self.target)
        {
            return Some(path);
        }
        let (projected, path) = projected_evaluated_path(
            program,
            self.machine,
            state,
            statement_index,
            place,
            self.selectors,
        )?;
        let (projected, path) = super::argument_paths::borrowed_argument_endpoint(
            program,
            projected,
            path,
            self.target,
        )?;
        // A captured selector changes storage coordinates, not the formal's
        // presentation. Static and evaluated projections share this relation.
        let presentation = super::argument_presentation::classify(
            program,
            projected,
            self.target,
            self.target_identity,
            &[],
        )?;
        (presentation != ArgumentPresentation::FixedArraySlice
            || custody.lends_unrestricted_borrow())
        .then_some(path)
    }
}
