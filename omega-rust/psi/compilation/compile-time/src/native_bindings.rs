//! Source admission of reached native Build choices. Requirement applications,
//! table declarations and stack promises are joined here before Omega owns
//! locator normalization, receipts, calling geometry and provider settlement.

use diagnostics::Diagnostic;
use language_semantics::declaration_selection::{
    CapturedNativeDeclaration, NativeBuildIntent, ProductTarget,
};
use semantic_vocabulary::{NativeBindingLocator, PackageKeyIdentity};
use symbols::SymbolHandle;
use typed_trees::{TypedTrees, trait_definition::RetainedTraitScope};

/// The entire applied requirement, including unused outer and local actuals.
/// An absent scope/member instance denotes the declaration's universal schema.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeRequirementApplication {
    pub requirement: SymbolHandle,
    pub closed_requirement: Option<SymbolHandle>,
    pub trait_scope: Option<RetainedTraitScope>,
}
impl NativeRequirementApplication {
    pub fn callable(self) -> SymbolHandle {
        self.closed_requirement.unwrap_or(self.requirement)
    }

    fn coordinates(
        self,
        typed: &TypedTrees,
    ) -> Result<(String, Option<String>, Option<String>), String> {
        let declared =
            if let Some((owner, signature)) = typed.trait_machine_signature(self.requirement) {
                typed.trait_requirement_callable_identity_text(owner, signature)
            } else {
                typed
                    .machine(self.requirement)
                    .and_then(|machine| typed.machine_callable_identity_text(machine))
                    .ok_or("native requirement lost its callable declaration identity")?
            };
        let outer = self
            .trait_scope
            .map(|retained| {
                retained
                    .scope(typed)
                    .and_then(|scope| scope.portable_application_identity(typed))
                    .and_then(|identity| typed.type_identity_text(identity))
                    .ok_or_else(|| {
                        "native requirement lost its complete outer application identity".to_owned()
                    })
            })
            .transpose()?;
        let local = self
            .closed_requirement
            .map(|closed| {
                let application = typed
                    .closed_requirement_application(closed)
                    .filter(|application| application.template == self.requirement)
                    .ok_or("native requirement lost its exact closed member application")?;
                let (owner, _) = typed
                    .trait_machine_signature(self.requirement)
                    .ok_or("closed native member lost its declaring trait")?;
                Ok::<_, String>(
                    typed.trait_requirement_callable_identity_text(owner, application.signature),
                )
            })
            .transpose()?;
        Ok((declared, outer, local))
    }

    pub fn identity(self, typed: &TypedTrees) -> Result<String, String> {
        let (declaration, outer, local) = self.coordinates(typed)?;
        Ok(format!("{declaration:?};outer={outer:?};member={local:?}"))
    }

    fn overlaps(self, other: Self, typed: &TypedTrees) -> Result<bool, String> {
        let left = self.coordinates(typed)?;
        let right = other.coordinates(typed)?;
        Ok(left.0 == right.0
            && left
                .1
                .as_ref()
                .zip(right.1.as_ref())
                .is_none_or(|(left, right)| left == right)
            && left
                .2
                .as_ref()
                .zip(right.2.as_ref())
                .is_none_or(|(left, right)| left == right))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeTableField {
    pub table: SymbolHandle,
    pub field: SymbolHandle,
    pub drops_receiver: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeBindingSupply {
    Locator(NativeBindingLocator),
    Table(NativeTableField),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeSameStackDemand {
    pub bytes: u64,
    pub producer: language_semantics::declaration_selection::NativeBindingProducer,
    pub source_occurrence_digest: [u8; 32],
    pub source_span: source::SourceSpan,
    pub event_ordinal: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeBindingSelection {
    pub subject: SymbolHandle,
    pub application: NativeRequirementApplication,
    pub target: ProductTarget,
    pub supply: NativeBindingSupply,
    pub same_stack_demand: Option<NativeSameStackDemand>,
    pub producer: language_semantics::declaration_selection::NativeBindingProducer,
    pub source_occurrence_digest: [u8; 32],
    pub supplier_package: PackageKeyIdentity,
    pub source_span: source::SourceSpan,
}

/// One source-prepared reached operation. The mutable Psi continuation owns
/// preparation; this immutable join cannot construct another source instance.
pub struct PreparedNativeIntent {
    pub application: NativeRequirementApplication,
    pub table: Option<NativeTableField>,
    pub declaration: CapturedNativeDeclaration,
    pub target: ProductTarget,
    pub intent: NativeBuildIntent,
    pub source_span: source::SourceSpan,
    pub event_ordinal: usize,
}

pub fn requirement_owner(
    typed: &TypedTrees,
    requirement: SymbolHandle,
) -> Result<SymbolHandle, String> {
    if let Some((owner, _)) = typed.trait_machine_signature(requirement) {
        return owner
            .is_boundary
            .then_some(owner.symbol)
            .ok_or_else(|| "native binding must designate a boundary requirement".into());
    }
    typed
        .machine(requirement)
        .filter(|machine| {
            machine.supply_mode == language_semantics::MachineSupplyMode::TopLevelRequirement
        })
        .map(|machine| machine.symbol)
        .ok_or_else(|| "native binding lost its exact boundary declaration".into())
}

/// Join separately reached footprints only within this completed activation.
/// The full application, target and supplier identify a row; equal footprints
/// retain their original occurrence while conflicting promises refuse.
pub fn collect(
    typed: &TypedTrees,
    intents: &[PreparedNativeIntent],
) -> Result<Vec<NativeBindingSelection>, Vec<Diagnostic>> {
    let fail = |intent: &PreparedNativeIntent, reason: &str| {
        vec![Diagnostic::error(reason).with_source_span(intent.source_span)]
    };
    let keys = intents
        .iter()
        .map(|intent| {
            intent
                .application
                .identity(typed)
                .map_err(|reason| fail(intent, &reason))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let same_key = |left: usize, right: usize| {
        keys[left] == keys[right]
            && intents[left].target == intents[right].target
            && intents[left].declaration.producer.package
                == intents[right].declaration.producer.package
    };
    let mut selections: Vec<NativeBindingSelection> = Vec::new();
    for (index, intent) in intents.iter().enumerate() {
        if !intent
            .intent
            .matches_operation(intent.declaration.request.operation)
        {
            return Err(fail(
                intent,
                "native effect has a different source operation",
            ));
        }
        if let NativeBuildIntent::SameStackDemand { bytes } = intent.intent {
            if bytes == 0 {
                return Err(fail(intent, "native same-stack demand must be positive"));
            }
            let mut matching = intents.iter().enumerate().filter(|(other_index, other)| {
                same_key(index, *other_index)
                    && !matches!(other.intent, NativeBuildIntent::SameStackDemand { .. })
            });
            if matching.next().is_none() {
                return Err(fail(
                    intent,
                    "native same-stack demand has no reached binding in this activation",
                ));
            }
            continue;
        }
        let subject = requirement_owner(typed, intent.application.requirement)
            .map_err(|reason| fail(intent, &reason))?;
        let supply = match &intent.intent {
            NativeBuildIntent::Locator(locator) => NativeBindingSupply::Locator(locator.clone()),
            NativeBuildIntent::Table { drops_receiver } => {
                let field = intent
                    .table
                    .filter(|field| field.drops_receiver == *drops_receiver)
                    .ok_or_else(|| {
                        fail(intent, "native table operation lost its exact typed field")
                    })?;
                NativeBindingSupply::Table(field)
            }
            NativeBuildIntent::SameStackDemand { .. } => unreachable!(),
        };
        let mut demand = None;
        for (_, footprint) in intents
            .iter()
            .enumerate()
            .filter(|(other_index, _)| same_key(index, *other_index))
        {
            let NativeBuildIntent::SameStackDemand { bytes } = footprint.intent else {
                continue;
            };
            if demand
                .as_ref()
                .is_some_and(|prior: &NativeSameStackDemand| prior.bytes != bytes)
            {
                return Err(fail(
                    footprint,
                    "conflicting same-stack demands for one native requirement application",
                ));
            }
            demand.get_or_insert_with(|| NativeSameStackDemand {
                bytes,
                producer: footprint.declaration.producer.clone(),
                source_occurrence_digest: footprint.declaration.source_occurrence_digest,
                source_span: footprint.source_span,
                event_ordinal: footprint.event_ordinal,
            });
        }
        for prior in &selections {
            if prior.target == intent.target
                && prior.supplier_package == intent.declaration.producer.package
                && prior
                    .application
                    .overlaps(intent.application, typed)
                    .map_err(|reason| fail(intent, &reason))?
                && (prior
                    .application
                    .identity(typed)
                    .map_err(|reason| fail(intent, &reason))?
                    != keys[index]
                    || prior.supply != supply)
            {
                return Err(fail(
                    intent,
                    "overlapping native bindings for one full requirement application, target and supplier",
                ));
            }
        }
        if selections.iter().any(|prior| {
            prior.application.identity(typed).ok().as_ref() == Some(&keys[index])
                && prior.target == intent.target
                && prior.supplier_package == intent.declaration.producer.package
                && prior.supply != supply
        }) {
            return Err(fail(
                intent,
                "conflicting native bindings for the same full requirement application, target and supplier",
            ));
        }
        selections.push(NativeBindingSelection {
            subject,
            application: intent.application,
            target: intent.target,
            supply,
            same_stack_demand: demand,
            producer: intent.declaration.producer.clone(),
            source_occurrence_digest: intent.declaration.source_occurrence_digest,
            supplier_package: intent.declaration.producer.package,
            source_span: intent.source_span,
        });
    }
    Ok(selections)
}

mod capture;
pub use capture::capture_package_intents;
