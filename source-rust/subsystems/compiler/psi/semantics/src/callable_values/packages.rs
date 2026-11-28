//! Callable values keep their original descriptor IDs through a package.
//! These rows change the type of the query only at a checked record field
//! write, a checked field read, or the matching indirect call. They supply
//! no candidate callees.
use super::*;
use semantic_vocabulary::StructuralFieldId;
use std::collections::BTreeMap;

#[derive(Default)]
pub struct PackageCallableSites {
    writes: BTreeMap<
        (DescriptorPosition, DescriptorLocation),
        (CallableContractId, CallableContractId),
    >,
    calls: Vec<OpenedCall>,
}

struct OpenedCall {
    operation: OperationId,
    contract: CallableContractId,
    admitted: Vec<CallableContractId>,
    stored_referents: Vec<Option<StructuralTypeId>>,
}

impl PackageCallableSites {
    pub fn record_field(
        &mut self,
        types: &[StructuralTypeDeclaration],
        position: DescriptorPosition,
        record: &terminal_psi::StructuralOperationResult,
        field: StructuralFieldId,
        actual: &terminal_psi::StructuralFieldType,
    ) -> CallableValueResult<()> {
        self.stored_field(
            types,
            position,
            (record.place, record.structural_type),
            (&[], field),
            actual,
        )
    }

    /// Retain the concrete contract only for the exact write whose package
    /// receipt admitted this callable field. This records a type transition;
    /// the ordinary storage equations still recover the descriptor itself.
    pub fn stored_field(
        &mut self,
        types: &[StructuralTypeDeclaration],
        position: DescriptorPosition,
        (destination, root): (PlaceId, StructuralTypeId),
        (path, field): (&[terminal_psi::StructuralPathSegment], StructuralFieldId),
        actual: &terminal_psi::StructuralFieldType,
    ) -> CallableValueResult<()> {
        let Some(ScalarType::Callable(actual)) = actual.scalar_type() else {
            return Ok(());
        };
        let declaration = crate::projected_field_declaration(types.iter(), root, path, field)
            .ok_or("package callable write names an absent field")?;
        let Some(ScalarType::Callable(public)) = declaration.field_type.scalar_type() else {
            return Err("package callable write names a non-callable field");
        };
        if actual != public {
            let location = crate::descriptor_lifetimes::descriptor_location(
                types,
                destination,
                root,
                path,
                Some(field),
            )
            .ok_or("package callable write has no storage path")?;
            if self
                .writes
                .insert((position, location), (public, actual))
                .is_some()
            {
                return Err("package callable field has two writes at one site");
            }
        }
        Ok(())
    }

    pub fn opened_call(
        &mut self,
        catalog: &TerminalCallableCatalog,
        operation: OperationId,
        contract: CallableContractId,
        admitted: Vec<CallableContractId>,
        referents: impl IntoIterator<Item = (usize, StructuralTypeId)>,
    ) -> CallableValueResult<()> {
        let interface = catalog
            .contract(contract)
            .ok_or("package call lacks its opened contract")?;
        let mut stored_referents = vec![None; interface.structural_parameters.len()];
        for (i, public) in referents {
            let formal = interface
                .structural_parameters
                .get(i)
                .ok_or("package call names an absent borrowed parameter")?;
            if formal.access == StructuralAccess::Owned
                || stored_referents[i].replace(public).is_some()
            {
                return Err("package call names an invalid borrowed parameter");
            }
        }
        let Err(index) = self
            .calls
            .binary_search_by_key(&operation, |call| call.operation)
        else {
            return Err("package call has two openings");
        };
        self.calls.insert(
            index,
            OpenedCall {
                operation,
                contract,
                admitted,
                stored_referents,
            },
        );
        Ok(())
    }

    pub(super) fn admits_write(
        &self,
        position: DescriptorPosition,
        location: &DescriptorLocation,
        public: CallableContractId,
        actual: CallableContractId,
    ) -> bool {
        self.writes.get(&(position, location.clone())) == Some(&(public, actual))
    }

    fn call(&self, invocation: &CallableInvocation) -> Option<&OpenedCall> {
        let CallableInvocationTarget::Value(_, contract) = invocation.target else {
            return None;
        };
        let index = self
            .calls
            .binary_search_by_key(&invocation.operation?, |call| call.operation)
            .ok()?;
        let call = &self.calls[index];
        (call.contract == contract).then_some(call)
    }

    pub(super) fn admits_descriptor(
        &self,
        invocation: &CallableInvocation,
        actual: CallableContractId,
    ) -> bool {
        self.call(invocation)
            .is_some_and(|call| call.admitted.contains(&actual))
    }

    pub(super) fn stored_argument(
        &self,
        invocation: &CallableInvocation,
        index: usize,
    ) -> Option<StructuralTypeId> {
        self.call(invocation)?
            .stored_referents
            .get(index)
            .copied()
            .flatten()
    }
}

pub fn opened_stored_callable_definition(
    types: &[StructuralTypeDeclaration],
    flow: &DescriptorMachine,
    position: DescriptorPosition,
    value: ValueId,
) -> CallableValueResult<CallableDefinition> {
    let Some(CallableDefinition::Stored { location, .. }) =
        stored_callable_definition(flow, position, value)
    else {
        return Err("package descriptor projection lost its storage origin");
    };
    let root = *flow
        .place_types
        .get(&location.root)
        .ok_or("package descriptor has no storage root")?;
    let path = crate::descriptor_lifetimes::descriptor_path(types, root, &location.path)
        .ok_or("package descriptor has an invalid storage path")?;
    let Some(ScalarType::Callable(stored)) =
        crate::runtime_structural_field(types.iter(), root, &path)
            .and_then(|field| field.field_type.scalar_type())
    else {
        return Err("package descriptor has no public storage contract");
    };
    Ok(CallableDefinition::Stored {
        position,
        location,
        storage_contract: Some(stored),
    })
}
