//! Structural domain and service declarations and installation reach
//! dependencies on the wire.

use super::super::CodecError;
use super::super::wire::{Reader, Writer};
use super::super::wire_fields::{
    ContentProjection, Field, Flag, Id, Ids, Index, List, Nominal, Optional, Proposition,
    wire_enum, wire_record,
};
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
        predicate: Optional<DomainPredicate>,
        byte_predicate: OptionalBytePredicate,
        content_projection: Optional<ContentProjection>,
        establishment_routes: List<Route>,
    }
    DomainPredicate: StructuralDomainPredicate { subject: Id, proposition: Proposition }
    Service: ServiceDeclaration { id: Id, identity: Index, parents: Ids }
    ReachDependency: InstallationReachDependency { requirement_identity: Index, upper_bound: Ids }
}

// Structural tags 1–3 and scalar tags 0–2 lacked issuer ownership and cannot
// answer a current cross-artifact route join. They stay retired rather than
// infer an owner from a callable's parameter or result types.
wire_enum! {
    Route: StructuralEstablishmentRoute, "StructuralEstablishmentRoute" {
        4 => CheckedRequirement { declaration: Nominal, requirement_identity: Index },
        5 => ExactMachine { declaration: Nominal, machine_identity: Index },
        6 => BoundaryRequirement { declaration: Nominal, requirement_identity: Index },
    }
}

/// A byte predicate's mask bit, or zero when the domain has none.
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

pub(crate) fn encode_required_memberships(
    writer: &mut Writer,
    required: &Option<Vec<DomainSemanticId>>,
) -> Result<(), CodecError> {
    Optional::<Ids>::write(writer, required)
}

pub(crate) fn decode_required_memberships(
    reader: &mut Reader<'_>,
) -> Result<Option<Vec<DomainSemanticId>>, CodecError> {
    Optional::<Ids>::read(reader)
}

pub(crate) fn encode_establishment_routes(
    writer: &mut Writer,
    routes: &[StructuralEstablishmentRoute],
) -> Result<(), CodecError> {
    writer.len("domain establishment routes", routes.len())?;
    for route in routes {
        Route::write(writer, route)?;
    }
    Ok(())
}

pub(crate) fn decode_establishment_routes(
    reader: &mut Reader<'_>,
) -> Result<Vec<StructuralEstablishmentRoute>, CodecError> {
    List::<Route>::read(reader)
}
