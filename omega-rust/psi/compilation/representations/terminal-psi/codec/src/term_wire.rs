/*
Canonical wire encoders and decoders for propositions, scalar terms, integer math terms, content
transformations, callable contracts, proof declarations, proof terms, mathematical certificates,
and scalar qualifications.
*/

pub(crate) use crate::semantic_module::*;

pub(crate) mod content_wire {

    use semantic_vocabulary::{
        ContentAlgebra, ContentAlgebraKind, ContentConservation, ContentDomainId,
        ContentPlaceSegment, ContentPlaceVersion, ContentProjectionIdentity,
        ContentStructuralPlace, ContentTerm, PropositionError, StructuralPlaceKind,
    };
    use terminal_psi::{
        ClaimContentProjection, ContentBackingAddressTransport, ContentBodyProjectionSubstitution,
        ContentBoundaryBodyRequirement, ContentClaimAddress, ContentClaimProjection,
        ContentConservationGuarantee, ContentEntryClaim, ContentIdentityReshuffle,
        ContentLineageStep, ContentPartitionComposition, ContentPlaceSubstitution,
        ContentTransformationCustody, ContentTransformationDerivation, StructuralPlaceDeclaration,
    };

    use super::CodecError;
    use super::wire::{Reader, Writer};
    use super::wire_fields::{
        ContentAlgebra as Algebra, Field, Id, Ids, List, ProjectionIdentity, Text, U64, wire_enum,
        wire_record,
    };
    use crate::sections::semantic_module::structural_place_wire::{
        decode_structural_place_kind, encode_structural_place_kind,
    };
    use crate::sections::semantic_module::wire::decode_counted;

    wire_record! {
        EntryClaim: ContentEntryClaim {
            claim: Id,
            input: ContentPlace,
            projections: List<ClaimProjection>,
        }
        PartitionComposition: ContentPartitionComposition {
            producer_operation: Id,
            source_report_fingerprint: U64,
            source_structural_places: List<ContentPlaceDeclaration>,
            source: Conservation,
            input_claims: Ids,
            substitutions: List<PlaceSubstitution>,
            derived: Conservation,
        }
        PlaceSubstitution: ContentPlaceSubstitution { source: ContentPlace, target: ContentPlace }
        Guarantee: ContentConservationGuarantee {
            report_fingerprint: U64,
            structural_places: List<ContentPlaceDeclaration>,
            conservation: Conservation,
        }
        Reshuffle: ContentIdentityReshuffle {
            claim: Id,
            input: ContentPlace,
            output: ContentPlace,
            projections: List<ClaimProjection>,
        }
        ClaimProjection: ClaimContentProjection { projection: ProjectionIdentity, algebra: Algebra }
        Derivation: ContentTransformationDerivation {
            conservation: Conservation,
            lineage_roots: Ids,
            ancestry: List<LineageStep>,
            inputs: List<ClaimSubject>,
            outputs: List<ClaimSubject>,
            custody: Custody,
        }
        LineageStep: ContentLineageStep { producer_operation: Id, inputs: Ids, outputs: Ids }
        ClaimSubject: ContentClaimProjection {
            claim: Id,
            projection: ProjectionIdentity,
            subject: ContentPlace,
        }
        BodyRequirement: ContentBoundaryBodyRequirement {
            boundary: Id,
            guarantee: Guarantee,
            substitutions: List<BodySubstitution>,
        }
        BodySubstitution: ContentBodyProjectionSubstitution { source: Term, target: Term }
        AddressTransport: ContentBackingAddressTransport { input: ClaimAddress, output: ClaimAddress }
        ClaimAddress: ContentClaimAddress { claim: Id, subject: ContentPlace }
        ContentPlace: ContentStructuralPlace {
            version: PlaceVersion,
            root: Id,
            segments: List<PlaceSegment>,
        }
    }

    wire_enum! {
        Custody: ContentTransformationCustody, "ContentTransformationCustody" {
            0 => Derived,
            1 => Boundary { requirements: List<BodyRequirement>, addresses: List<AddressTransport> },
        }
        PlaceVersion: ContentPlaceVersion, "ContentPlaceVersion" { 1 => Entry, 2 => Current }
        PlaceSegment: ContentPlaceSegment, "ContentPlaceSegment" {
            1 => Field(name: Text),
            2 => FixedIndex(index: U64),
            3 => Case(name: Text),
        }
    }

    struct ContentPlaceDeclaration;
    impl Field<StructuralPlaceDeclaration> for ContentPlaceDeclaration {
        fn write(
            writer: &mut Writer,
            value: &StructuralPlaceDeclaration,
        ) -> Result<(), CodecError> {
            if matches!(value.kind, StructuralPlaceKind::BlockParameter { .. }) {
                return Err(CodecError::InvalidTag("ContentStructuralPlaceKind", 8));
            }
            writer.id(value.id);
            encode_structural_place_kind(writer, value.kind);
            Ok(())
        }
        fn read(reader: &mut Reader<'_>) -> Result<StructuralPlaceDeclaration, CodecError> {
            let id = reader.id("PlaceId")?;
            let kind = decode_structural_place_kind(reader)?;
            if matches!(kind, StructuralPlaceKind::BlockParameter { .. }) {
                return Err(CodecError::InvalidTag("ContentStructuralPlaceKind", 8));
            }
            Ok(StructuralPlaceDeclaration { id, kind })
        }
    }

    struct Conservation;
    impl Field<ContentConservation> for Conservation {
        fn write(writer: &mut Writer, value: &ContentConservation) -> Result<(), CodecError> {
            encode_content_algebra(writer, value.algebra())?;
            encode_content_term(writer, value.left())?;
            encode_content_term(writer, value.right())
        }
        fn read(reader: &mut Reader<'_>) -> Result<ContentConservation, CodecError> {
            Ok(ContentConservation::new(
                decode_content_algebra(reader)?,
                decode_content_term(reader)?,
                decode_content_term(reader)?,
            ))
        }
    }

    struct Term;
    impl Field<ContentTerm> for Term {
        fn write(writer: &mut Writer, value: &ContentTerm) -> Result<(), CodecError> {
            encode_content_term(writer, value)
        }
        fn read(reader: &mut Reader<'_>) -> Result<ContentTerm, CodecError> {
            decode_content_term(reader)
        }
    }

    pub(crate) fn encode_content_transformations(
        writer: &mut Writer,
        derivations: &[ContentTransformationDerivation],
    ) -> Result<(), CodecError> {
        writer.counted(
            "content transformation derivations",
            derivations,
            Derivation::write,
        )
    }

    pub(crate) fn decode_content_transformations(
        reader: &mut Reader<'_>,
    ) -> Result<Vec<ContentTransformationDerivation>, CodecError> {
        List::<Derivation>::read(reader)
    }

    pub(crate) fn encode_content_algebra(
        writer: &mut Writer,
        algebra: &ContentAlgebra,
    ) -> Result<(), CodecError> {
        writer.u8(match algebra.kind {
            ContentAlgebraKind::IntervalSet => 1,
            ContentAlgebraKind::CountedQuantity => 2,
        });
        writer.string("content algebra parameter", &algebra.parameter)
    }

    /// A separation's members are projections, never separations, so this
    /// encoding nests at most once and needs no table.
    pub(crate) fn encode_content_term(
        writer: &mut Writer,
        term: &ContentTerm,
    ) -> Result<(), CodecError> {
        match term {
            ContentTerm::Projection { .. } => encode_content_projection(writer, term),
            ContentTerm::Separate(terms) => {
                writer.u8(2);
                writer.len("separated content terms", terms.len())?;
                for term in terms {
                    encode_content_projection(writer, term)?;
                }
                Ok(())
            }
        }
    }

    fn encode_content_projection(
        writer: &mut Writer,
        term: &ContentTerm,
    ) -> Result<(), CodecError> {
        let ContentTerm::Projection {
            projection,
            subject,
        } = term
        else {
            return Err(CodecError::MalformedProposition(
                PropositionError::NestedContentSeparation,
            ));
        };
        writer.u8(1);
        writer.id(projection.domain);
        writer.u64(projection.projection_report_fingerprint);
        ContentPlace::write(writer, subject)
    }

    pub(crate) fn decode_content_algebra(
        reader: &mut Reader<'_>,
    ) -> Result<ContentAlgebra, CodecError> {
        let kind = match reader.u8()? {
            1 => ContentAlgebraKind::IntervalSet,
            2 => ContentAlgebraKind::CountedQuantity,
            tag => return Err(CodecError::InvalidTag("ContentAlgebraKind", tag)),
        };
        Ok(ContentAlgebra {
            kind,
            parameter: reader.string("content algebra parameter")?,
        })
    }

    pub(crate) fn decode_content_term(reader: &mut Reader<'_>) -> Result<ContentTerm, CodecError> {
        match reader.u8()? {
            1 => decode_content_projection(reader),
            2 => {
                let terms = decode_counted(reader, |reader| match reader.u8()? {
                    1 => decode_content_projection(reader),
                    2 => Err(CodecError::MalformedProposition(
                        PropositionError::NestedContentSeparation,
                    )),
                    tag => Err(CodecError::InvalidTag("ContentTerm", tag)),
                })?;
                ContentTerm::separate(terms).map_err(CodecError::MalformedProposition)
            }
            tag => Err(CodecError::InvalidTag("ContentTerm", tag)),
        }
    }

    fn decode_content_projection(reader: &mut Reader<'_>) -> Result<ContentTerm, CodecError> {
        let projection = ContentProjectionIdentity {
            domain: reader.id::<ContentDomainId>("ContentDomainId")?,
            projection_report_fingerprint: reader.u64()?,
        };
        Ok(ContentTerm::Projection {
            projection,
            subject: ContentPlace::read(reader)?,
        })
    }
}

pub(crate) mod contract_wire {

    use semantic_vocabulary::Termination as TerminationPromise;

    use terminal_psi::{
        ContractClause, CrashCause, CrashPredicateTerm, CrashRouteBucket, CrashRouteGuard,
        MachineContract, OutcomeSpecificEnsure, OutcomeSpecificEvidence, OutcomeSpecificGuard,
        SuccessorEdge,
    };

    use super::machine_wire::{Claim, Declaration, MachineResult, ParameterKind, ReachApplication};
    use super::wire::{Reader, Writer};
    use super::wire_fields::{
        ArgumentSubstitution, Arguments, CleanupAction, CrashRoutes, Field, Flag, Id, Ids, List,
        MachineDeclaration, Optional, ProofTerms, Proposition, ScalarTerms, Text, U32,
        delegated_fields, wire_enum, wire_record,
    };
    use super::{CodecError, decode_proposition, encode_proposition};

    wire_record! {
        Successor: SuccessorEdge {
            edge: Id,
            target: Id,
            arguments: Ids,
            erased_arguments: ScalarTerms,
            erased_proof_arguments: ProofTerms,
            structural_arguments: Arguments,
            cleanup_actions: List<CleanupAction>,
        }
        Contract: MachineContract {
            id: Id,
            declaration: Optional<MachineDeclaration>,
            parameters: List<Declaration>,
            parameter_order: List<ParameterKind>,
            structural_parameters: List<super::structural_signature_wire::StructuralParameter>,
            result: MachineResult,
            entry_claims: List<Claim>,
            declared_service_reach: Optional<Ids>,
            declared_synchronous_entries: Optional<Ids>,
            declared_crash_routes: Optional<CrashRoutes>,
            declared_suspension: Optional<Flag>,
            declared_blocking: Optional<Flag>,
            published_may_suspend: Flag,
            published_may_block: Flag,
            closed_reach_application: ReachApplication,
            published_service_ceiling: Ids,
            published_synchronous_entries: Ids,
            published_writes: Optional<List<(U32, Ids)>>,
            termination: Termination,
            content_entry_claims: List<super::content_wire::EntryClaim>,
            crash_routes: CrashRoutes,
            erased_scalar_formals: List<Declaration>,
            erased_proof_formals: ProofFormals,
            requires: List<Proposition>,
            source_checked_requires: List<Proposition>,
            ensures: List<Clause>,
            outcome_specific_ensures: List<OutcomeEnsure>,
        }
        Clause: ContractClause { obligation: Id, proposition: Proposition }
        OutcomeEnsure: OutcomeSpecificEnsure {
            guard: Guard,
            position: U32,
            obligation: Id,
            proposition: Proposition,
            evidence: Optional<Evidence>,
        }
        Guard: OutcomeSpecificGuard { result_type: Id, result_case: Id }
        Evidence: OutcomeSpecificEvidence {
            term: Id,
            output_field: Text,
            argument_subjects: List<ArgumentSubstitution>,
        }
        Bucket: CrashRouteBucket { cause: Cause, alternatives: List<RouteGuard> }
    }

    wire_enum! {
        Termination: TerminationPromise, "termination guarantee" {
            0 => NoGuarantee,
            1 => Terminates,
        }
        Cause: CrashCause, "CrashCause" { 1 => Trap, 2 => Abort }
        RouteGuard: CrashRouteGuard, "CrashRouteGuard" {
            0 => Truth,
            1 => Predicate(predicate: CrashPredicate),
        }
    }

    delegated_fields! {
        ProofFormals: Vec<terminal_psi::ErasedProofFormal>
            => super::proof_term_wire::encode_proof_formals,
               super::proof_term_wire::decode_proof_formals;
    }

    pub(crate) struct CrashPredicate;
    impl Field<CrashPredicateTerm> for CrashPredicate {
        fn write(writer: &mut Writer, value: &CrashPredicateTerm) -> Result<(), CodecError> {
            encode_proposition(writer, value.proposition())
        }
        fn read(reader: &mut Reader<'_>) -> Result<CrashPredicateTerm, CodecError> {
            Ok(CrashPredicateTerm::new(decode_proposition(reader)?))
        }
    }
}

pub(crate) mod integer_math_term_wire {

    use semantic_vocabulary::{IntegerMathLiteral, IntegerMathTerm};

    use super::CodecError;
    use super::scalar_term_wire::{decode_scalar_term, encode_scalar_term};
    use super::scalar_wire::{decode_integer_type, encode_integer_type};
    use super::wire::{Reader, Writer};
    use crate::term_table::{TableChildren, TableTerm, decode_term_table, encode_term_table};

    /// Encodes one integer math term as a postorder table; an embedded scalar
    /// term is its own scalar table inside the embedding row.
    pub(crate) fn encode_integer_math_term(
        writer: &mut Writer,
        term: &IntegerMathTerm,
    ) -> Result<(), CodecError> {
        encode_term_table(writer, term)
    }

    pub(crate) fn decode_integer_math_term(
        reader: &mut Reader<'_>,
    ) -> Result<IntegerMathTerm, CodecError> {
        decode_term_table(reader)
    }

    impl TableTerm for IntegerMathTerm {
        const LABEL: &'static str = "integer math term";

        fn children<'term>(&'term self, children: &mut Vec<&'term Self>) {
            match self {
                IntegerMathTerm::Add(left, right)
                | IntegerMathTerm::Subtract(left, right)
                | IntegerMathTerm::Multiply(left, right)
                | IntegerMathTerm::ShiftLeft {
                    value: left,
                    count: right,
                } => children.extend([left.as_ref(), right.as_ref()]),
                IntegerMathTerm::Embed(_)
                | IntegerMathTerm::IntegerLiteral(_)
                | IntegerMathTerm::MathValue { .. } => {}
            }
        }

        fn write_row(&self, writer: &mut Writer) -> Result<(), CodecError> {
            match self {
                IntegerMathTerm::Embed(term) => {
                    writer.u8(7);
                    encode_scalar_term(writer, term)?;
                }
                IntegerMathTerm::IntegerLiteral(literal) => {
                    writer.u8(1);
                    writer.boolean(literal.negative());
                    writer.bytes(&literal.magnitude().to_le_bytes());
                }
                IntegerMathTerm::MathValue { source_type, value } => {
                    writer.u8(2);
                    encode_integer_type(writer, *source_type);
                    writer.id(*value);
                }
                IntegerMathTerm::Add(_, _) => writer.u8(3),
                IntegerMathTerm::Subtract(_, _) => writer.u8(4),
                IntegerMathTerm::Multiply(_, _) => writer.u8(5),
                IntegerMathTerm::ShiftLeft { .. } => writer.u8(6),
            }
            Ok(())
        }

        fn read_row(
            reader: &mut Reader<'_>,
            children: &mut TableChildren<'_, Self>,
        ) -> Result<Self, CodecError> {
            let mut pair = |reader: &mut Reader<'_>| {
                let mut operands = children.take(reader, 2)?.into_iter();
                match (operands.next(), operands.next()) {
                    (Some(left), Some(right)) => Ok((Box::new(left), Box::new(right))),
                    _ => Err(CodecError::NonCanonicalTermTable(Self::LABEL)),
                }
            };
            Ok(match reader.u8()? {
                7 => IntegerMathTerm::Embed(Box::new(decode_scalar_term(reader)?)),
                1 => IntegerMathTerm::IntegerLiteral(
                    IntegerMathLiteral::new(
                        match reader.u8()? {
                            0 => false,
                            1 => true,
                            tag => return Err(CodecError::InvalidTag("Boolean", tag)),
                        },
                        u128::from_le_bytes(reader.array()?),
                    )
                    .map_err(CodecError::MalformedProposition)?,
                ),
                2 => IntegerMathTerm::MathValue {
                    source_type: decode_integer_type(reader)?,
                    value: reader.id("ValueId")?,
                },
                3 => {
                    let (left, right) = pair(reader)?;
                    IntegerMathTerm::Add(left, right)
                }
                4 => {
                    let (left, right) = pair(reader)?;
                    IntegerMathTerm::Subtract(left, right)
                }
                5 => {
                    let (left, right) = pair(reader)?;
                    IntegerMathTerm::Multiply(left, right)
                }
                6 => {
                    let (value, count) = pair(reader)?;
                    IntegerMathTerm::ShiftLeft { value, count }
                }
                tag => return Err(CodecError::InvalidTag("IntegerMathTerm", tag)),
            })
        }
    }
}

pub(crate) mod mathematical_certificate_wire {

    use std::collections::HashMap;

    use proof_admission::{
        Declaration, Level, MathematicalCertificate, Sort, Term, TermArena, TermHandle,
    };

    use super::CodecError;
    use super::wire::{Reader, Writer, bounded_count, decode_canonical, decode_counted};

    const MAGIC: &[u8; 8] = b"PSICORE\0";
    const FORMAT_MARKER: u16 = 3;

    pub struct DecodedMathematicalCertificate {
        pub arena: TermArena,
        pub certificate: MathematicalCertificate,
    }

    pub fn encode_mathematical_certificate(
        arena: &TermArena,
        certificate: &MathematicalCertificate,
    ) -> Result<Vec<u8>, CodecError> {
        let mut term_writer = MathematicalTermWriter::new(arena);
        for declaration in &certificate.signature {
            term_writer.encode_term(declaration.ty)?;
            if let Some(body) = declaration.body {
                term_writer.encode_term(body)?;
            }
        }
        for &binding in &certificate.context {
            term_writer.encode_term(binding)?;
        }
        term_writer.encode_term(certificate.term)?;
        term_writer.encode_term(certificate.expected)?;

        let MathematicalTermWriter {
            nodes,
            by_handle,
            by_bytes: _by_bytes,
            count,
            ..
        } = term_writer;

        let mut writer = Writer::default();
        writer.bytes(MAGIC);
        writer.u16(FORMAT_MARKER);
        writer.u32(certificate.level_arity);
        writer.u32(count);
        writer.bytes(&nodes.finish());
        writer.len(
            "mathematical certificate signature",
            certificate.signature.len(),
        )?;
        for declaration in &certificate.signature {
            writer.u32(declaration.level_arity);
            writer.u32(by_handle[&declaration.ty]);
            match declaration.body {
                Some(body) => {
                    writer.u8(1);
                    writer.u32(by_handle[&body]);
                }
                None => writer.u8(0),
            }
        }
        writer.len(
            "mathematical certificate context",
            certificate.context.len(),
        )?;
        for &binding in &certificate.context {
            writer.u32(by_handle[&binding]);
        }
        writer.u32(by_handle[&certificate.term]);
        writer.u32(by_handle[&certificate.expected]);
        Ok(writer.finish())
    }

    pub fn decode_mathematical_certificate(
        bytes: &[u8],
    ) -> Result<DecodedMathematicalCertificate, CodecError> {
        decode_canonical(
            bytes,
            |reader| {
                if reader.take(MAGIC.len())? != MAGIC {
                    return Err(CodecError::InvalidMagic);
                }
                let format_marker = reader.u16()?;
                if format_marker != FORMAT_MARKER {
                    return Err(CodecError::UnsupportedFormatMarker(format_marker));
                }
                let level_arity = reader.u32()?;
                let node_count = bounded_count(reader, 1)?;
                let mut arena = TermArena::new();
                let mut handles = Vec::with_capacity(node_count);
                for _ in 0..node_count {
                    let term = decode_term(reader, &handles)?;
                    handles.push(arena.insert(term));
                }
                let signature_count = bounded_count(reader, 1)?;
                let mut signature = Vec::with_capacity(signature_count);
                for _ in 0..signature_count {
                    let level_arity = reader.u32()?;
                    let ty = decode_root(reader, &handles)?;
                    let body = if reader.boolean()? {
                        Some(decode_root(reader, &handles)?)
                    } else {
                        None
                    };
                    signature.push(Declaration {
                        level_arity,
                        ty,
                        body,
                    });
                }
                let context_count = bounded_count(reader, 1)?;
                let mut context = Vec::with_capacity(context_count);
                for _ in 0..context_count {
                    context.push(decode_root(reader, &handles)?);
                }
                let term = decode_root(reader, &handles)?;
                let expected = decode_root(reader, &handles)?;
                let certificate = MathematicalCertificate {
                    signature,
                    level_arity,
                    context,
                    term,
                    expected,
                };
                Ok(DecodedMathematicalCertificate { arena, certificate })
            },
            |decoded| encode_mathematical_certificate(&decoded.arena, &decoded.certificate),
        )
    }

    struct MathematicalTermWriter<'arena> {
        arena: &'arena TermArena,
        nodes: Writer,
        by_handle: HashMap<TermHandle, u32>,
        by_bytes: HashMap<Vec<u8>, u32>,
        count: u32,
    }

    impl<'arena> MathematicalTermWriter<'arena> {
        fn new(arena: &'arena TermArena) -> Self {
            Self {
                arena,
                nodes: Writer::default(),
                by_handle: HashMap::new(),
                by_bytes: HashMap::new(),
                count: 0,
            }
        }

        /// The term's children in the order its row names them.
        fn children(term: &Term) -> Vec<TermHandle> {
            match *term {
                Term::Pi { domain, codomain } => vec![domain, codomain],
                Term::Lambda { domain, body } => vec![domain, body],
                Term::Apply { function, argument } => vec![function, argument],
                Term::Sigma { domain, codomain } => vec![domain, codomain],
                Term::Pair { first, second } => vec![first, second],
                Term::Fst { pair } => vec![pair],
                Term::Snd { pair } => vec![pair],
                Term::CaseTwo {
                    motive,
                    zero_branch,
                    one_branch,
                    scrutinee,
                } => vec![motive, zero_branch, one_branch, scrutinee],
                Term::Id { ty, left, right } => vec![ty, left, right],
                Term::Refl { ty, value } => vec![ty, value],
                Term::IdElim {
                    motive,
                    base,
                    endpoint,
                    proof,
                } => vec![motive, base, endpoint, proof],
                Term::W { carrier, children } => vec![carrier, children],
                Term::Sup {
                    carrier,
                    children,
                    label,
                    function,
                } => vec![carrier, children, label, function],
                Term::IndW { motive, step, tree } => vec![motive, step, tree],
                Term::EmptyElim { ty, scrutinee } => vec![ty, scrutinee],
                Term::Squash { ty } => vec![ty],
                Term::SquashIntro { ty, value } => vec![ty, value],
                Term::SquashElim {
                    proposition,
                    function,
                    scrutinee,
                } => vec![proposition, function, scrutinee],
                Term::Box { ty } => vec![ty],
                Term::BoxIntro { ty, value } => vec![ty, value],
                Term::BoxElim {
                    motive,
                    body,
                    scrutinee,
                } => vec![motive, body, scrutinee],
                _ => Vec::new(),
            }
        }

        /// Appends `handle` and every term it reaches, children before parents,
        /// with an explicit stack; returns `handle`'s row index.
        fn encode_term(&mut self, handle: TermHandle) -> Result<u32, CodecError> {
            enum Step {
                Visit(TermHandle),
                Emit(TermHandle),
            }
            let mut pending = vec![Step::Visit(handle)];
            while let Some(step) = pending.pop() {
                match step {
                    Step::Visit(handle) => {
                        if self.by_handle.contains_key(&handle) {
                            continue;
                        }
                        let term = self.arena.get(handle);
                        if matches!(term, Term::Dummy) {
                            return Err(CodecError::MalformedMathematicalCertificate(
                                "dummy term cannot be certified",
                            ));
                        }
                        pending.push(Step::Emit(handle));
                        pending.extend(Self::children(&term).into_iter().rev().map(Step::Visit));
                    }
                    Step::Emit(handle) => {
                        if !self.by_handle.contains_key(&handle) {
                            self.emit(handle)?;
                        }
                    }
                }
            }
            Ok(self.by_handle[&handle])
        }

        /// Writes one row whose children already have rows.
        fn emit(&mut self, handle: TermHandle) -> Result<(), CodecError> {
            let mut node = Writer::default();
            match self.arena.get(handle) {
                Term::Dummy => {
                    return Err(CodecError::MalformedMathematicalCertificate(
                        "dummy term cannot be certified",
                    ));
                }
                Term::Variable(index) => {
                    node.u8(1);
                    node.u32(index);
                }
                Term::Sort(sort) => {
                    node.u8(2);
                    encode_sort(&mut node, sort)?;
                }
                Term::Pi { domain, codomain } => {
                    let domain = self.by_handle[&domain];
                    let codomain = self.by_handle[&codomain];
                    node.u8(3);
                    node.u32(domain);
                    node.u32(codomain);
                }
                Term::Lambda { domain, body } => {
                    let domain = self.by_handle[&domain];
                    let body = self.by_handle[&body];
                    node.u8(4);
                    node.u32(domain);
                    node.u32(body);
                }
                Term::Apply { function, argument } => {
                    let function = self.by_handle[&function];
                    let argument = self.by_handle[&argument];
                    node.u8(5);
                    node.u32(function);
                    node.u32(argument);
                }
                Term::Sigma { domain, codomain } => {
                    let domain = self.by_handle[&domain];
                    let codomain = self.by_handle[&codomain];
                    node.u8(6);
                    node.u32(domain);
                    node.u32(codomain);
                }
                Term::Pair { first, second } => {
                    let first = self.by_handle[&first];
                    let second = self.by_handle[&second];
                    node.u8(7);
                    node.u32(first);
                    node.u32(second);
                }
                Term::Fst { pair } => {
                    let pair = self.by_handle[&pair];
                    node.u8(8);
                    node.u32(pair);
                }
                Term::Snd { pair } => {
                    let pair = self.by_handle[&pair];
                    node.u8(9);
                    node.u32(pair);
                }
                Term::Two => {
                    node.u8(10);
                }
                Term::TwoZero => {
                    node.u8(11);
                }
                Term::TwoOne => {
                    node.u8(12);
                }
                Term::CaseTwo {
                    motive,
                    zero_branch,
                    one_branch,
                    scrutinee,
                } => {
                    let motive = self.by_handle[&motive];
                    let zero_branch = self.by_handle[&zero_branch];
                    let one_branch = self.by_handle[&one_branch];
                    let scrutinee = self.by_handle[&scrutinee];
                    node.u8(13);
                    node.u32(motive);
                    node.u32(zero_branch);
                    node.u32(one_branch);
                    node.u32(scrutinee);
                }
                Term::Id { ty, left, right } => {
                    let ty = self.by_handle[&ty];
                    let left = self.by_handle[&left];
                    let right = self.by_handle[&right];
                    node.u8(14);
                    node.u32(ty);
                    node.u32(left);
                    node.u32(right);
                }
                Term::Refl { ty, value } => {
                    let ty = self.by_handle[&ty];
                    let value = self.by_handle[&value];
                    node.u8(15);
                    node.u32(ty);
                    node.u32(value);
                }
                Term::IdElim {
                    motive,
                    base,
                    endpoint,
                    proof,
                } => {
                    let motive = self.by_handle[&motive];
                    let base = self.by_handle[&base];
                    let endpoint = self.by_handle[&endpoint];
                    let proof = self.by_handle[&proof];
                    node.u8(16);
                    node.u32(motive);
                    node.u32(base);
                    node.u32(endpoint);
                    node.u32(proof);
                }
                Term::W { carrier, children } => {
                    let carrier = self.by_handle[&carrier];
                    let children = self.by_handle[&children];
                    node.u8(17);
                    node.u32(carrier);
                    node.u32(children);
                }
                Term::Sup {
                    carrier,
                    children,
                    label,
                    function,
                } => {
                    let carrier = self.by_handle[&carrier];
                    let children = self.by_handle[&children];
                    let label = self.by_handle[&label];
                    let function = self.by_handle[&function];
                    node.u8(18);
                    node.u32(carrier);
                    node.u32(children);
                    node.u32(label);
                    node.u32(function);
                }
                Term::IndW { motive, step, tree } => {
                    let motive = self.by_handle[&motive];
                    let step = self.by_handle[&step];
                    let tree = self.by_handle[&tree];
                    node.u8(19);
                    node.u32(motive);
                    node.u32(step);
                    node.u32(tree);
                }
                Term::Constant {
                    declaration,
                    levels,
                } => {
                    node.u8(20);
                    node.u32(declaration);
                    node.len("constant level arguments", levels.len())?;
                    for level in levels {
                        encode_level(&mut node, level)?;
                    }
                }
                Term::Empty => {
                    node.u8(21);
                }
                Term::EmptyElim { ty, scrutinee } => {
                    let ty = self.by_handle[&ty];
                    let scrutinee = self.by_handle[&scrutinee];
                    node.u8(22);
                    node.u32(ty);
                    node.u32(scrutinee);
                }
                Term::Squash { ty } => {
                    let ty = self.by_handle[&ty];
                    node.u8(23);
                    node.u32(ty);
                }
                Term::SquashIntro { ty, value } => {
                    let ty = self.by_handle[&ty];
                    let value = self.by_handle[&value];
                    node.u8(24);
                    node.u32(ty);
                    node.u32(value);
                }
                Term::SquashElim {
                    proposition,
                    function,
                    scrutinee,
                } => {
                    let proposition = self.by_handle[&proposition];
                    let function = self.by_handle[&function];
                    let scrutinee = self.by_handle[&scrutinee];
                    node.u8(25);
                    node.u32(proposition);
                    node.u32(function);
                    node.u32(scrutinee);
                }
                Term::Box { ty } => {
                    let ty = self.by_handle[&ty];
                    node.u8(26);
                    node.u32(ty);
                }
                Term::BoxIntro { ty, value } => {
                    let ty = self.by_handle[&ty];
                    let value = self.by_handle[&value];
                    node.u8(27);
                    node.u32(ty);
                    node.u32(value);
                }
                Term::BoxElim {
                    motive,
                    body,
                    scrutinee,
                } => {
                    let motive = self.by_handle[&motive];
                    let body = self.by_handle[&body];
                    let scrutinee = self.by_handle[&scrutinee];
                    node.u8(28);
                    node.u32(motive);
                    node.u32(body);
                    node.u32(scrutinee);
                }
            }
            let bytes = node.finish();
            if let Some(&index) = self.by_bytes.get(&bytes) {
                self.by_handle.insert(handle, index);
                return Ok(());
            }
            let index = self.count;
            self.nodes.bytes(&bytes);
            self.by_bytes.insert(bytes, index);
            self.by_handle.insert(handle, index);
            self.count += 1;
            Ok(())
        }
    }

    fn encode_sort(writer: &mut Writer, sort: Sort) -> Result<(), CodecError> {
        let tag = match sort {
            Sort::Type(_) => 1,
            Sort::Strict(_) => 2,
        };
        writer.u8(tag);
        encode_level(writer, sort.level())
    }

    /// Levels stay inline in prefix order: constructor tag, then operands.
    fn encode_level(writer: &mut Writer, level: Level) -> Result<(), CodecError> {
        let mut pending = vec![&level];
        while let Some(level) = pending.pop() {
            match level {
                Level::Constant(value) => {
                    writer.u8(1);
                    writer.u32(*value);
                }
                Level::Parameter(index) => {
                    writer.u8(2);
                    writer.u32(*index);
                }
                Level::Successor(inner) => {
                    writer.u8(3);
                    pending.push(inner);
                }
                Level::Maximum(left, right) => {
                    writer.u8(4);
                    pending.push(right);
                    pending.push(left);
                }
            }
        }
        Ok(())
    }

    fn decode_term(reader: &mut Reader<'_>, handles: &[TermHandle]) -> Result<Term, CodecError> {
        let child = |reader: &mut Reader<'_>| -> Result<TermHandle, CodecError> {
            let index = usize::try_from(reader.u32()?).map_err(|_| {
                CodecError::MalformedMathematicalCertificate("term child index too large")
            })?;
            if index >= handles.len() {
                return Err(CodecError::MalformedMathematicalCertificate(
                    "term child does not precede its parent",
                ));
            }
            Ok(handles[index])
        };
        Ok(match reader.u8()? {
            1 => Term::Variable(reader.u32()?),
            2 => Term::Sort(decode_sort(reader)?),
            3 => {
                let domain = child(reader)?;
                let codomain = child(reader)?;
                Term::Pi { domain, codomain }
            }
            4 => {
                let domain = child(reader)?;
                let body = child(reader)?;
                Term::Lambda { domain, body }
            }
            5 => {
                let function = child(reader)?;
                let argument = child(reader)?;
                Term::Apply { function, argument }
            }
            6 => {
                let domain = child(reader)?;
                let codomain = child(reader)?;
                Term::Sigma { domain, codomain }
            }
            7 => {
                let first = child(reader)?;
                let second = child(reader)?;
                Term::Pair { first, second }
            }
            8 => {
                let pair = child(reader)?;
                Term::Fst { pair }
            }
            9 => {
                let pair = child(reader)?;
                Term::Snd { pair }
            }
            10 => Term::Two,
            11 => Term::TwoZero,
            12 => Term::TwoOne,
            13 => {
                let motive = child(reader)?;
                let zero_branch = child(reader)?;
                let one_branch = child(reader)?;
                let scrutinee = child(reader)?;
                Term::CaseTwo {
                    motive,
                    zero_branch,
                    one_branch,
                    scrutinee,
                }
            }
            14 => {
                let ty = child(reader)?;
                let left = child(reader)?;
                let right = child(reader)?;
                Term::Id { ty, left, right }
            }
            15 => {
                let ty = child(reader)?;
                let value = child(reader)?;
                Term::Refl { ty, value }
            }
            16 => {
                let motive = child(reader)?;
                let base = child(reader)?;
                let endpoint = child(reader)?;
                let proof = child(reader)?;
                Term::IdElim {
                    motive,
                    base,
                    endpoint,
                    proof,
                }
            }
            17 => {
                let carrier = child(reader)?;
                let children = child(reader)?;
                Term::W { carrier, children }
            }
            18 => {
                let carrier = child(reader)?;
                let children = child(reader)?;
                let label = child(reader)?;
                let function = child(reader)?;
                Term::Sup {
                    carrier,
                    children,
                    label,
                    function,
                }
            }
            19 => {
                let motive = child(reader)?;
                let step = child(reader)?;
                let tree = child(reader)?;
                Term::IndW { motive, step, tree }
            }
            20 => {
                let declaration = reader.u32()?;
                let levels = decode_counted(reader, decode_level)?;
                Term::Constant {
                    declaration,
                    levels,
                }
            }
            21 => Term::Empty,
            22 => {
                let ty = child(reader)?;
                let scrutinee = child(reader)?;
                Term::EmptyElim { ty, scrutinee }
            }
            23 => {
                let ty = child(reader)?;
                Term::Squash { ty }
            }
            24 => {
                let ty = child(reader)?;
                let value = child(reader)?;
                Term::SquashIntro { ty, value }
            }
            25 => {
                let proposition = child(reader)?;
                let function = child(reader)?;
                let scrutinee = child(reader)?;
                Term::SquashElim {
                    proposition,
                    function,
                    scrutinee,
                }
            }
            26 => {
                let ty = child(reader)?;
                Term::Box { ty }
            }
            27 => {
                let ty = child(reader)?;
                let value = child(reader)?;
                Term::BoxIntro { ty, value }
            }
            28 => {
                let motive = child(reader)?;
                let body = child(reader)?;
                let scrutinee = child(reader)?;
                Term::BoxElim {
                    motive,
                    body,
                    scrutinee,
                }
            }
            tag => return Err(CodecError::InvalidTag("MathematicalTerm", tag)),
        })
    }

    fn decode_sort(reader: &mut Reader<'_>) -> Result<Sort, CodecError> {
        Ok(match reader.u8()? {
            1 => Sort::Type(decode_level(reader)?),
            2 => Sort::Strict(decode_level(reader)?),
            tag => return Err(CodecError::InvalidTag("MathematicalSort", tag)),
        })
    }

    /// Reads a prefix-order level with an explicit stack of unfinished
    /// constructors, so nesting is bounded only by the bytes present.
    fn decode_level(reader: &mut Reader<'_>) -> Result<Level, CodecError> {
        enum Frame {
            Successor,
            MaximumLeft,
            MaximumRight(Level),
        }
        let mut frames = Vec::new();
        loop {
            let mut value = match reader.u8()? {
                1 => Level::Constant(reader.u32()?),
                2 => Level::Parameter(reader.u32()?),
                3 => {
                    frames.push(Frame::Successor);
                    continue;
                }
                4 => {
                    frames.push(Frame::MaximumLeft);
                    continue;
                }
                tag => return Err(CodecError::InvalidTag("MathematicalLevel", tag)),
            };
            loop {
                match frames.pop() {
                    None => return Ok(value),
                    Some(Frame::Successor) => value = Level::Successor(Box::new(value)),
                    Some(Frame::MaximumLeft) => {
                        frames.push(Frame::MaximumRight(value));
                        break;
                    }
                    Some(Frame::MaximumRight(left)) => {
                        value = Level::Maximum(Box::new(left), Box::new(value));
                    }
                }
            }
        }
    }

    fn decode_root(
        reader: &mut Reader<'_>,
        handles: &[TermHandle],
    ) -> Result<TermHandle, CodecError> {
        let index = usize::try_from(reader.u32()?)
            .map_err(|_| CodecError::MalformedMathematicalCertificate("root index too large"))?;
        handles.get(index).copied().ok_or({
            CodecError::MalformedMathematicalCertificate("root index outside the term table")
        })
    }

    #[cfg(test)]
    mod level_worklist_tests {
        use super::*;

        #[test]
        fn inline_level_bytes_preserve_constructor_and_argument_order() {
            let level = Level::Maximum(
                Box::new(Level::Constant(17)),
                Box::new(Level::Successor(Box::new(Level::Parameter(3)))),
            );
            let mut writer = Writer::default();
            encode_level(&mut writer, level).unwrap();
            assert_eq!(writer.finish(), vec![4, 1, 17, 0, 0, 0, 3, 2, 3, 0, 0, 0]);
        }

        #[test]
        fn deep_levels_round_trip_on_the_default_stack() {
            std::thread::spawn(|| {
                let mut deep = Level::Parameter(0);
                for _ in 0..131_072 {
                    deep = Level::Successor(Box::new(deep));
                }
                deep = Level::Maximum(Box::new(deep), Box::new(Level::Constant(1)));
                let mut writer = Writer::default();
                encode_level(&mut writer, deep.clone()).unwrap();
                let bytes = writer.finish();
                let mut reader = Reader::new(&bytes);
                assert_eq!(decode_level(&mut reader).unwrap(), deep);
                assert_eq!(reader.remaining(), 0);
                let truncated = vec![3; 131_072];
                assert!(matches!(
                    decode_level(&mut Reader::new(&truncated)),
                    Err(CodecError::UnexpectedEnd)
                ));
            })
            .join()
            .expect("level codec and cleanup use the default stack");
        }
    }
}

pub(crate) mod proof_declaration_wire {

    use terminal_psi::{
        EvidenceInterfaceIdentity, EvidenceProjectionIdentity, EvidenceRequirementIdentity,
        PropositionApplicationIdentity, PropositionBinderArgumentIdentity,
        PropositionBinderArgumentKind, PropositionBinderDeclaration, PropositionBinderKind,
        PropositionDeclaration, PropositionEvidence, TerminalIdentityIndex,
    };

    use super::CodecError;
    use super::wire::{Reader, Writer};
    use super::wire_fields::{Field, Id, Index, List, Optional, Text, wire_enum, wire_record};

    wire_record! {
        Declaration: PropositionDeclaration {
            id: Id,
            name: Text,
            binders: List<Binder>,
            parameter_types: List<Text>,
            evidence: Evidence,
        }
        Binder: PropositionBinderDeclaration { name: Text, kind: BinderKind }
        Application: PropositionApplicationIdentity {
            id: Id,
            declaration: Id,
            binder_arguments: List<BinderArgument>,
            arguments: List<Text>,
            evidence_interface: Optional<Interface>,
        }
        Projection: EvidenceProjectionIdentity {
            term: Id,
            declaring_trait_identity: Index,
            declaring_trait_arguments: List<Text>,
            requirement_identity: Index,
        }
        Interface: EvidenceInterfaceIdentity {
            trait_identity: Index,
            arguments: List<Text>,
            requirements: List<Requirement>,
        }
        Requirement: EvidenceRequirementIdentity {
            declaring_trait_identity: Index,
            declaring_trait_arguments: List<Text>,
            requirement_identity: Index,
        }
    }

    wire_enum! {
        BinderKind: PropositionBinderKind, "PropositionBinderKind" {
            1 => Type,
            2 => Const { type_identity: Index },
            3 => Machine,
        }
        Evidence: PropositionEvidence, "PropositionEvidence" {
            1 => FactOnly,
            2 => Witness { evidence_type: Text },
        }
        ArgumentKind: PropositionBinderArgumentKind, "PropositionBinderArgumentKind" {
            1 => Type,
            2 => Const,
            3 => Machine,
        }
    }

    struct BinderArgument;
    impl Field<PropositionBinderArgumentIdentity> for BinderArgument {
        fn write(
            writer: &mut Writer,
            value: &PropositionBinderArgumentIdentity,
        ) -> Result<(), CodecError> {
            ArgumentKind::write(writer, &value.kind)?;
            match &value.evidence_projection {
                None => {
                    writer.u8(0);
                    writer.u32(value.identity.0);
                    Ok(())
                }
                Some(projection) => {
                    writer.u8(1);
                    Projection::write(writer, projection)
                }
            }
        }
        fn read(reader: &mut Reader<'_>) -> Result<PropositionBinderArgumentIdentity, CodecError> {
            let kind = ArgumentKind::read(reader)?;
            let (identity, evidence_projection) = match reader.u8()? {
                0 => (TerminalIdentityIndex(reader.u32()?), None),
                1 => (
                    TerminalIdentityIndex::ABSENT,
                    Some(Projection::read(reader)?),
                ),
                tag => return Err(CodecError::InvalidTag("PropositionBinderArgument", tag)),
            };
            Ok(PropositionBinderArgumentIdentity {
                kind,
                identity,
                evidence_projection,
            })
        }
    }
}

pub(crate) mod proof_term_wire {

    use semantic_vocabulary::{MathematicalArgument, ProofFormalScope, ProofTerm, ProofTermField};
    use terminal_psi::ErasedProofFormal;
    use terminal_psi::TerminalIdentityIndex;

    use super::CodecError;
    use super::scalar_term_wire::{decode_scalar_term, encode_scalar_term};
    use super::wire::{Reader, Writer, bounded_count, decode_bytes, decode_counted};
    use crate::term_table::{TableChildren, TableTerm, decode_term_table, encode_term_table};

    const PROOF_TERM_CONSTRUCTION: u8 = 1;
    const PROOF_TERM_FORMAL: u8 = 2;
    const PROOF_TERM_SCALAR: u8 = 3;
    const PROOF_TERM_ARRAY: u8 = 4;
    const PROOF_TERM_MATHEMATICAL: u8 = 5;
    const PROOF_TERM_MATHEMATICAL_APPLICATION: u8 = 6;
    const PROOF_TERM_INVOCATION_FORMAL: u8 = 7;
    const PROOF_TERM_MIXED_MATHEMATICAL_APPLICATION: u8 = 8;
    const PROOF_TERM_PROJECTION: u8 = 9;

    pub(crate) fn encode_proof_terms(
        writer: &mut Writer,
        terms: &[ProofTerm],
    ) -> Result<(), CodecError> {
        writer.len("proof terms", terms.len())?;
        for term in terms {
            encode_term_table(writer, term)?;
        }
        Ok(())
    }

    impl TableTerm for ProofTerm {
        const LABEL: &'static str = "proof term";

        fn children<'term>(&'term self, children: &mut Vec<&'term Self>) {
            match self {
                ProofTerm::Construction { fields, .. } => {
                    children.extend(fields.iter().map(|field| &field.term))
                }
                ProofTerm::Array { elements, .. } => children.extend(elements),
                _ => {}
            }
        }

        fn write_row(&self, writer: &mut Writer) -> Result<(), CodecError> {
            write_proof_row(writer, self)
        }

        fn read_row(
            reader: &mut Reader<'_>,
            children: &mut TableChildren<'_, Self>,
        ) -> Result<Self, CodecError> {
            let term = read_proof_row(reader, children)?;
            term.validate_node()
                .map_err(CodecError::MalformedProposition)?;
            Ok(term)
        }
    }

    /// One row; a construction's field names and an array's length precede
    /// the member handles.
    fn write_proof_row(writer: &mut Writer, term: &ProofTerm) -> Result<(), CodecError> {
        match term {
            ProofTerm::Mathematical {
                type_identity,
                certificate,
            } => {
                writer.u8(PROOF_TERM_MATHEMATICAL);
                writer.string("proof term type identity", type_identity)?;
                writer.len("mathematical proof certificate", certificate.len())?;
                writer.bytes(certificate);
            }
            ProofTerm::MathematicalApplication {
                type_identity,
                certificate,
                arguments,
                formal_scope,
            } => {
                let mixed = arguments
                    .iter()
                    .any(|argument| matches!(argument, MathematicalArgument::Scalar { .. }));
                writer.u8(if mixed {
                    PROOF_TERM_MIXED_MATHEMATICAL_APPLICATION
                } else {
                    PROOF_TERM_MATHEMATICAL_APPLICATION
                });
                writer.string("proof term type identity", type_identity)?;
                writer.len("mathematical function certificate", certificate.len())?;
                writer.bytes(certificate);
                writer.u8(match formal_scope {
                    ProofFormalScope::Block => 0,
                    ProofFormalScope::Invocation => 1,
                });
                writer.len("mathematical function arguments", arguments.len())?;
                for argument in arguments {
                    match argument {
                        MathematicalArgument::Formal { position } => {
                            if mixed {
                                writer.u8(0);
                            }
                            writer.u32(*position);
                        }
                        MathematicalArgument::Scalar {
                            type_identity,
                            term,
                        } => {
                            writer.u8(1);
                            writer.string("mathematical scalar source carrier", type_identity)?;
                            encode_scalar_term(writer, term)?;
                        }
                    }
                }
            }
            ProofTerm::Construction {
                type_identity,
                case_identity,
                fields,
            } => {
                writer.u8(PROOF_TERM_CONSTRUCTION);
                writer.string("proof term type identity", type_identity)?;
                writer.boolean(case_identity.is_some());
                if let Some(case_identity) = case_identity {
                    writer.string("proof term case identity", case_identity)?;
                }
                writer.len("proof term fields", fields.len())?;
                for field in fields {
                    writer.string("proof term field identity", &field.field_identity)?;
                }
            }
            ProofTerm::Array {
                type_identity,
                elements,
            } => {
                writer.u8(PROOF_TERM_ARRAY);
                writer.string("proof term type identity", type_identity)?;
                writer.len("proof array elements", elements.len())?;
            }
            ProofTerm::Formal { position } => {
                writer.u8(PROOF_TERM_FORMAL);
                writer.u32(*position);
            }
            ProofTerm::InvocationFormal { position } => {
                writer.u8(PROOF_TERM_INVOCATION_FORMAL);
                writer.u32(*position);
            }
            ProofTerm::Scalar(scalar) => {
                writer.u8(PROOF_TERM_SCALAR);
                encode_scalar_term(writer, scalar)?;
            }
            ProofTerm::Projection {
                type_identity,
                formal_position,
                fields,
            } => {
                writer.u8(PROOF_TERM_PROJECTION);
                writer.string("proof term type identity", type_identity)?;
                writer.u32(*formal_position);
                writer.len("proof projection fields", fields.len())?;
                for field in fields {
                    writer.string("proof projection field", field)?;
                }
            }
        }
        Ok(())
    }

    pub(crate) fn decode_proof_terms(
        reader: &mut Reader<'_>,
    ) -> Result<Vec<ProofTerm>, CodecError> {
        decode_counted(reader, decode_term_table)
    }

    fn read_proof_row(
        reader: &mut Reader<'_>,
        children: &mut TableChildren<'_, ProofTerm>,
    ) -> Result<ProofTerm, CodecError> {
        match reader.u8()? {
            PROOF_TERM_MATHEMATICAL => {
                let type_identity = reader.string("proof term type identity")?;
                let certificate = decode_bytes(reader)?;
                super::mathematical_certificate_wire::decode_mathematical_certificate(
                    &certificate,
                )?;
                let term = ProofTerm::Mathematical {
                    type_identity,
                    certificate,
                };
                Ok(term)
            }
            tag @ (PROOF_TERM_MATHEMATICAL_APPLICATION
            | PROOF_TERM_MIXED_MATHEMATICAL_APPLICATION) => {
                let type_identity = reader.string("proof term type identity")?;
                let certificate = decode_bytes(reader)?;
                super::mathematical_certificate_wire::decode_mathematical_certificate(
                    &certificate,
                )?;
                let formal_scope = match reader.u8()? {
                    0 => ProofFormalScope::Block,
                    1 => ProofFormalScope::Invocation,
                    tag => return Err(CodecError::InvalidTag("proof formal scope", tag)),
                };
                let minimum_argument_bytes = if tag == PROOF_TERM_MIXED_MATHEMATICAL_APPLICATION {
                    5
                } else {
                    4
                };
                let count = bounded_count(reader, minimum_argument_bytes)?;
                let mut arguments = Vec::with_capacity(count);
                for _ in 0..count {
                    let kind = if tag == PROOF_TERM_MIXED_MATHEMATICAL_APPLICATION {
                        reader.u8()?
                    } else {
                        0
                    };
                    arguments.push(match kind {
                        0 => MathematicalArgument::Formal {
                            position: reader.u32()?,
                        },
                        1 => MathematicalArgument::Scalar {
                            type_identity: reader.string("mathematical scalar source carrier")?,
                            term: decode_scalar_term(reader)?,
                        },
                        tag => {
                            return Err(CodecError::InvalidTag("mathematical argument kind", tag));
                        }
                    });
                }
                let term = ProofTerm::MathematicalApplication {
                    type_identity,
                    certificate,
                    arguments,
                    formal_scope,
                };
                Ok(term)
            }
            PROOF_TERM_CONSTRUCTION => {
                let type_identity = reader.string("proof term type identity")?;
                let case_identity = if reader.boolean()? {
                    Some(reader.string("proof term case identity")?)
                } else {
                    None
                };
                let identities =
                    decode_counted(reader, |reader| reader.string("proof term field identity"))?;
                let terms = children.take(reader, identities.len())?;
                let fields = identities
                    .into_iter()
                    .zip(terms)
                    .map(|(field_identity, term)| ProofTermField {
                        field_identity,
                        term,
                    })
                    .collect();
                let term = ProofTerm::Construction {
                    type_identity,
                    case_identity,
                    fields,
                };
                Ok(term)
            }
            PROOF_TERM_ARRAY => {
                let type_identity = reader.string("proof term type identity")?;
                let count = bounded_count(reader, 4)?;
                let elements = children.take(reader, count)?;
                let term = ProofTerm::Array {
                    type_identity,
                    elements,
                };
                Ok(term)
            }
            PROOF_TERM_FORMAL => Ok(ProofTerm::Formal {
                position: reader.u32()?,
            }),
            PROOF_TERM_INVOCATION_FORMAL => Ok(ProofTerm::InvocationFormal {
                position: reader.u32()?,
            }),
            PROOF_TERM_SCALAR => {
                let term = ProofTerm::Scalar(decode_scalar_term(reader)?);
                Ok(term)
            }
            PROOF_TERM_PROJECTION => {
                let type_identity = reader.string("proof term type identity")?;
                let formal_position = reader.u32()?;
                let fields =
                    decode_counted(reader, |reader| reader.string("proof projection field"))?;
                let term = ProofTerm::Projection {
                    type_identity,
                    formal_position,
                    fields,
                };
                Ok(term)
            }
            tag => Err(CodecError::InvalidTag("proof term", tag)),
        }
    }

    pub(crate) fn encode_proof_formals(
        writer: &mut Writer,
        formals: &[ErasedProofFormal],
    ) -> Result<(), CodecError> {
        writer.len("erased proof formals", formals.len())?;
        for formal in formals {
            writer.u32(formal.source_position);
            writer.u32(formal.type_identity.0);
        }
        Ok(())
    }

    pub(crate) fn decode_proof_formals(
        reader: &mut Reader<'_>,
    ) -> Result<Vec<ErasedProofFormal>, CodecError> {
        decode_counted(reader, |reader| {
            let source_position = reader.u32()?;
            let type_identity = TerminalIdentityIndex(reader.u32()?);
            if type_identity.is_absent() {
                return Err(CodecError::MalformedStructuralFoundation(
                    "empty erased proof formal type",
                ));
            }
            Ok(ErasedProofFormal {
                source_position,
                type_identity,
            })
        })
    }

    #[cfg(test)]
    mod mathematical_tests {
        use super::super::mathematical_certificate_wire::encode_mathematical_certificate;

        use super::*;
        use proof_admission::{Declaration, MathematicalCertificate, Term, TermArena};

        #[test]
        fn mixed_application_wire_tags_and_argument_roster_are_explicit() {
            let mut arena = TermArena::new();
            let two = arena.insert(Term::Two);
            let zero = arena.insert(Term::TwoZero);
            let certificate = encode_mathematical_certificate(
                &arena,
                &MathematicalCertificate {
                    signature: vec![],
                    level_arity: 0,
                    context: vec![],
                    term: zero,
                    expected: two,
                },
            )
            .unwrap();
            let term = ProofTerm::MathematicalApplication {
                type_identity: "Example".into(),
                certificate,
                arguments: vec![
                    MathematicalArgument::Scalar {
                        type_identity: "Primitive".into(),
                        term: semantic_vocabulary::ScalarTerm::Value {
                            id: semantic_vocabulary::ValueId::new(1).unwrap(),
                            scalar_type: semantic_vocabulary::ScalarType::Boolean,
                        },
                    },
                    MathematicalArgument::Formal { position: 0 },
                ],
                formal_scope: ProofFormalScope::Invocation,
            };
            let mut writer = Writer::default();
            encode_proof_terms(&mut writer, std::slice::from_ref(&term)).unwrap();
            let encoded = writer.finish();
            assert_eq!(encoded[8], PROOF_TERM_MIXED_MATHEMATICAL_APPLICATION);
            assert_eq!(
                decode_proof_terms(&mut Reader::new(&encoded)).unwrap(),
                vec![term]
            );
            let mut prefix = Reader::new(&encoded);
            assert_eq!(prefix.count().unwrap(), 1);
            assert_eq!(prefix.count().unwrap(), 1, "a one-row proof-term table");
            prefix.u8().unwrap();
            prefix.string("identity").unwrap();
            let bytes = prefix.count().unwrap() as usize;
            prefix.take(bytes).unwrap();
            prefix.u8().unwrap();
            prefix.count().unwrap();
            let argument_offset = encoded.len() - prefix.remaining();
            let mut changed = encoded.clone();
            changed[argument_offset] = 255;
            assert!(matches!(
                decode_proof_terms(&mut Reader::new(&changed)),
                Err(CodecError::InvalidTag("mathematical argument kind", 255))
            ));
            let mut missing = encoded.clone();
            missing.pop();
            assert!(decode_proof_terms(&mut Reader::new(&missing)).is_err());
            let mut erased_only = decode_proof_terms(&mut Reader::new(&encoded)).unwrap();
            let ProofTerm::MathematicalApplication { arguments, .. } = &mut erased_only[0] else {
                unreachable!()
            };
            arguments.retain(|argument| matches!(argument, MathematicalArgument::Formal { .. }));
            let mut writer = Writer::default();
            encode_proof_terms(&mut writer, &erased_only).unwrap();
            let erased_bytes = writer.finish();
            assert_eq!(erased_bytes[8], PROOF_TERM_MATHEMATICAL_APPLICATION);
            let mut alternative = erased_bytes.clone();
            alternative[8] = PROOF_TERM_MIXED_MATHEMATICAL_APPLICATION;
            alternative.insert(argument_offset, 0);
            let decoded = decode_proof_terms(&mut Reader::new(&alternative)).unwrap();
            let mut writer = Writer::default();
            encode_proof_terms(&mut writer, &decoded).unwrap();
            let canonical = writer.finish();
            assert_eq!(canonical, erased_bytes);
            assert_ne!(canonical, alternative);
            let mut count_overflow = encoded.clone();
            count_overflow[argument_offset - 4..argument_offset]
                .copy_from_slice(&u32::MAX.to_le_bytes());
            assert!(matches!(
                decode_proof_terms(&mut Reader::new(&count_overflow)),
                Err(CodecError::UnexpectedEnd)
            ));
        }

        #[test]
        fn mathematical_actual_round_trips_complete_signature_and_rejects_wire_changes() {
            let mut arena = TermArena::new();
            let two = arena.insert(Term::Two);
            let zero = arena.insert(Term::TwoZero);
            let declaration = Declaration::assumption(0, two);
            let subject = arena.insert(Term::Constant {
                declaration: 0,
                levels: vec![],
            });
            let certificate = MathematicalCertificate {
                signature: vec![declaration],
                level_arity: 0,
                context: vec![],
                term: subject,
                expected: two,
            };
            let bytes = encode_mathematical_certificate(&arena, &certificate).unwrap();
            let term = ProofTerm::Mathematical {
                type_identity: "Example<0>".into(),
                certificate: bytes,
            };
            let mut writer = Writer::default();
            encode_proof_terms(&mut writer, std::slice::from_ref(&term)).unwrap();
            let encoded = writer.finish();
            let mut reader = Reader::new(&encoded);
            assert_eq!(decode_proof_terms(&mut reader).unwrap(), vec![term]);
            assert_eq!(reader.remaining(), 0);

            let mut changed = encoded.clone();
            changed.pop();
            assert!(decode_proof_terms(&mut Reader::new(&changed)).is_err());
            let empty = ProofTerm::Mathematical {
                type_identity: String::new(),
                certificate: encode_mathematical_certificate(
                    &arena,
                    &MathematicalCertificate {
                        term: zero,
                        ..certificate
                    },
                )
                .unwrap(),
            };
            let mut writer = Writer::default();
            encode_proof_terms(&mut writer, &[empty]).unwrap();
            assert!(decode_proof_terms(&mut Reader::new(&writer.finish())).is_err());
        }
    }
}

pub(crate) mod proposition_wire {

    use semantic_vocabulary::{
        ContentConservation, Proposition, PropositionId, StructuralCaseSubject,
        StructuralIdentitySubject,
    };

    use super::content_wire::{
        decode_content_algebra, decode_content_term, encode_content_algebra, encode_content_term,
    };
    use super::integer_math_term_wire::{decode_integer_math_term, encode_integer_math_term};
    use super::structural_field_wire::{
        decode_byte_sequence_field, decode_canonical_structural_field,
        decode_ieee_float_comparison_kind, decode_ieee_float_field, decode_ieee_float_format,
        encode_byte_sequence_field, encode_canonical_structural_field,
        encode_ieee_float_comparison_kind, encode_ieee_float_field, encode_ieee_float_format,
    };
    use super::wire::{Reader, Writer, bounded_count};
    use super::{CodecError, decode_scalar_term, encode_scalar_term};
    use crate::term_table::{TableChildren, TableTerm, decode_term_table, encode_term_table};

    /// Encodes one proposition as a postorder table (`crate::term_table`);
    /// its scalar, integer math and content operands are their own encodings
    /// inside the row.
    pub(crate) fn encode_proposition(
        writer: &mut Writer,
        proposition: &Proposition,
    ) -> Result<(), CodecError> {
        encode_term_table(writer, proposition)
    }

    pub(crate) fn decode_proposition(reader: &mut Reader<'_>) -> Result<Proposition, CodecError> {
        decode_term_table(reader)
    }

    impl TableTerm for Proposition {
        const LABEL: &'static str = "proposition";

        fn children<'term>(&'term self, children: &mut Vec<&'term Self>) {
            match self {
                Proposition::Conjunction(members) | Proposition::Disjunction(members) => {
                    children.extend(members)
                }
                Proposition::Implication {
                    premise,
                    conclusion,
                } => children.extend([premise.as_ref(), conclusion.as_ref()]),
                _ => {}
            }
        }

        fn write_row(&self, writer: &mut Writer) -> Result<(), CodecError> {
            write_proposition_row(writer, self)
        }

        fn read_row(
            reader: &mut Reader<'_>,
            children: &mut TableChildren<'_, Self>,
        ) -> Result<Self, CodecError> {
            read_proposition_row(reader, children)
        }
    }

    fn write_proposition_row(
        writer: &mut Writer,
        proposition: &Proposition,
    ) -> Result<(), CodecError> {
        match proposition {
            Proposition::Truth => writer.u8(1),
            Proposition::Falsehood => writer.u8(2),
            Proposition::Atom(id) => {
                writer.u8(3);
                writer.id(*id);
            }
            Proposition::Equal(left, right) => {
                writer.u8(4);
                encode_scalar_term(writer, left)?;
                encode_scalar_term(writer, right)?;
            }
            Proposition::LessThan(left, right) => {
                writer.u8(5);
                encode_scalar_term(writer, left)?;
                encode_scalar_term(writer, right)?;
            }
            Proposition::LessOrEqual(left, right) => {
                writer.u8(6);
                encode_scalar_term(writer, left)?;
                encode_scalar_term(writer, right)?;
            }
            semantic_vocabulary::integer_math_comparison_operands!(left, right) => {
                writer.u8(match proposition {
                    Proposition::IntegerMathEqual(_, _) => 14,
                    Proposition::IntegerMathLessThan(_, _) => 15,
                    Proposition::IntegerMathLessOrEqual(_, _) => 16,
                    _ => unreachable!(),
                });
                encode_integer_math_term(writer, left)?;
                encode_integer_math_term(writer, right)?;
            }
            Proposition::Conjunction(conjuncts) => {
                writer.u8(7);
                writer.len("conjuncts", conjuncts.len())?;
            }
            Proposition::Implication { .. } => writer.u8(8),
            Proposition::ContentConservation(conservation) => {
                writer.u8(9);
                encode_content_algebra(writer, conservation.algebra())?;
                encode_content_term(writer, conservation.left())?;
                encode_content_term(writer, conservation.right())?;
            }
            Proposition::Disjunction(disjuncts) => {
                writer.u8(10);
                writer.len("disjuncts", disjuncts.len())?;
            }
            Proposition::IeeeFloatComparison {
                kind,
                format,
                left,
                right,
            } => {
                writer.u8(11);
                encode_ieee_float_comparison_kind(writer, *kind);
                encode_ieee_float_format(writer, *format);
                encode_ieee_float_field(writer, left)?;
                encode_ieee_float_field(writer, right)?;
            }
            Proposition::ScalarIeeeFloatComparison {
                kind,
                format,
                left,
                right,
            } => {
                writer.u8(17);
                encode_ieee_float_comparison_kind(writer, *kind);
                encode_ieee_float_format(writer, *format);
                encode_scalar_term(writer, left)?;
                encode_scalar_term(writer, right)?;
            }
            Proposition::ByteSequenceEqual { left, right } => {
                writer.u8(12);
                encode_byte_sequence_field(writer, left)?;
                encode_byte_sequence_field(writer, right)?;
            }
            Proposition::StructuralCaseMembership { subject, case } => {
                writer.u8(13);
                encode_canonical_structural_field(
                    writer,
                    subject.root(),
                    subject.path(),
                    "structural case subject path",
                )?;
                writer.id(*case);
            }
            Proposition::StructuralIdentity { left, right } => {
                writer.u8(18);
                for subject in [left, right] {
                    writer.id(subject.structural_type());
                    encode_canonical_structural_field(
                        writer,
                        subject.root(),
                        subject.path(),
                        "structural identity subject path",
                    )?;
                }
            }
        }
        Ok(())
    }

    fn read_proposition_row(
        reader: &mut Reader<'_>,
        children: &mut TableChildren<'_, Proposition>,
    ) -> Result<Proposition, CodecError> {
        Ok(match reader.u8()? {
            1 => Proposition::Truth,
            2 => Proposition::Falsehood,
            3 => Proposition::Atom(reader.id::<PropositionId>("PropositionId")?),
            4 => Proposition::Equal(decode_scalar_term(reader)?, decode_scalar_term(reader)?),
            5 => Proposition::LessThan(decode_scalar_term(reader)?, decode_scalar_term(reader)?),
            6 => Proposition::LessOrEqual(decode_scalar_term(reader)?, decode_scalar_term(reader)?),
            7 => {
                let count = bounded_count(reader, 4)?;
                Proposition::Conjunction(children.take(reader, count)?)
            }
            8 => {
                let mut operands = children.take(reader, 2)?.into_iter();
                match (operands.next(), operands.next()) {
                    (Some(premise), Some(conclusion)) => Proposition::Implication {
                        premise: Box::new(premise),
                        conclusion: Box::new(conclusion),
                    },
                    _ => return Err(CodecError::NonCanonicalTermTable("proposition")),
                }
            }
            9 => {
                let algebra = decode_content_algebra(reader)?;
                let left = decode_content_term(reader)?;
                let right = decode_content_term(reader)?;
                Proposition::ContentConservation(ContentConservation::new(algebra, left, right))
            }
            10 => {
                let count = bounded_count(reader, 4)?;
                Proposition::Disjunction(children.take(reader, count)?)
            }
            11 => Proposition::IeeeFloatComparison {
                kind: decode_ieee_float_comparison_kind(reader)?,
                format: decode_ieee_float_format(reader)?,
                left: decode_ieee_float_field(reader)?,
                right: decode_ieee_float_field(reader)?,
            },
            12 => Proposition::ByteSequenceEqual {
                left: decode_byte_sequence_field(reader)?,
                right: decode_byte_sequence_field(reader)?,
            },
            13 => {
                let (root, path) = decode_canonical_structural_field(reader)?;
                Proposition::StructuralCaseMembership {
                    subject: StructuralCaseSubject::new(root, path),
                    case: reader.id("StructuralCaseId")?,
                }
            }
            14 => Proposition::IntegerMathEqual(
                decode_integer_math_term(reader)?,
                decode_integer_math_term(reader)?,
            ),
            15 => Proposition::IntegerMathLessThan(
                decode_integer_math_term(reader)?,
                decode_integer_math_term(reader)?,
            ),
            16 => Proposition::IntegerMathLessOrEqual(
                decode_integer_math_term(reader)?,
                decode_integer_math_term(reader)?,
            ),
            17 => Proposition::ScalarIeeeFloatComparison {
                kind: decode_ieee_float_comparison_kind(reader)?,
                format: decode_ieee_float_format(reader)?,
                left: decode_scalar_term(reader)?,
                right: decode_scalar_term(reader)?,
            },
            18 => {
                let mut subject = || {
                    let structural_type = reader.id("StructuralTypeId")?;
                    let (root, path) = decode_canonical_structural_field(reader)?;
                    Ok::<_, CodecError>(StructuralIdentitySubject::new(root, path, structural_type))
                };
                Proposition::StructuralIdentity {
                    left: subject()?,
                    right: subject()?,
                }
            }
            tag => return Err(CodecError::InvalidTag("Proposition", tag)),
        })
    }
}

pub(crate) mod scalar_qualification_wire {
    use super::module_wire::declaration_wire::{
        decode_establishment_routes, decode_required_memberships, encode_establishment_routes,
        encode_required_memberships,
    };
    use super::wire::{require_increasing, strictly_increasing};
    use super::{
        CodecError, decode_counted,
        scalar_wire::{
            decode_ieee_float_value, decode_integer_type, decode_integer_value, decode_scalar_type,
            encode_ieee_float_value, encode_integer_type, encode_integer_value, encode_scalar_type,
        },
        wire::{Reader, Writer, decode_ids},
    };
    use semantic_vocabulary::ScalarQualificationSetId;
    use terminal_psi::{
        ScalarDomainDeclaration, ScalarDomainPredicate, ScalarFloatRange, ScalarIntegerRange,
        ScalarQualificationCatalog, ScalarQualificationCoercion, ScalarQualificationSet,
        ScalarReturnOrigin, ScalarReturnQualification, ScalarReturnSite, TerminalIdentityIndex,
    };

    pub(crate) fn encode(
        writer: &mut Writer,
        catalog: &ScalarQualificationCatalog,
    ) -> Result<(), CodecError> {
        writer.len("scalar domains", catalog.domains.len())?;
        for domain in &catalog.domains {
            writer.id(domain.id);
            writer.id(domain.semantic_domain);
            encode_required_memberships(writer, &domain.required_memberships)?;
            writer.u32(domain.identity.0);
            encode_scalar_type(writer, domain.carrier);
            writer.boolean(domain.predicate.is_some());
            if let Some(predicate) = &domain.predicate {
                writer.id(predicate.subject);
                super::encode_proposition(writer, &predicate.proposition)?;
            }
            encode_establishment_routes(writer, &domain.establishment_routes)?;
        }
        writer.len("scalar return qualifications", catalog.returns.len())?;
        for returned in &catalog.returns {
            writer.id(returned.machine);
            writer.id(returned.edge);
            writer.id(returned.source);
            writer.id(returned.result);
            match returned.site {
                ScalarReturnSite::NormalExit => writer.u8(0),
                ScalarReturnSite::Completion { argument_ordinal } => {
                    writer.u8(1);
                    writer.u32(argument_ordinal);
                }
            }
            writer.id(returned.domain);
            writer.len("scalar return origins", returned.origins.len())?;
            for origin in &returned.origins {
                match origin {
                    ScalarReturnOrigin::Forwarded => writer.u8(0),
                    ScalarReturnOrigin::Vacuous => writer.u8(2),
                    ScalarReturnOrigin::Established { route } => {
                        writer.u8(1);
                        writer.u32(*route);
                    }
                }
            }
        }
        writer.len("scalar qualification sets", catalog.sets.len())?;
        for set in &catalog.sets {
            writer.u64(set.id.get());
            writer.len("scalar qualification members", set.domains.len())?;
            for domain in &set.domains {
                writer.id(*domain);
            }
        }
        writer.len("scalar field qualifications", catalog.fields.len())?;
        for field in &catalog.fields {
            writer.id(field.field);
            encode_scalar_type(writer, field.value_type.scalar_type);
            writer.u64(field.value_type.qualifications.get());
        }
        writer.len("scalar qualification coercions", catalog.coercions.len())?;
        for coercion in &catalog.coercions {
            writer.id(coercion.machine);
            writer.id(coercion.edge);
            writer.u32(coercion.argument_ordinal);
            writer.id(coercion.source);
            writer.id(coercion.destination);
        }
        writer.len(
            "scalar float entry ranges",
            catalog.float_entry_ranges.len(),
        )?;
        for range in &catalog.float_entry_ranges {
            writer.id(range.machine);
            writer.id(range.parameter);
            encode_ieee_float_value(writer, range.minimum);
            encode_ieee_float_value(writer, range.maximum);
            writer.boolean(range.maximum_inclusive);
        }
        writer.len(
            "scalar integer entry ranges",
            catalog.integer_entry_ranges.len(),
        )?;
        for range in &catalog.integer_entry_ranges {
            writer.id(range.machine);
            writer.id(range.parameter);
            encode_integer_type(writer, range.integer_type);
            encode_integer_value(writer, range.minimum);
            encode_integer_value(writer, range.maximum);
        }
        Ok(())
    }

    pub(crate) fn decode(
        reader: &mut Reader<'_>,
    ) -> Result<ScalarQualificationCatalog, CodecError> {
        Ok(ScalarQualificationCatalog {
            domains: decode_counted(reader, |reader| {
                Ok(ScalarDomainDeclaration {
                    id: reader.id("ScalarDomainId")?,
                    semantic_domain: reader.id("DomainSemanticId")?,
                    required_memberships: decode_required_memberships(reader)?,
                    identity: TerminalIdentityIndex(reader.u32()?),
                    carrier: decode_scalar_type(reader)?,
                    predicate: if reader.boolean()? {
                        Some(ScalarDomainPredicate {
                            subject: reader.id("ValueId")?,
                            proposition: super::decode_proposition(reader)?,
                        })
                    } else {
                        None
                    },
                    establishment_routes: decode_establishment_routes(reader)?,
                })
            })?,
            returns: decode_counted(reader, |reader| {
                Ok(ScalarReturnQualification {
                    machine: reader.id("MachineId")?,
                    edge: reader.id("EdgeId")?,
                    source: reader.id("ValueId")?,
                    result: reader.id("ValueId")?,
                    site: match reader.u8()? {
                        0 => ScalarReturnSite::NormalExit,
                        1 => ScalarReturnSite::Completion {
                            argument_ordinal: reader.u32()?,
                        },
                        tag => return Err(CodecError::InvalidTag("ScalarReturnSite", tag)),
                    },
                    domain: reader.id("ScalarDomainId")?,
                    origins: decode_counted(reader, |reader| match reader.u8()? {
                        0 => Ok(ScalarReturnOrigin::Forwarded),
                        2 => Ok(ScalarReturnOrigin::Vacuous),
                        1 => Ok(ScalarReturnOrigin::Established {
                            route: reader.u32()?,
                        }),
                        tag => Err(CodecError::InvalidTag("ScalarReturnOrigin", tag)),
                    })?,
                })
            })?,
            sets: decode_counted(reader, |reader| {
                Ok(ScalarQualificationSet {
                    id: ScalarQualificationSetId::new(reader.u64()?),
                    domains: decode_ids(reader, "ScalarDomainId")?,
                })
            })?,
            fields: decode_counted(reader, |reader| {
                Ok(terminal_psi::ScalarFieldQualification {
                    field: reader.id("StructuralFieldId")?,
                    value_type: semantic_vocabulary::QualifiedScalarType {
                        scalar_type: decode_scalar_type(reader)?,
                        qualifications: ScalarQualificationSetId::new(reader.u64()?),
                    },
                })
            })?,
            coercions: decode_counted(reader, |reader| {
                Ok(ScalarQualificationCoercion {
                    machine: reader.id("MachineId")?,
                    edge: reader.id("EdgeId")?,
                    argument_ordinal: reader.u32()?,
                    source: reader.id("ValueId")?,
                    destination: reader.id("ValueId")?,
                })
            })?,
            float_entry_ranges: decode_counted(reader, |reader| {
                Ok(ScalarFloatRange {
                    machine: reader.id("MachineId")?,
                    parameter: reader.id("ValueId")?,
                    minimum: decode_ieee_float_value(reader)?,
                    maximum: decode_ieee_float_value(reader)?,
                    maximum_inclusive: reader.boolean()?,
                })
            })?,
            integer_entry_ranges: decode_counted(reader, |reader| {
                Ok(ScalarIntegerRange {
                    machine: reader.id("MachineId")?,
                    parameter: reader.id("ValueId")?,
                    integer_type: decode_integer_type(reader)?,
                    minimum: decode_integer_value(reader)?,
                    maximum: decode_integer_value(reader)?,
                })
            })?,
        })
    }

    pub(crate) fn validate(catalog: &ScalarQualificationCatalog) -> Result<(), CodecError> {
        require_increasing(
            catalog.fields.iter().map(|row| row.field),
            "scalar field qualifications",
        )?;
        require_increasing(
            catalog
                .returns
                .iter()
                .map(|row| (row.machine, row.edge, row.domain)),
            "scalar return qualifications",
        )?;
        for returned in &catalog.returns {
            if returned.origins.is_empty() || !strictly_increasing(returned.origins.iter()) {
                return Err(CodecError::NonCanonicalOrder("scalar return origins"));
            }
        }
        require_increasing(
            catalog.domains.iter().map(|domain| domain.id),
            "scalar domains",
        )?;
        for domain in &catalog.domains {
            require_increasing(
                domain
                    .establishment_routes
                    .as_deref()
                    .unwrap_or_default()
                    .iter(),
                "scalar domain establishment routes",
            )?;
            if domain
                .establishment_routes
                .as_deref()
                .unwrap_or_default()
                .iter()
                .any(|route| route.identity().is_absent())
            {
                return Err(CodecError::NonCanonicalEncoding);
            }
        }
        require_increasing(
            catalog.sets.iter().map(|set| set.id),
            "scalar qualification sets",
        )?;
        for (index, set) in catalog.sets.iter().enumerate() {
            if set.id.is_empty() || set.domains.is_empty() {
                return Err(CodecError::NonCanonicalEncoding);
            }
            require_increasing(set.domains.iter().copied(), "scalar qualification members")?;
            if catalog.sets[..index]
                .iter()
                .any(|previous| previous.domains == set.domains)
            {
                return Err(CodecError::NonCanonicalEncoding);
            }
        }
        require_increasing(
            catalog
                .coercions
                .iter()
                .map(|coercion| (coercion.machine, coercion.edge, coercion.argument_ordinal)),
            "scalar qualification coercions",
        )?;
        require_increasing(
            catalog
                .float_entry_ranges
                .iter()
                .map(|range| (range.machine, range.parameter)),
            "scalar float entry ranges",
        )?;
        if catalog
            .float_entry_ranges
            .iter()
            .any(|range| range.minimum.format() != range.maximum.format())
        {
            return Err(CodecError::NonCanonicalEncoding);
        }
        require_increasing(
            catalog
                .integer_entry_ranges
                .iter()
                .map(|range| (range.machine, range.parameter)),
            "scalar integer entry ranges",
        )?;
        if catalog
            .integer_entry_ranges
            .iter()
            .any(|range| !range.ordered())
        {
            return Err(CodecError::NonCanonicalEncoding);
        }
        Ok(())
    }
}

pub(crate) mod scalar_term_wire {

    use semantic_vocabulary::{
        CanonicalStructuralPathSegment, IntegerType, PropositionError, ScalarReadProjection,
        ScalarTerm,
    };

    use super::CodecError;
    use super::scalar_wire::{
        decode_integer_type, decode_integer_value, decode_scalar_type, encode_integer_type,
        encode_integer_value, encode_scalar_type,
    };
    use super::structural_field_wire::{
        decode_canonical_structural_field, decode_ieee_float_format,
        encode_canonical_structural_field, encode_ieee_float_format,
    };
    use super::wire::{Reader, Writer, decode_counted};
    use crate::term_table::{TableChildren, TableTerm, decode_term_table, encode_term_table};

    pub(crate) fn encode_scalar_terms(
        writer: &mut Writer,
        terms: &[ScalarTerm],
    ) -> Result<(), CodecError> {
        writer.len("scalar terms", terms.len())?;
        for term in terms {
            encode_scalar_term(writer, term)?;
        }
        Ok(())
    }

    /// Encodes one scalar term as a postorder table (`crate::term_table`).
    pub(crate) fn encode_scalar_term(
        writer: &mut Writer,
        term: &ScalarTerm,
    ) -> Result<(), CodecError> {
        encode_term_table(writer, term)
    }

    pub(crate) fn decode_scalar_terms(
        reader: &mut Reader<'_>,
    ) -> Result<Vec<ScalarTerm>, CodecError> {
        decode_counted(reader, decode_scalar_term)
    }

    pub(crate) fn decode_scalar_term(reader: &mut Reader<'_>) -> Result<ScalarTerm, CodecError> {
        decode_term_table(reader)
    }

    impl TableTerm for ScalarTerm {
        const LABEL: &'static str = "scalar term";

        fn children<'term>(&'term self, children: &mut Vec<&'term Self>) {
            children.extend(self.operands());
        }

        fn write_row(&self, writer: &mut Writer) -> Result<(), CodecError> {
            write_scalar_row(writer, self)
        }

        fn read_row(
            reader: &mut Reader<'_>,
            children: &mut TableChildren<'_, Self>,
        ) -> Result<Self, CodecError> {
            read_scalar_row(reader, children)
        }
    }

    /// One row: the form tag and fields; operand handles follow in the table.
    /// Every node passes its local check, as the decoder's constructors require.
    fn write_scalar_row(writer: &mut Writer, term: &ScalarTerm) -> Result<(), CodecError> {
        term.validate_after_operands()
            .map_err(CodecError::MalformedProposition)?;
        match term {
            ScalarTerm::Application {
                application,
                scalar_type,
            } => {
                writer.u8(41);
                writer.id(*application);
                encode_scalar_type(writer, *scalar_type);
            }
            ScalarTerm::StructuralRead {
                root,
                path,
                scalar_type,
            } => {
                writer.u8(40);
                writer.id(*root);
                encode_scalar_type(writer, *scalar_type);
                writer.len("scalar read path", path.len())?;
                for projection in path {
                    match projection {
                        ScalarReadProjection::Referent => writer.u8(5),
                        ScalarReadProjection::Extent => writer.u8(6),
                        ScalarReadProjection::Static(CanonicalStructuralPathSegment::Field(
                            field,
                        )) => {
                            writer.u8(1);
                            writer.id(*field);
                        }
                        ScalarReadProjection::Static(
                            CanonicalStructuralPathSegment::FixedIndex(index),
                        ) => {
                            writer.u8(2);
                            writer.u64(*index);
                        }
                        ScalarReadProjection::Static(CanonicalStructuralPathSegment::Case(
                            case,
                        )) => {
                            writer.u8(3);
                            writer.id(*case);
                        }
                        ScalarReadProjection::Static(CanonicalStructuralPathSegment::Referent) => {
                            return Err(CodecError::MalformedProposition(
                                semantic_vocabulary::PropositionError::NonCanonicalScalarReadProjection,
                            ));
                        }
                        ScalarReadProjection::Index(_) => writer.u8(4),
                    }
                }
            }
            ScalarTerm::IeeeFloatField { root, path, format } => {
                writer.u8(38);
                encode_canonical_structural_field(writer, *root, path, "IEEE float field path")?;
                encode_ieee_float_format(writer, *format);
            }
            ScalarTerm::IeeeFloatLiteral(value) => {
                writer.u8(37);
                match value {
                    semantic_vocabulary::IeeeFloatValue::Binary32(bits) => {
                        writer.u8(1);
                        writer.u32(*bits);
                    }
                    semantic_vocabulary::IeeeFloatValue::Binary64(bits) => {
                        writer.u8(2);
                        writer.u64(*bits);
                    }
                }
            }
            ScalarTerm::Value { id, scalar_type } => {
                writer.u8(1);
                writer.id(*id);
                encode_scalar_type(writer, *scalar_type);
            }
            ScalarTerm::BooleanField { root, path } => {
                writer.u8(34);
                encode_canonical_structural_field(writer, *root, path, "Boolean field path")?;
            }
            ScalarTerm::IntegerField {
                root,
                path,
                scalar_type,
            } => {
                writer.u8(35);
                encode_canonical_structural_field(writer, *root, path, "Integer field path")?;
                encode_integer_type(writer, *scalar_type);
            }
            ScalarTerm::ByteSequencePredicate {
                predicate,
                root,
                path,
            } => {
                writer.u8(39);
                writer.u8(predicate.mask_bit());
                encode_canonical_structural_field(writer, *root, path, "Byte predicate path")?;
            }
            ScalarTerm::ViewExtent {
                root,
                path,
                scalar_type,
            } => {
                writer.u8(36);
                encode_canonical_structural_field(writer, *root, path, "View extent path")?;
                encode_integer_type(writer, *scalar_type);
            }
            ScalarTerm::Boolean(value) => {
                writer.u8(2);
                writer.u8(u8::from(*value));
            }
            ScalarTerm::BooleanNot { .. } => writer.u8(10),
            ScalarTerm::BooleanEqual { .. } => writer.u8(11),
            ScalarTerm::IntegerBitwiseNot { scalar_type, .. } => {
                writer.u8(20);
                encode_integer_type(writer, *scalar_type);
            }
            ScalarTerm::IntegerWiden {
                source_type,
                target_type,
                ..
            } => typed_pair_row(writer, 21, *source_type, *target_type),
            ScalarTerm::IntegerExactCast {
                source_type,
                target_type,
                ..
            } => typed_pair_row(writer, 22, *source_type, *target_type),
            ScalarTerm::WrappingIntegerShiftLeft {
                value_type,
                count_type,
                ..
            } => typed_pair_row(writer, 18, *value_type, *count_type),
            ScalarTerm::WrappingIntegerShiftRight {
                value_type,
                count_type,
                ..
            } => typed_pair_row(writer, 19, *value_type, *count_type),
            ScalarTerm::ExactIntegerShiftRight {
                value_type,
                count_type,
                ..
            } => typed_pair_row(writer, 23, *value_type, *count_type),
            ScalarTerm::ExactIntegerShiftLeft {
                value_type,
                count_type,
                ..
            } => typed_pair_row(writer, 24, *value_type, *count_type),
            ScalarTerm::Integer { scalar_type, value } => {
                writer.u8(3);
                encode_integer_type(writer, *scalar_type);
                encode_integer_value(writer, *value);
            }
            ScalarTerm::IntegerEqual { scalar_type, .. } => typed_row(writer, 12, *scalar_type),
            ScalarTerm::IntegerLessThan { scalar_type, .. } => typed_row(writer, 13, *scalar_type),
            ScalarTerm::IntegerLessOrEqual { scalar_type, .. } => {
                typed_row(writer, 14, *scalar_type)
            }
            ScalarTerm::IntegerBitwiseAnd { scalar_type, .. } => {
                typed_row(writer, 15, *scalar_type)
            }
            ScalarTerm::IntegerBitwiseOr { scalar_type, .. } => typed_row(writer, 16, *scalar_type),
            ScalarTerm::IntegerBitwiseXor { scalar_type, .. } => {
                typed_row(writer, 17, *scalar_type)
            }
            ScalarTerm::ExactIntegerAdd { scalar_type, .. } => typed_row(writer, 25, *scalar_type),
            ScalarTerm::ExactIntegerSubtract { scalar_type, .. } => {
                typed_row(writer, 26, *scalar_type)
            }
            ScalarTerm::ExactIntegerMultiply { scalar_type, .. } => {
                typed_row(writer, 27, *scalar_type)
            }
            ScalarTerm::ExactIntegerDivide { scalar_type, .. } => {
                typed_row(writer, 28, *scalar_type)
            }
            ScalarTerm::ExactIntegerRemainder { scalar_type, .. } => {
                typed_row(writer, 29, *scalar_type)
            }
            ScalarTerm::WrappingIntegerDivide { scalar_type, .. } => {
                typed_row(writer, 30, *scalar_type)
            }
            ScalarTerm::WrappingIntegerRemainder { scalar_type, .. } => {
                typed_row(writer, 31, *scalar_type)
            }
            ScalarTerm::SaturatingIntegerDivide { scalar_type, .. } => {
                typed_row(writer, 32, *scalar_type)
            }
            ScalarTerm::SaturatingIntegerRemainder { scalar_type, .. } => {
                typed_row(writer, 33, *scalar_type)
            }
            ScalarTerm::WrappingIntegerAdd { scalar_type, .. } => {
                typed_row(writer, 4, *scalar_type)
            }
            ScalarTerm::SaturatingIntegerAdd { scalar_type, .. } => {
                typed_row(writer, 5, *scalar_type)
            }
            ScalarTerm::WrappingIntegerSubtract { scalar_type, .. } => {
                typed_row(writer, 6, *scalar_type)
            }
            ScalarTerm::SaturatingIntegerSubtract { scalar_type, .. } => {
                typed_row(writer, 7, *scalar_type)
            }
            ScalarTerm::WrappingIntegerMultiply { scalar_type, .. } => {
                typed_row(writer, 8, *scalar_type)
            }
            ScalarTerm::SaturatingIntegerMultiply { scalar_type, .. } => {
                typed_row(writer, 9, *scalar_type)
            }
        }
        Ok(())
    }

    fn typed_row(writer: &mut Writer, tag: u8, scalar_type: IntegerType) {
        writer.u8(tag);
        encode_integer_type(writer, scalar_type);
    }

    fn typed_pair_row(writer: &mut Writer, tag: u8, first: IntegerType, second: IntegerType) {
        writer.u8(tag);
        encode_integer_type(writer, first);
        encode_integer_type(writer, second);
    }

    type Binary = fn(IntegerType, ScalarTerm, ScalarTerm) -> Result<ScalarTerm, PropositionError>;
    type TypedPairBinary = fn(
        IntegerType,
        IntegerType,
        ScalarTerm,
        ScalarTerm,
    ) -> Result<ScalarTerm, PropositionError>;
    type TypedPairUnary =
        fn(IntegerType, IntegerType, ScalarTerm) -> Result<ScalarTerm, PropositionError>;

    fn two(
        reader: &mut Reader<'_>,
        children: &mut TableChildren<'_, ScalarTerm>,
    ) -> Result<(ScalarTerm, ScalarTerm), CodecError> {
        let mut operands = children.take(reader, 2)?.into_iter();
        match (operands.next(), operands.next()) {
            (Some(left), Some(right)) => Ok((left, right)),
            _ => Err(CodecError::NonCanonicalTermTable("scalar term")),
        }
    }

    fn one(
        reader: &mut Reader<'_>,
        children: &mut TableChildren<'_, ScalarTerm>,
    ) -> Result<ScalarTerm, CodecError> {
        children
            .take(reader, 1)?
            .pop()
            .ok_or(CodecError::NonCanonicalTermTable("scalar term"))
    }

    fn binary_row(
        reader: &mut Reader<'_>,
        children: &mut TableChildren<'_, ScalarTerm>,
        constructor: Binary,
    ) -> Result<ScalarTerm, CodecError> {
        let scalar_type = decode_integer_type(reader)?;
        let (left, right) = two(reader, children)?;
        constructor(scalar_type, left, right).map_err(CodecError::MalformedProposition)
    }

    fn typed_pair_binary_row(
        reader: &mut Reader<'_>,
        children: &mut TableChildren<'_, ScalarTerm>,
        constructor: TypedPairBinary,
    ) -> Result<ScalarTerm, CodecError> {
        let first = decode_integer_type(reader)?;
        let second = decode_integer_type(reader)?;
        let (value, count) = two(reader, children)?;
        constructor(first, second, value, count).map_err(CodecError::MalformedProposition)
    }

    fn typed_pair_unary_row(
        reader: &mut Reader<'_>,
        children: &mut TableChildren<'_, ScalarTerm>,
        constructor: TypedPairUnary,
    ) -> Result<ScalarTerm, CodecError> {
        let source = decode_integer_type(reader)?;
        let target = decode_integer_type(reader)?;
        let operand = one(reader, children)?;
        constructor(source, target, operand).map_err(CodecError::MalformedProposition)
    }

    fn read_scalar_row(
        reader: &mut Reader<'_>,
        children: &mut TableChildren<'_, ScalarTerm>,
    ) -> Result<ScalarTerm, CodecError> {
        Ok(match reader.u8()? {
            41 => ScalarTerm::Application {
                application: reader.id("ScalarApplicationId")?,
                scalar_type: decode_scalar_type(reader)?,
            },
            1 => ScalarTerm::value(reader.id("ValueId")?, decode_scalar_type(reader)?),
            2 => ScalarTerm::boolean(reader.boolean()?),
            3 => {
                let scalar_type = decode_integer_type(reader)?;
                let value = decode_integer_value(reader)?;
                ScalarTerm::integer(scalar_type, value).map_err(CodecError::MalformedProposition)?
            }
            4 => binary_row(reader, children, ScalarTerm::wrapping_integer_add)?,
            5 => binary_row(reader, children, ScalarTerm::saturating_integer_add)?,
            6 => binary_row(reader, children, ScalarTerm::wrapping_integer_subtract)?,
            7 => binary_row(reader, children, ScalarTerm::saturating_integer_subtract)?,
            8 => binary_row(reader, children, ScalarTerm::wrapping_integer_multiply)?,
            9 => binary_row(reader, children, ScalarTerm::saturating_integer_multiply)?,
            10 => ScalarTerm::boolean_not(one(reader, children)?)
                .map_err(CodecError::MalformedProposition)?,
            11 => {
                let (left, right) = two(reader, children)?;
                ScalarTerm::boolean_equal(left, right).map_err(CodecError::MalformedProposition)?
            }
            12 => binary_row(reader, children, ScalarTerm::integer_equal)?,
            13 => binary_row(reader, children, ScalarTerm::integer_less_than)?,
            14 => binary_row(reader, children, ScalarTerm::integer_less_or_equal)?,
            15 => binary_row(reader, children, ScalarTerm::integer_bitwise_and)?,
            16 => binary_row(reader, children, ScalarTerm::integer_bitwise_or)?,
            17 => binary_row(reader, children, ScalarTerm::integer_bitwise_xor)?,
            18 => typed_pair_binary_row(reader, children, ScalarTerm::wrapping_integer_shift_left)?,
            19 => {
                typed_pair_binary_row(reader, children, ScalarTerm::wrapping_integer_shift_right)?
            }
            20 => {
                let scalar_type = decode_integer_type(reader)?;
                let operand = one(reader, children)?;
                ScalarTerm::integer_bitwise_not(scalar_type, operand)
                    .map_err(CodecError::MalformedProposition)?
            }
            21 => typed_pair_unary_row(reader, children, ScalarTerm::integer_widen)?,
            22 => typed_pair_unary_row(reader, children, ScalarTerm::integer_exact_cast)?,
            23 => typed_pair_binary_row(reader, children, ScalarTerm::exact_integer_shift_right)?,
            24 => typed_pair_binary_row(reader, children, ScalarTerm::exact_integer_shift_left)?,
            25 => binary_row(reader, children, ScalarTerm::exact_integer_add)?,
            26 => binary_row(reader, children, ScalarTerm::exact_integer_subtract)?,
            27 => binary_row(reader, children, ScalarTerm::exact_integer_multiply)?,
            28 => binary_row(reader, children, ScalarTerm::exact_integer_divide)?,
            29 => binary_row(reader, children, ScalarTerm::exact_integer_remainder)?,
            30 => binary_row(reader, children, ScalarTerm::wrapping_integer_divide)?,
            31 => binary_row(reader, children, ScalarTerm::wrapping_integer_remainder)?,
            32 => binary_row(reader, children, ScalarTerm::saturating_integer_divide)?,
            33 => binary_row(reader, children, ScalarTerm::saturating_integer_remainder)?,
            34 => {
                let (root, path) = decode_canonical_structural_field(reader)?;
                ScalarTerm::boolean_field_path(root, path)
            }
            35 => {
                let (root, path) = decode_canonical_structural_field(reader)?;
                ScalarTerm::integer_field_path(root, path, decode_integer_type(reader)?)
            }
            36 => {
                let (root, path) = decode_canonical_structural_field(reader)?;
                ScalarTerm::view_extent(root, path, decode_integer_type(reader)?)
            }
            37 => ScalarTerm::IeeeFloatLiteral(match reader.u8()? {
                1 => semantic_vocabulary::IeeeFloatValue::Binary32(reader.u32()?),
                2 => semantic_vocabulary::IeeeFloatValue::Binary64(reader.u64()?),
                tag => return Err(CodecError::InvalidTag("IeeeFloatValue", tag)),
            }),
            38 => {
                let (root, path) = decode_canonical_structural_field(reader)?;
                let term = ScalarTerm::ieee_float_field_path(
                    root,
                    path,
                    decode_ieee_float_format(reader)?,
                );
                term.validate_after_operands()
                    .map_err(CodecError::MalformedProposition)?;
                term
            }
            39 => {
                let predicate = match reader.u8()? {
                    1 => semantic_vocabulary::ByteSequencePredicate::ValidUtf8,
                    2 => semantic_vocabulary::ByteSequencePredicate::NoNul,
                    4 => semantic_vocabulary::ByteSequencePredicate::AsciiOnly,
                    8 => semantic_vocabulary::ByteSequencePredicate::NonEmpty,
                    tag => return Err(CodecError::InvalidTag("ByteSequencePredicate", tag)),
                };
                let (root, path) = decode_canonical_structural_field(reader)?;
                ScalarTerm::byte_sequence_predicate(predicate, root, path)
            }
            40 => {
                let root = reader.id("PlaceId")?;
                let scalar_type = decode_scalar_type(reader)?;
                // `None` marks a dynamic index whose operand handle follows the path.
                let segments = decode_counted(reader, |reader| {
                    Ok(match reader.u8()? {
                        1 => Some(ScalarReadProjection::Static(
                            CanonicalStructuralPathSegment::Field(reader.id("StructuralFieldId")?),
                        )),
                        2 => Some(ScalarReadProjection::Static(
                            CanonicalStructuralPathSegment::FixedIndex(reader.u64()?),
                        )),
                        3 => Some(ScalarReadProjection::Static(
                            CanonicalStructuralPathSegment::Case(reader.id("StructuralCaseId")?),
                        )),
                        4 => None,
                        5 => Some(ScalarReadProjection::Referent),
                        6 => Some(ScalarReadProjection::Extent),
                        tag => return Err(CodecError::InvalidTag("ScalarReadProjection", tag)),
                    })
                })?;
                let index_count = segments.iter().filter(|segment| segment.is_none()).count();
                let mut indices = children.take(reader, index_count)?.into_iter();
                let mut path = Vec::with_capacity(segments.len());
                for segment in segments {
                    path.push(match segment {
                        Some(segment) => segment,
                        None => ScalarReadProjection::Index(Box::new(
                            indices
                                .next()
                                .ok_or(CodecError::NonCanonicalTermTable("scalar term"))?,
                        )),
                    });
                }
                let term = ScalarTerm::StructuralRead {
                    root,
                    path,
                    scalar_type,
                };
                term.validate_after_operands()
                    .map_err(CodecError::MalformedProposition)?;
                term
            }
            tag => return Err(CodecError::InvalidTag("ScalarTerm", tag)),
        })
    }

    #[cfg(test)]
    mod scalar_read_projection_tests {
        use super::*;
        use semantic_vocabulary::{PlaceId, ScalarType};

        #[test]
        fn explicit_reference_read_has_an_exact_canonical_wire_roundtrip() {
            let term = ScalarTerm::structural_read(
                PlaceId::new(1).unwrap(),
                vec![ScalarReadProjection::Referent],
                ScalarType::Boolean,
            )
            .unwrap();
            let mut writer = Writer::default();
            encode_scalar_term(&mut writer, &term).unwrap();
            let bytes = writer.finish();
            let mut reader = Reader::new(&bytes);
            let decoded = decode_scalar_term(&mut reader).unwrap();
            assert_eq!(decoded, term);
            assert_eq!(reader.remaining(), 0);
            let mut repeated = Writer::default();
            encode_scalar_term(&mut repeated, &decoded).unwrap();
            assert_eq!(repeated.finish(), bytes);
        }

        #[test]
        fn static_reference_alias_cannot_be_published_on_the_scalar_read_wire() {
            let term = ScalarTerm::StructuralRead {
                root: PlaceId::new(1).unwrap(),
                path: vec![ScalarReadProjection::Static(
                    CanonicalStructuralPathSegment::Referent,
                )],
                scalar_type: ScalarType::Boolean,
            };
            assert!(matches!(
                encode_scalar_term(&mut Writer::default(), &term),
                Err(CodecError::MalformedProposition(
                    PropositionError::NonCanonicalScalarReadProjection
                ))
            ));
        }
    }
}

pub(crate) mod scalar_wire {

    use semantic_vocabulary::{
        IeeeFloatValue, IntegerCarrier, IntegerSign, IntegerType, IntegerValue, ScalarType,
    };

    use super::CodecError;
    use super::structural_field_wire::{decode_ieee_float_format, encode_ieee_float_format};
    use super::wire::{Reader, Writer};

    pub(crate) fn encode_scalar_type(writer: &mut Writer, scalar_type: ScalarType) {
        match scalar_type {
            ScalarType::Boolean => writer.u8(1),
            ScalarType::Integer(integer_type) => {
                writer.u8(2);
                encode_integer_type(writer, integer_type);
            }
            ScalarType::IeeeFloat(format) => {
                writer.u8(3);
                encode_ieee_float_format(writer, format);
            }
        }
    }

    pub(crate) fn encode_ieee_float_value(writer: &mut Writer, value: IeeeFloatValue) {
        match value {
            IeeeFloatValue::Binary32(bits) => {
                writer.u8(1);
                writer.u32(bits);
            }
            IeeeFloatValue::Binary64(bits) => {
                writer.u8(2);
                writer.u64(bits);
            }
        }
    }

    pub(crate) fn encode_integer_type(writer: &mut Writer, integer_type: IntegerType) {
        writer.u8(match (integer_type.carrier(), integer_type.sign()) {
            (IntegerCarrier::Fixed, IntegerSign::Signed) => 1,
            (IntegerCarrier::Fixed, IntegerSign::Unsigned) => 2,
            (IntegerCarrier::Address, IntegerSign::Unsigned) => 3,
            (IntegerCarrier::Address, IntegerSign::Signed) => {
                unreachable!("address carriers are unsigned")
            }
        });
        writer.u16(integer_type.bits());
    }

    pub(crate) fn encode_integer_value(writer: &mut Writer, value: IntegerValue) {
        match value {
            IntegerValue::Signed(value) => {
                writer.u8(1);
                writer.bytes(&value.to_le_bytes());
            }
            IntegerValue::Unsigned(value) => {
                writer.u8(2);
                writer.bytes(&value.to_le_bytes());
            }
        }
    }

    pub(crate) fn decode_scalar_type(reader: &mut Reader<'_>) -> Result<ScalarType, CodecError> {
        Ok(match reader.u8()? {
            1 => ScalarType::Boolean,
            2 => ScalarType::Integer(decode_integer_type(reader)?),
            3 => ScalarType::IeeeFloat(decode_ieee_float_format(reader)?),
            tag => return Err(CodecError::InvalidTag("ScalarType", tag)),
        })
    }

    pub(crate) fn decode_ieee_float_value(
        reader: &mut Reader<'_>,
    ) -> Result<IeeeFloatValue, CodecError> {
        Ok(match reader.u8()? {
            1 => IeeeFloatValue::Binary32(reader.u32()?),
            2 => IeeeFloatValue::Binary64(reader.u64()?),
            tag => return Err(CodecError::InvalidTag("IeeeFloatValue", tag)),
        })
    }

    pub(crate) fn decode_integer_type(reader: &mut Reader<'_>) -> Result<IntegerType, CodecError> {
        let tag = reader.u8()?;
        let bits = reader.u16()?;
        match tag {
            1 => IntegerType::new(IntegerSign::Signed, bits),
            2 => IntegerType::new(IntegerSign::Unsigned, bits),
            3 => IntegerType::address(bits),
            tag => return Err(CodecError::InvalidTag("IntegerSign", tag)),
        }
        .map_err(CodecError::MalformedProposition)
    }

    pub(crate) fn decode_integer_value(
        reader: &mut Reader<'_>,
    ) -> Result<IntegerValue, CodecError> {
        Ok(match reader.u8()? {
            1 => IntegerValue::Signed(i128::from_le_bytes(reader.array()?)),
            2 => IntegerValue::Unsigned(u128::from_le_bytes(reader.array()?)),
            tag => return Err(CodecError::InvalidTag("IntegerValue", tag)),
        })
    }
}
