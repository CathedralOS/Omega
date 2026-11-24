use access_vocabulary::AtomicPermissions as DomainAtomicPermissions;
use access_vocabulary::{AccessPermissions, ObservationDemand};

use crate::CodecError;
use crate::fields::{
    ContentProjection, Field, Flag, Id, Ids, Index, List, Nominal, Optional, Proposition,
    wire_enum, wire_record,
};
use crate::wire::{Reader, Writer};
use semantic_vocabulary::DomainSemanticId;
use terminal_psi::{
    ByteSequencePredicate, InstallationReachDependency, ServiceDeclaration,
    StructuralDomainDeclaration, StructuralDomainPredicate, StructuralEstablishmentRoute,
};

wire_record! {
    Domain: StructuralDomainDeclaration {
        id: Id,
        semantic_domain: Id,
        required_memberships: Optional<Ids>,
        complete_predicate_theory: Flag,
        identity: Index,
        carrier: Id,
        subject_identity: Index,
        predicate: Optional<DomainPredicate>,
        byte_predicate: OptionalBytePredicate,
        content_projection: Optional<ContentProjection>,
        establishment_routes: Optional<List<Route>>,
        access_policy: Optional<DomainAccess>,
    }
    DomainPredicate: StructuralDomainPredicate { subject: Id, proposition: Proposition }
    Service: ServiceDeclaration { id: Id, identity: Index, parents: Ids }
    ReachDependency: InstallationReachDependency { requirement_identity: Index, upper_bound: Ids }
}

wire_enum! {
    Route: StructuralEstablishmentRoute, "StructuralEstablishmentRoute" {
        4 => CheckedRequirement { declaration: Nominal, requirement_identity: Index },
        5 => ExactMachine { declaration: Nominal, machine_identity: Index },
        6 => BoundaryRequirement { declaration: Nominal, requirement_identity: Index },
    }
}

type Policy = terminal_psi::StructuralDomainAccessPlan;
type Region = (
    schema_reflection::SchemaNodeHandle,
    Option<Vec<terminal_psi::StructuralPathSegment>>,
);
type Caller = (
    schema_reflection::CallableDescription,
    StructuralEstablishmentRoute,
);
type Observation = Option<(ObservationDemand, access_vocabulary::AccessExposure)>;

wire_record! {
    Permissions: AccessPermissions {
        read: Flag, take: Flag, write: Flag, atomic: AtomicPermissions,
    }
    AtomicPermissions: DomainAtomicPermissions {
        load: Flag, store: Flag, fetch_add: Flag, fetch_sub: Flag,
        fetch_xor: Flag, fetch_or: Flag, fetch_and: Flag, swap: Flag,
        compare_exchange: Flag, compare_exchange_once: Flag,
        try_exchange: Flag, try_exchange_once: Flag,
    }
}

struct DomainRegion;
impl Field<Region> for DomainRegion {
    fn write(writer: &mut Writer, value: &Region) -> Result<(), CodecError> {
        writer.u32(value.0.index() as u32);
        <Optional<crate::fields::Path> as Field<_>>::write(writer, &value.1)
    }
    fn read(reader: &mut Reader<'_>) -> Result<Region, CodecError> {
        Ok((
            schema_reflection::SchemaNodeHandle::from_index(reader.u32()?),
            <Optional<crate::fields::Path> as Field<_>>::read(reader)?,
        ))
    }
}
struct DomainCaller;
impl Field<Caller> for DomainCaller {
    fn write(writer: &mut Writer, value: &Caller) -> Result<(), CodecError> {
        writer.boolean(value.0.0);
        for bytes in &value.0.1 {
            <crate::fields::Bytes as Field<_>>::write(writer, bytes)?;
        }
        Route::write(writer, &value.1)
    }
    fn read(reader: &mut Reader<'_>) -> Result<Caller, CodecError> {
        let required = reader.boolean()?;
        let mut bytes = [Vec::new(), Vec::new(), Vec::new()];
        for value in &mut bytes {
            *value = <crate::fields::Bytes as Field<_>>::read(reader)?;
        }
        Ok(((required, bytes), Route::read(reader)?))
    }
}
struct DomainObservation;
impl Field<Observation> for DomainObservation {
    fn write(writer: &mut Writer, value: &Observation) -> Result<(), CodecError> {
        writer.boolean(value.is_some());
        if let Some((demand, exposure)) = value {
            let (family, permissions) = demand.stored_permissions();
            writer.u8(family);
            Permissions::write(writer, &permissions)?;
            writer.u8(match exposure {
                access_vocabulary::AccessExposure::Exported => 0,
                access_vocabulary::AccessExposure::BindingPrivate => 1,
            });
        }
        Ok(())
    }
    fn read(reader: &mut Reader<'_>) -> Result<Observation, CodecError> {
        if !reader.boolean()? {
            return Ok(None);
        }
        let family = reader.u8()?;
        let demand = ObservationDemand::from_stored_permissions(family, Permissions::read(reader)?)
            .ok_or_else(|| {
                CodecError::AccessPlan(
                    "operations do not belong to the field observation family".into(),
                )
            })?;
        let exposure = match reader.u8()? {
            0 => access_vocabulary::AccessExposure::Exported,
            1 => access_vocabulary::AccessExposure::BindingPrivate,
            tag => return Err(CodecError::InvalidTag("domain observation exposure", tag)),
        };
        Ok(Some((demand, exposure)))
    }
}
struct DomainAccess;
impl Field<Policy> for DomainAccess {
    fn write(writer: &mut Writer, value: &Policy) -> Result<(), CodecError> {
        <crate::fields::Bytes as Field<_>>::write(writer, &value.schema.runtime_image())?;
        <Optional<List<DomainCaller>> as Field<_>>::write(writer, &value.issuance)?;
        for rows in [&value.readers, &value.writers] {
            writer.len("domain caller rules", rows.len())?;
            for (region, callers) in rows {
                DomainRegion::write(writer, region)?;
                <List<DomainCaller> as Field<_>>::write(writer, callers)?;
            }
        }
        writer.len("domain observations", value.observations.len())?;
        for (region, observation) in &value.observations {
            DomainRegion::write(writer, region)?;
            DomainObservation::write(writer, observation)?;
        }
        writer.len("domain reach", value.reach.services().len())?;
        for service in value.reach.services() {
            writer.u64(service.normalized_identity());
        }
        Ok(())
    }
    fn read(reader: &mut Reader<'_>) -> Result<Policy, CodecError> {
        let image = <crate::fields::Bytes as Field<_>>::read(reader)?;
        let schema = schema_reflection::SemanticSchemaGraph::from_runtime_image(&image)
            .map_err(CodecError::AccessPlan)?;
        let issuance = <Optional<List<DomainCaller>> as Field<_>>::read(reader)?;
        let mut rules = || {
            crate::wire::decode_counted(reader, |reader| {
                Ok((
                    DomainRegion::read(reader)?,
                    <List<DomainCaller> as Field<_>>::read(reader)?,
                ))
            })
        };
        let readers = rules()?;
        let writers = rules()?;
        let observations = crate::wire::decode_counted(reader, |reader| {
            Ok((
                DomainRegion::read(reader)?,
                DomainObservation::read(reader)?,
            ))
        })?;
        let services = crate::wire::decode_counted(reader, |reader| {
            access_vocabulary::BoundaryServiceReachId::from_normalized_identity(reader.u64()?)
                .map_err(|error| CodecError::AccessPlan(error.to_string()))
        })?;
        Ok(access_admission::AccessPlan {
            schema,
            issuance,
            readers,
            writers,
            observations,
            reach: access_vocabulary::BoundaryReach::from_services(services),
        })
    }
}

struct OptionalBytePredicate;
impl Field<Option<ByteSequencePredicate>> for OptionalBytePredicate {
    fn write(writer: &mut Writer, value: &Option<ByteSequencePredicate>) -> Result<(), CodecError> {
        writer.u8(value.map_or(0, |predicate| predicate.mask_bit()));
        Ok(())
    }
    fn read(reader: &mut Reader<'_>) -> Result<Option<ByteSequencePredicate>, CodecError> {
        Ok(match reader.u8()? {
            0 => None,
            1 => Some(ByteSequencePredicate::ValidUtf8),
            2 => Some(ByteSequencePredicate::NoNul),
            4 => Some(ByteSequencePredicate::AsciiOnly),
            8 => Some(ByteSequencePredicate::NonEmpty),
            tag => return Err(CodecError::InvalidTag("ByteSequencePredicate", tag)),
        })
    }
}

pub fn encode_required_memberships(
    writer: &mut Writer,
    required: &Option<Vec<DomainSemanticId>>,
) -> Result<(), CodecError> {
    Optional::<Ids>::write(writer, required)
}

pub fn decode_required_memberships(
    reader: &mut Reader<'_>,
) -> Result<Option<Vec<DomainSemanticId>>, CodecError> {
    Optional::<Ids>::read(reader)
}

pub fn encode_establishment_routes(
    writer: &mut Writer,
    routes: &Option<Vec<StructuralEstablishmentRoute>>,
) -> Result<(), CodecError> {
    Optional::<List<Route>>::write(writer, routes)
}

pub fn decode_establishment_routes(
    reader: &mut Reader<'_>,
) -> Result<Option<Vec<StructuralEstablishmentRoute>>, CodecError> {
    Optional::<List<Route>>::read(reader)
}

/// The complete domain declaration has one encoding for artifact transport
/// and downstream identity checks. Omitting a rule from an identity would let
/// a changed permission reuse an earlier checked optimization unit.
pub fn encode_structural_domain_declaration(
    declaration: &terminal_psi::StructuralDomainDeclaration,
) -> Result<Vec<u8>, CodecError> {
    let mut writer = Writer::default();
    Domain::write(&mut writer, declaration)?;
    Ok(writer.finish())
}

#[cfg(test)]
mod observation_tests {
    use super::*;
    use access_vocabulary::AccessExposure;

    #[test]
    fn stable_operations_round_trip_independently_in_psi() {
        for mask in 0u8..16 {
            let observation = Some((
                ObservationDemand::Stable {
                    read: mask & 1 != 0,
                    take: mask & 2 != 0,
                    write: mask & 4 != 0,
                    swap: mask & 8 != 0,
                },
                AccessExposure::BindingPrivate,
            ));
            let mut writer = Writer::default();
            DomainObservation::write(&mut writer, &observation).unwrap();
            let bytes = writer.finish();
            assert_eq!(
                DomainObservation::read(&mut Reader::new(&bytes)).unwrap(),
                observation
            );
        }

        // The existing Stable read encoding remains valid. Its ordinary
        // read flag cannot be interpreted as an atomic operation.
        let mut original = [0u8; 18];
        original[0] = 1;
        original[2] = 1;
        let observation = DomainObservation::read(&mut Reader::new(&original)).unwrap();
        let mut writer = Writer::default();
        DomainObservation::write(&mut writer, &observation).unwrap();
        assert_eq!(writer.finish(), original);

        original[1] = 2;
        assert!(DomainObservation::read(&mut Reader::new(&original)).is_err());
        original[1] = 1;
        original[3] = 1;
        assert!(DomainObservation::read(&mut Reader::new(&original)).is_err());
    }
}
