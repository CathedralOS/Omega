#![forbid(unsafe_code)]

//! Canonical binary encoding and semantic identity for terminal Psi.
//!
//! Only the semantic module is encoded here. Proof bundles, installation
//! records, and debug/source maps have separate identities and can be replaced
//! without changing [`TerminalPsiIdentity`].

use std::collections::BTreeMap;

mod artifact_manifest;
mod debug_map;
mod proof_bundle;

pub use artifact_manifest::{
    ArtifactManifestError, SectionFingerprint, TerminalArtifactIdentity, TerminalArtifactManifest,
    build_artifact_manifest, validate_artifact_manifest,
};
pub use debug_map::{
    DebugFileId, DebugMapError, DebugSite, DebugSourceDigest, DebugSourceFile, DebugSourceOrigin,
    DebugSourceSpan, DebugSubject, TerminalDebugMap, decode_debug_map, encode_debug_map,
    source_digest, validate_debug_map,
};
pub use proof_bundle::{
    ProofBundleFingerprint, ProofCodecError, decode_proof_bundle, encode_proof_bundle,
    proof_bundle_fingerprint,
};
pub use psi_terminal::{SemanticFingerprint, TerminalPsiIdentity};

use psi_core::{
    ClaimId, ContentAlgebra, ContentAlgebraKind, ContentConservation, ContentDomainId,
    ContentPlaceSegment, ContentPlaceVersion, ContentProjectionIdentity, ContentStructuralPlace,
    ContentTerm, IntegerCarrier, IntegerSign, IntegerType, IntegerValue, Proposition,
    PropositionError, PropositionId, PsiSemanticId, ScalarTerm, ScalarType, StructuralPlaceKind,
};
use psi_terminal::{
    Block, ClaimContentProjection, ContentEntryClaim, ContentIdentityReshuffle,
    ContentPartitionComposition, ContentPlaceSubstitution, ContractClause, CrashCause,
    CrashContextMaximum, MachineContract, Operation, OperationKind, PropositionApplicationIdentity,
    PropositionBinderArgumentIdentity, PropositionBinderArgumentKind, PropositionBinderDeclaration,
    PropositionBinderKind, PropositionDeclaration, PropositionEvidence, SemanticVersion,
    StructuralPlaceDeclaration, SuccessorEdge, TerminalMachine, TerminalModule, Terminator,
    ValueDeclaration,
};
use psi_terminal_verifier::{ModuleError, validate_module};
use sha2::{Digest, Sha256};

const MAGIC: &[u8; 8] = b"PSITERM\0";
const FORMAT_VERSION: u16 = 1;
const FINGERPRINT_DOMAIN: &[u8] = b"psi-terminal-semantic-fingerprint-v1\0";
const MAX_PROPOSITION_DEPTH: usize = 256;
const MAX_SCALAR_TERM_DEPTH: usize = 256;
const MAX_CONTENT_TERM_DEPTH: usize = 256;
const MAX_CONTENT_IDENTITY_BYTES: usize = 1 << 20;

pub fn encode_module(module: &TerminalModule) -> Result<Vec<u8>, CodecError> {
    validate_canonical_order(module)?;
    validate_module(module).map_err(CodecError::InvalidModule)?;
    encode_raw(module)
}

pub fn decode_module(bytes: &[u8]) -> Result<TerminalModule, CodecError> {
    let mut reader = Reader::new(bytes);
    if reader.take(MAGIC.len())? != MAGIC {
        return Err(CodecError::InvalidMagic);
    }
    let format_version = reader.u16()?;
    if format_version != FORMAT_VERSION {
        return Err(CodecError::UnsupportedFormatVersion(format_version));
    }
    let module = decode_module_body(&mut reader)?;
    if reader.remaining() != 0 {
        return Err(CodecError::TrailingBytes(reader.remaining()));
    }
    validate_canonical_order(&module)?;
    validate_module(&module).map_err(CodecError::InvalidModule)?;
    if encode_raw(&module)? != bytes {
        return Err(CodecError::NonCanonicalEncoding);
    }
    Ok(module)
}

pub fn semantic_fingerprint(module: &TerminalModule) -> Result<SemanticFingerprint, CodecError> {
    let bytes = encode_module(module)?;
    Ok(fingerprint_bytes(&bytes))
}

pub fn terminal_psi_identity(module: &TerminalModule) -> Result<TerminalPsiIdentity, CodecError> {
    Ok(TerminalPsiIdentity {
        semantic_version: module.semantic_version,
        program_fingerprint: semantic_fingerprint(module)?,
    })
}

/// Re-encode a valid older semantic module under the current vocabulary.
///
/// Migration is deliberately explicit: it preserves the executable graph and
/// proof obligations, translates any newly required semantic metadata, and
/// changes semantic version and therefore program fingerprint. Older bytes are
/// never silently reinterpreted as the current version.
pub fn migrate_module_to_current(module: &TerminalModule) -> Result<TerminalModule, CodecError> {
    validate_canonical_order(module)?;
    validate_module(module).map_err(CodecError::InvalidModule)?;
    let mut migrated = module.clone();
    if migrated.semantic_version < SemanticVersion::V14 {
        for machine in &mut migrated.machines {
            let mut claim_remap = BTreeMap::new();
            for (index, reshuffle) in machine.content_identity_reshuffles.iter().enumerate() {
                let claim = ClaimId::new(
                    u64::try_from(index)
                        .expect("an in-memory claim count fits u64")
                        .checked_add(1)
                        .expect("an in-memory claim count cannot exhaust u64"),
                )
                .expect("dense claim identities begin at one");
                claim_remap.insert(reshuffle.claim, claim);
            }
            for reshuffle in &mut machine.content_identity_reshuffles {
                reshuffle.claim = claim_remap[&reshuffle.claim];
            }
            for composition in &mut machine.content_partition_compositions {
                for claim in &mut composition.input_claims {
                    *claim = claim_remap[claim];
                }
                composition.input_claims.sort();
            }
            machine.content_entry_claims = machine
                .content_identity_reshuffles
                .iter()
                .map(|reshuffle| ContentEntryClaim {
                    claim: reshuffle.claim,
                    input: reshuffle.input.clone(),
                    projections: reshuffle.projections.clone(),
                })
                .collect();
        }
    }
    if migrated.semantic_version < SemanticVersion::V24 {
        for machine in &mut migrated.machines {
            let mut causes = machine
                .blocks
                .iter()
                .filter_map(|block| match block.terminator {
                    Terminator::Crash { cause, .. } => Some(cause),
                    _ => None,
                })
                .collect::<Vec<_>>();
            causes.sort();
            causes.dedup();
            machine.contract.crash_context = causes
                .into_iter()
                .map(|cause| CrashContextMaximum {
                    cause,
                    maximum_scope: "ExecutionDomain".to_owned(),
                })
                .collect();
        }
    }
    migrated.semantic_version = SemanticVersion::CURRENT;
    validate_canonical_order(&migrated)?;
    validate_module(&migrated).map_err(CodecError::InvalidModule)?;
    Ok(migrated)
}

fn fingerprint_bytes(bytes: &[u8]) -> SemanticFingerprint {
    let mut digest = Sha256::new();
    digest.update(FINGERPRINT_DOMAIN);
    let byte_len =
        u64::try_from(bytes.len()).expect("terminal-Psi bytes fit the u64 digest domain");
    digest.update(byte_len.to_le_bytes());
    digest.update(bytes);
    SemanticFingerprint::from_bytes(digest.finalize().into())
}

fn validate_canonical_order(module: &TerminalModule) -> Result<(), CodecError> {
    if module.semantic_version < SemanticVersion::V16
        && (!module.proposition_declarations.is_empty()
            || !module.proposition_applications.is_empty())
    {
        return Err(CodecError::PropositionVocabularyRequiresV16);
    }
    if !strictly_increasing(module.proposition_declarations.iter().cloned().map(
        |mut declaration| {
            declaration.id = PropositionId::new(1).expect("one is nonzero");
            declaration
        },
    )) {
        return Err(CodecError::NonCanonicalOrder(
            "proposition declarations by semantic identity",
        ));
    }
    if !strictly_increasing(
        module
            .proposition_declarations
            .iter()
            .map(|declaration| declaration.id),
    ) {
        return Err(CodecError::NonCanonicalOrder(
            "proposition declarations by PropositionId",
        ));
    }
    if !strictly_increasing(module.proposition_applications.iter().cloned().map(
        |mut application| {
            application.id = PropositionId::new(1).expect("one is nonzero");
            application
        },
    )) {
        return Err(CodecError::NonCanonicalOrder(
            "proposition applications by semantic identity",
        ));
    }
    if !strictly_increasing(
        module
            .proposition_applications
            .iter()
            .map(|application| application.id),
    ) {
        return Err(CodecError::NonCanonicalOrder(
            "proposition applications by PropositionId",
        ));
    }
    if !strictly_increasing(module.machines.iter().map(|machine| machine.id)) {
        return Err(CodecError::NonCanonicalOrder("machines by MachineId"));
    }
    for machine in &module.machines {
        if machine
            .contract
            .crash_context
            .windows(2)
            .any(|pair| pair[0].cause >= pair[1].cause)
        {
            return Err(CodecError::NonCanonicalOrder(
                "crash context maxima by cause",
            ));
        }
        if !strictly_increasing(machine.blocks.iter().map(|block| block.id)) {
            return Err(CodecError::NonCanonicalOrder("blocks by BlockId"));
        }
        if !strictly_increasing(machine.structural_places.iter().map(|place| place.id)) {
            return Err(CodecError::NonCanonicalOrder(
                "structural places by PlaceId",
            ));
        }
        if !strictly_increasing(
            machine
                .content_entry_claims
                .iter()
                .map(|binding| binding.claim),
        ) {
            return Err(CodecError::NonCanonicalOrder(
                "content entry claims by ClaimId",
            ));
        }
        if machine
            .content_entry_claims
            .iter()
            .any(|binding| !strictly_increasing(binding.projections.iter()))
        {
            return Err(CodecError::NonCanonicalOrder(
                "entry-claim content projections by identity and algebra",
            ));
        }
        if !strictly_increasing(
            machine
                .content_identity_reshuffles
                .iter()
                .map(|reshuffle| reshuffle.claim),
        ) {
            return Err(CodecError::NonCanonicalOrder(
                "content identity reshuffles by ClaimId",
            ));
        }
        if machine
            .content_identity_reshuffles
            .iter()
            .any(|reshuffle| !strictly_increasing(reshuffle.projections.iter()))
        {
            return Err(CodecError::NonCanonicalOrder(
                "claim content projections by identity and algebra",
            ));
        }
        if !strictly_increasing(machine.content_partition_compositions.iter()) {
            return Err(CodecError::NonCanonicalOrder(
                "content partition compositions",
            ));
        }
        for composition in &machine.content_partition_compositions {
            if !strictly_increasing(
                composition
                    .source_structural_places
                    .iter()
                    .map(|place| place.id),
            ) {
                return Err(CodecError::NonCanonicalOrder(
                    "partition source structural places by PlaceId",
                ));
            }
            if !strictly_increasing(composition.input_claims.iter().copied()) {
                return Err(CodecError::NonCanonicalOrder(
                    "partition input claims by ClaimId",
                ));
            }
            if !strictly_increasing(composition.substitutions.iter()) {
                return Err(CodecError::NonCanonicalOrder(
                    "partition place substitutions",
                ));
            }
        }
        if !strictly_increasing(
            machine
                .contract
                .ensures
                .iter()
                .map(|clause| clause.obligation),
        ) {
            return Err(CodecError::NonCanonicalOrder("ensures by ObligationId"));
        }
        let propositions = machine.contract.requires.iter().chain(
            machine
                .contract
                .ensures
                .iter()
                .map(|clause| &clause.proposition),
        );
        for proposition in propositions {
            validate_canonical_proposition(proposition, 0)?;
        }
        if !canonical_propositions_strictly_increase(&machine.contract.requires)? {
            return Err(CodecError::NonCanonicalOrder("requires propositions"));
        }
    }
    Ok(())
}

fn validate_canonical_proposition(
    proposition: &Proposition,
    depth: usize,
) -> Result<(), CodecError> {
    if depth > MAX_PROPOSITION_DEPTH {
        return Err(CodecError::PropositionNestingTooDeep);
    }
    match proposition {
        Proposition::Truth | Proposition::Falsehood | Proposition::Atom(_) => Ok(()),
        Proposition::Equal(left, right) => {
            validate_scalar_term_depth(left)?;
            validate_scalar_term_depth(right)?;
            if canonical_scalar_term_bytes(left)? > canonical_scalar_term_bytes(right)? {
                return Err(CodecError::NonCanonicalOrder("equality operands"));
            }
            Ok(())
        }
        Proposition::LessThan(left, right) | Proposition::LessOrEqual(left, right) => {
            validate_scalar_term_depth(left)?;
            validate_scalar_term_depth(right)
        }
        Proposition::Conjunction(conjuncts) => {
            if conjuncts
                .iter()
                .any(|conjunct| matches!(conjunct, Proposition::Conjunction(_)))
            {
                return Err(CodecError::NestedConjunction);
            }
            for conjunct in conjuncts {
                validate_canonical_proposition(conjunct, depth + 1)?;
            }
            if !canonical_propositions_strictly_increase(conjuncts)? {
                return Err(CodecError::NonCanonicalOrder("conjunction propositions"));
            }
            Ok(())
        }
        Proposition::Implication {
            premise,
            conclusion,
        } => {
            validate_canonical_proposition(premise, depth + 1)?;
            validate_canonical_proposition(conclusion, depth + 1)
        }
        Proposition::ContentConservation(conservation) => {
            validate_content_term_depth(conservation.left(), 0)?;
            validate_content_term_depth(conservation.right(), 0)
        }
    }
}

fn canonical_propositions_strictly_increase(
    propositions: &[Proposition],
) -> Result<bool, CodecError> {
    let mut previous = None;
    for proposition in propositions {
        let bytes = canonical_proposition_bytes(proposition)?;
        if previous.as_ref().is_some_and(|previous| previous >= &bytes) {
            return Ok(false);
        }
        previous = Some(bytes);
    }
    Ok(true)
}

fn canonical_proposition_bytes(proposition: &Proposition) -> Result<Vec<u8>, CodecError> {
    let mut writer = Writer::default();
    encode_proposition(&mut writer, proposition, 0)?;
    Ok(writer.finish())
}

fn canonical_scalar_term_bytes(term: &ScalarTerm) -> Result<Vec<u8>, CodecError> {
    let mut writer = Writer::default();
    encode_scalar_term(&mut writer, term, 0)?;
    Ok(writer.finish())
}

fn validate_scalar_term_depth(term: &ScalarTerm) -> Result<(), CodecError> {
    let mut pending = vec![(term, 0_usize)];
    while let Some((term, depth)) = pending.pop() {
        if depth > MAX_SCALAR_TERM_DEPTH {
            return Err(CodecError::ScalarTermNestingTooDeep);
        }
        match term {
            ScalarTerm::BooleanNot { operand }
            | ScalarTerm::IntegerBitwiseNot { operand, .. }
            | ScalarTerm::IntegerWiden { operand, .. }
            | ScalarTerm::IntegerExactCast { operand, .. } => {
                pending.push((operand, depth + 1));
            }
            ScalarTerm::BooleanEqual { left, right }
            | ScalarTerm::IntegerEqual { left, right, .. }
            | ScalarTerm::IntegerLessThan { left, right, .. }
            | ScalarTerm::IntegerLessOrEqual { left, right, .. }
            | ScalarTerm::IntegerBitwiseAnd { left, right, .. }
            | ScalarTerm::IntegerBitwiseOr { left, right, .. }
            | ScalarTerm::IntegerBitwiseXor { left, right, .. }
            | ScalarTerm::ExactIntegerAdd { left, right, .. }
            | ScalarTerm::ExactIntegerSubtract { left, right, .. }
            | ScalarTerm::ExactIntegerMultiply { left, right, .. }
            | ScalarTerm::ExactIntegerDivide { left, right, .. }
            | ScalarTerm::ExactIntegerRemainder { left, right, .. }
            | ScalarTerm::WrappingIntegerDivide { left, right, .. }
            | ScalarTerm::WrappingIntegerRemainder { left, right, .. }
            | ScalarTerm::SaturatingIntegerDivide { left, right, .. }
            | ScalarTerm::SaturatingIntegerRemainder { left, right, .. }
            | ScalarTerm::WrappingIntegerAdd { left, right, .. }
            | ScalarTerm::SaturatingIntegerAdd { left, right, .. }
            | ScalarTerm::WrappingIntegerSubtract { left, right, .. }
            | ScalarTerm::SaturatingIntegerSubtract { left, right, .. }
            | ScalarTerm::WrappingIntegerMultiply { left, right, .. }
            | ScalarTerm::SaturatingIntegerMultiply { left, right, .. } => {
                pending.push((left, depth + 1));
                pending.push((right, depth + 1));
            }
            ScalarTerm::WrappingIntegerShiftLeft { value, count, .. }
            | ScalarTerm::WrappingIntegerShiftRight { value, count, .. }
            | ScalarTerm::ExactIntegerShiftLeft { value, count, .. }
            | ScalarTerm::ExactIntegerShiftRight { value, count, .. } => {
                pending.push((value, depth + 1));
                pending.push((count, depth + 1));
            }
            ScalarTerm::Value { .. } | ScalarTerm::Boolean(_) | ScalarTerm::Integer { .. } => {}
        }
    }
    Ok(())
}

fn validate_content_term_depth(term: &ContentTerm, depth: usize) -> Result<(), CodecError> {
    if depth > MAX_CONTENT_TERM_DEPTH {
        return Err(CodecError::ContentTermNestingTooDeep);
    }
    if let ContentTerm::Separate(terms) = term {
        for term in terms {
            validate_content_term_depth(term, depth + 1)?;
        }
    }
    Ok(())
}

fn strictly_increasing<T: Ord>(values: impl IntoIterator<Item = T>) -> bool {
    let mut previous = None;
    for value in values {
        if previous.as_ref().is_some_and(|previous| previous >= &value) {
            return false;
        }
        previous = Some(value);
    }
    true
}

fn encode_raw(module: &TerminalModule) -> Result<Vec<u8>, CodecError> {
    let mut writer = Writer::default();
    writer.bytes(MAGIC);
    writer.u16(FORMAT_VERSION);
    writer.u16(module.semantic_version.get());
    writer.id(module.entry);
    if module.semantic_version >= SemanticVersion::V16 {
        writer.len(
            "proposition declarations",
            module.proposition_declarations.len(),
        )?;
        for declaration in &module.proposition_declarations {
            encode_proposition_declaration(&mut writer, declaration)?;
        }
        writer.len(
            "proposition applications",
            module.proposition_applications.len(),
        )?;
        for application in &module.proposition_applications {
            encode_proposition_application(&mut writer, application)?;
        }
    }
    writer.len("machines", module.machines.len())?;
    for machine in &module.machines {
        encode_machine(&mut writer, module.semantic_version, machine)?;
    }
    Ok(writer.finish())
}

fn encode_proposition_declaration(
    writer: &mut Writer,
    declaration: &PropositionDeclaration,
) -> Result<(), CodecError> {
    writer.id(declaration.id);
    writer.string("proposition name", &declaration.name)?;
    writer.len("proposition binders", declaration.binders.len())?;
    for binder in &declaration.binders {
        writer.string("proposition binder name", &binder.name)?;
        match &binder.kind {
            PropositionBinderKind::Type => writer.u8(1),
            PropositionBinderKind::Const { type_identity } => {
                writer.u8(2);
                writer.string("proposition const binder type", type_identity)?;
            }
            PropositionBinderKind::Machine => writer.u8(3),
        }
    }
    writer.len(
        "proposition parameter types",
        declaration.parameter_types.len(),
    )?;
    for parameter_type in &declaration.parameter_types {
        writer.string("proposition parameter type", parameter_type)?;
    }
    match &declaration.evidence {
        PropositionEvidence::FactOnly => writer.u8(1),
        PropositionEvidence::Witness { evidence_type } => {
            writer.u8(2);
            writer.string("proposition evidence type", evidence_type)?;
        }
    }
    Ok(())
}

fn encode_proposition_application(
    writer: &mut Writer,
    application: &PropositionApplicationIdentity,
) -> Result<(), CodecError> {
    writer.id(application.id);
    writer.id(application.declaration);
    writer.len(
        "proposition binder arguments",
        application.binder_arguments.len(),
    )?;
    for argument in &application.binder_arguments {
        writer.u8(match argument.kind {
            PropositionBinderArgumentKind::Type => 1,
            PropositionBinderArgumentKind::Const => 2,
            PropositionBinderArgumentKind::Machine => 3,
        });
        writer.string("proposition binder argument", &argument.identity)?;
    }
    writer.len("proposition arguments", application.arguments.len())?;
    for argument in &application.arguments {
        writer.string("proposition argument", argument)?;
    }
    Ok(())
}

fn encode_machine(
    writer: &mut Writer,
    semantic_version: SemanticVersion,
    machine: &TerminalMachine,
) -> Result<(), CodecError> {
    writer.id(machine.id);
    encode_declarations(writer, "machine parameters", &machine.parameters)?;
    encode_declaration(writer, machine.result);
    if semantic_version >= SemanticVersion::V9 {
        writer.len("structural places", machine.structural_places.len())?;
        for place in &machine.structural_places {
            writer.id(place.id);
            encode_structural_place_kind(writer, place.kind);
        }
    }
    if semantic_version >= SemanticVersion::V14 {
        writer.len("content entry claims", machine.content_entry_claims.len())?;
        for binding in &machine.content_entry_claims {
            encode_content_entry_claim(writer, binding)?;
        }
    }
    if semantic_version >= SemanticVersion::V10 {
        writer.len(
            "content identity reshuffles",
            machine.content_identity_reshuffles.len(),
        )?;
        for reshuffle in &machine.content_identity_reshuffles {
            encode_content_identity_reshuffle(writer, reshuffle)?;
        }
    }
    if semantic_version >= SemanticVersion::V12 {
        writer.len(
            "content partition compositions",
            machine.content_partition_compositions.len(),
        )?;
        for composition in &machine.content_partition_compositions {
            encode_content_partition_composition(writer, composition)?;
        }
    }
    writer.id(machine.entry);
    writer.len("blocks", machine.blocks.len())?;
    for block in &machine.blocks {
        encode_block(writer, semantic_version, block)?;
    }
    encode_contract(writer, semantic_version, &machine.contract)
}

fn encode_content_entry_claim(
    writer: &mut Writer,
    binding: &ContentEntryClaim,
) -> Result<(), CodecError> {
    writer.id(binding.claim);
    encode_content_structural_place(writer, &binding.input)?;
    encode_claim_content_projections(writer, &binding.projections)
}

fn encode_content_partition_composition(
    writer: &mut Writer,
    composition: &ContentPartitionComposition,
) -> Result<(), CodecError> {
    writer.u64(composition.source_fingerprint);
    writer.len(
        "partition source structural places",
        composition.source_structural_places.len(),
    )?;
    for place in &composition.source_structural_places {
        writer.id(place.id);
        encode_structural_place_kind(writer, place.kind);
    }
    encode_content_conservation(writer, &composition.source)?;
    writer.len("partition input claims", composition.input_claims.len())?;
    for claim in &composition.input_claims {
        writer.id(*claim);
    }
    writer.len(
        "partition place substitutions",
        composition.substitutions.len(),
    )?;
    for substitution in &composition.substitutions {
        encode_content_structural_place(writer, &substitution.source)?;
        encode_content_structural_place(writer, &substitution.target)?;
    }
    encode_content_conservation(writer, &composition.derived)
}

fn encode_content_conservation(
    writer: &mut Writer,
    conservation: &ContentConservation,
) -> Result<(), CodecError> {
    encode_content_algebra(writer, conservation.algebra())?;
    encode_content_term(writer, conservation.left(), 0)?;
    encode_content_term(writer, conservation.right(), 0)
}

fn encode_content_identity_reshuffle(
    writer: &mut Writer,
    reshuffle: &ContentIdentityReshuffle,
) -> Result<(), CodecError> {
    writer.id(reshuffle.claim);
    encode_content_structural_place(writer, &reshuffle.input)?;
    encode_content_structural_place(writer, &reshuffle.output)?;
    encode_claim_content_projections(writer, &reshuffle.projections)
}

fn encode_claim_content_projections(
    writer: &mut Writer,
    projections: &[ClaimContentProjection],
) -> Result<(), CodecError> {
    writer.len("claim content projections", projections.len())?;
    for content in projections {
        writer.id(content.projection.domain);
        writer.u64(content.projection.projection_fingerprint);
        encode_content_algebra(writer, &content.algebra)?;
    }
    Ok(())
}

fn encode_declarations(
    writer: &mut Writer,
    label: &'static str,
    declarations: &[ValueDeclaration],
) -> Result<(), CodecError> {
    writer.len(label, declarations.len())?;
    for declaration in declarations {
        encode_declaration(writer, *declaration);
    }
    Ok(())
}

fn encode_declaration(writer: &mut Writer, declaration: ValueDeclaration) {
    writer.id(declaration.id);
    encode_scalar_type(writer, declaration.scalar_type);
}

fn encode_block(
    writer: &mut Writer,
    semantic_version: SemanticVersion,
    block: &Block,
) -> Result<(), CodecError> {
    writer.id(block.id);
    encode_declarations(writer, "block parameters", &block.parameters)?;
    writer.len("operations", block.operations.len())?;
    for operation in &block.operations {
        writer.id(operation.id);
        encode_declaration(writer, operation.result);
        match operation.kind {
            OperationKind::IntegerConstant { value } => {
                writer.u8(1);
                encode_integer_value(writer, value);
            }
            OperationKind::BooleanConstant { value } => {
                writer.u8(2);
                writer.u8(u8::from(value));
            }
            OperationKind::BooleanNot { operand } => {
                writer.u8(9);
                writer.id(operand);
            }
            OperationKind::BooleanEqual { left, right } => {
                writer.u8(10);
                writer.id(left);
                writer.id(right);
            }
            OperationKind::IntegerEqual { left, right } => {
                writer.u8(11);
                writer.id(left);
                writer.id(right);
            }
            OperationKind::IntegerLessThan { left, right } => {
                writer.u8(12);
                writer.id(left);
                writer.id(right);
            }
            OperationKind::IntegerLessOrEqual { left, right } => {
                writer.u8(13);
                writer.id(left);
                writer.id(right);
            }
            OperationKind::IntegerBitwiseNot { operand } => {
                writer.u8(19);
                writer.id(operand);
            }
            OperationKind::IntegerWiden { operand } => {
                writer.u8(20);
                writer.id(operand);
            }
            OperationKind::IntegerExactCast {
                operand,
                obligation,
            } => {
                writer.u8(21);
                writer.id(operand);
                writer.id(obligation);
            }
            OperationKind::IntegerBitwiseAnd { left, right } => {
                writer.u8(14);
                writer.id(left);
                writer.id(right);
            }
            OperationKind::IntegerBitwiseOr { left, right } => {
                writer.u8(15);
                writer.id(left);
                writer.id(right);
            }
            OperationKind::IntegerBitwiseXor { left, right } => {
                writer.u8(16);
                writer.id(left);
                writer.id(right);
            }
            OperationKind::WrappingIntegerShiftLeft { value, count } => {
                writer.u8(17);
                writer.id(value);
                writer.id(count);
            }
            OperationKind::WrappingIntegerShiftRight { value, count } => {
                writer.u8(18);
                writer.id(value);
                writer.id(count);
            }
            OperationKind::ExactIntegerShiftLeft {
                value,
                count,
                obligation,
            } => {
                writer.u8(23);
                writer.id(value);
                writer.id(count);
                writer.id(obligation);
            }
            OperationKind::ExactIntegerShiftRight {
                value,
                count,
                obligation,
            } => {
                writer.u8(22);
                writer.id(value);
                writer.id(count);
                writer.id(obligation);
            }
            OperationKind::ExactIntegerAdd {
                left,
                right,
                obligation,
            } => {
                writer.u8(24);
                writer.id(left);
                writer.id(right);
                writer.id(obligation);
            }
            OperationKind::ExactIntegerSubtract {
                left,
                right,
                obligation,
            } => {
                writer.u8(25);
                writer.id(left);
                writer.id(right);
                writer.id(obligation);
            }
            OperationKind::ExactIntegerMultiply {
                left,
                right,
                obligation,
            } => {
                writer.u8(26);
                writer.id(left);
                writer.id(right);
                writer.id(obligation);
            }
            OperationKind::ExactIntegerDivide {
                left,
                right,
                obligation,
            } => {
                writer.u8(27);
                writer.id(left);
                writer.id(right);
                writer.id(obligation);
            }
            OperationKind::ExactIntegerRemainder {
                left,
                right,
                obligation,
            } => {
                writer.u8(28);
                writer.id(left);
                writer.id(right);
                writer.id(obligation);
            }
            OperationKind::WrappingIntegerDivide {
                left,
                right,
                obligation,
            } => {
                writer.u8(29);
                writer.id(left);
                writer.id(right);
                writer.id(obligation);
            }
            OperationKind::WrappingIntegerRemainder {
                left,
                right,
                obligation,
            } => {
                writer.u8(30);
                writer.id(left);
                writer.id(right);
                writer.id(obligation);
            }
            OperationKind::SaturatingIntegerDivide {
                left,
                right,
                obligation,
            } => {
                writer.u8(31);
                writer.id(left);
                writer.id(right);
                writer.id(obligation);
            }
            OperationKind::SaturatingIntegerRemainder {
                left,
                right,
                obligation,
            } => {
                writer.u8(32);
                writer.id(left);
                writer.id(right);
                writer.id(obligation);
            }
            OperationKind::WrappingIntegerAdd { left, right } => {
                writer.u8(3);
                writer.id(left);
                writer.id(right);
            }
            OperationKind::SaturatingIntegerAdd { left, right } => {
                writer.u8(4);
                writer.id(left);
                writer.id(right);
            }
            OperationKind::WrappingIntegerSubtract { left, right } => {
                writer.u8(5);
                writer.id(left);
                writer.id(right);
            }
            OperationKind::SaturatingIntegerSubtract { left, right } => {
                writer.u8(6);
                writer.id(left);
                writer.id(right);
            }
            OperationKind::WrappingIntegerMultiply { left, right } => {
                writer.u8(7);
                writer.id(left);
                writer.id(right);
            }
            OperationKind::SaturatingIntegerMultiply { left, right } => {
                writer.u8(8);
                writer.id(left);
                writer.id(right);
            }
        }
    }
    match &block.terminator {
        Terminator::Jump {
            edge,
            target,
            arguments,
        } => {
            writer.u8(1);
            writer.id(*edge);
            writer.id(*target);
            writer.len("jump arguments", arguments.len())?;
            for argument in arguments {
                writer.id(*argument);
            }
        }
        Terminator::Return { edge, value } => {
            writer.u8(2);
            writer.id(*edge);
            writer.id(*value);
        }
        Terminator::Conditional {
            condition,
            when_true,
            when_false,
        } => {
            writer.u8(3);
            writer.id(*condition);
            encode_successor_edge(writer, when_true)?;
            encode_successor_edge(writer, when_false)?;
        }
        Terminator::Crash {
            edge,
            cause,
            damage_minimum,
            containment_demand,
            frontier_lower_bound,
        } => {
            writer.u8(4);
            writer.id(*edge);
            writer.u8(match cause {
                CrashCause::Trap => 1,
                CrashCause::Abort => 2,
            });
            if semantic_version >= SemanticVersion::V23 {
                writer.string("crash damage minimum", damage_minimum)?;
            }
            writer.string("crash containment demand", containment_demand)?;
            writer.len("crash frontier lower bound", frontier_lower_bound.len())?;
            for claim in frontier_lower_bound {
                writer.id(*claim);
            }
        }
    }
    Ok(())
}

fn encode_successor_edge(writer: &mut Writer, successor: &SuccessorEdge) -> Result<(), CodecError> {
    writer.id(successor.edge);
    writer.id(successor.target);
    writer.len("conditional successor arguments", successor.arguments.len())?;
    for argument in &successor.arguments {
        writer.id(*argument);
    }
    Ok(())
}

fn encode_contract(
    writer: &mut Writer,
    semantic_version: SemanticVersion,
    contract: &MachineContract,
) -> Result<(), CodecError> {
    writer.id(contract.id);
    if semantic_version >= SemanticVersion::V24 {
        writer.len("crash context maxima", contract.crash_context.len())?;
        for maximum in &contract.crash_context {
            writer.u8(match maximum.cause {
                CrashCause::Trap => 1,
                CrashCause::Abort => 2,
            });
            writer.string("crash context maximum", &maximum.maximum_scope)?;
        }
    }
    writer.len("requires", contract.requires.len())?;
    for proposition in &contract.requires {
        encode_proposition(writer, proposition, 0)?;
    }
    writer.len("ensures", contract.ensures.len())?;
    for clause in &contract.ensures {
        writer.id(clause.obligation);
        encode_proposition(writer, &clause.proposition, 0)?;
    }
    Ok(())
}

fn encode_proposition(
    writer: &mut Writer,
    proposition: &Proposition,
    depth: usize,
) -> Result<(), CodecError> {
    if depth > MAX_PROPOSITION_DEPTH {
        return Err(CodecError::PropositionNestingTooDeep);
    }
    match proposition {
        Proposition::Truth => writer.u8(1),
        Proposition::Falsehood => writer.u8(2),
        Proposition::Atom(id) => {
            writer.u8(3);
            writer.id(*id);
        }
        Proposition::Equal(left, right) => {
            writer.u8(4);
            encode_scalar_term(writer, left, 0)?;
            encode_scalar_term(writer, right, 0)?;
        }
        Proposition::LessThan(left, right) => {
            writer.u8(5);
            encode_scalar_term(writer, left, 0)?;
            encode_scalar_term(writer, right, 0)?;
        }
        Proposition::LessOrEqual(left, right) => {
            writer.u8(6);
            encode_scalar_term(writer, left, 0)?;
            encode_scalar_term(writer, right, 0)?;
        }
        Proposition::Conjunction(conjuncts) => {
            writer.u8(7);
            writer.len("conjuncts", conjuncts.len())?;
            for conjunct in conjuncts {
                encode_proposition(writer, conjunct, depth + 1)?;
            }
        }
        Proposition::Implication {
            premise,
            conclusion,
        } => {
            writer.u8(8);
            encode_proposition(writer, premise, depth + 1)?;
            encode_proposition(writer, conclusion, depth + 1)?;
        }
        Proposition::ContentConservation(conservation) => {
            writer.u8(9);
            encode_content_algebra(writer, conservation.algebra())?;
            encode_content_term(writer, conservation.left(), 0)?;
            encode_content_term(writer, conservation.right(), 0)?;
        }
    }
    Ok(())
}

fn encode_structural_place_kind(writer: &mut Writer, kind: StructuralPlaceKind) {
    match kind {
        StructuralPlaceKind::Parameter { position, is_self } => {
            writer.u8(1);
            writer.u32(position);
            writer.u8(u8::from(is_self));
        }
        StructuralPlaceKind::Result => writer.u8(2),
    }
}

fn encode_content_algebra(writer: &mut Writer, algebra: &ContentAlgebra) -> Result<(), CodecError> {
    writer.u8(match algebra.kind {
        ContentAlgebraKind::IntervalSet => 1,
        ContentAlgebraKind::CountedQuantity => 2,
    });
    writer.string("content algebra parameter", &algebra.parameter)
}

fn encode_content_term(
    writer: &mut Writer,
    term: &ContentTerm,
    depth: usize,
) -> Result<(), CodecError> {
    if depth > MAX_CONTENT_TERM_DEPTH {
        return Err(CodecError::ContentTermNestingTooDeep);
    }
    match term {
        ContentTerm::Projection {
            projection,
            subject,
        } => {
            writer.u8(1);
            writer.id(projection.domain);
            writer.u64(projection.projection_fingerprint);
            encode_content_structural_place(writer, subject)?;
        }
        ContentTerm::Separate(terms) => {
            writer.u8(2);
            writer.len("separated content terms", terms.len())?;
            for term in terms {
                encode_content_term(writer, term, depth + 1)?;
            }
        }
    }
    Ok(())
}

fn encode_content_structural_place(
    writer: &mut Writer,
    subject: &ContentStructuralPlace,
) -> Result<(), CodecError> {
    writer.u8(match subject.version {
        ContentPlaceVersion::Entry => 1,
        ContentPlaceVersion::Current => 2,
    });
    writer.id(subject.root);
    writer.len("content place segments", subject.segments.len())?;
    for segment in &subject.segments {
        match segment {
            ContentPlaceSegment::Case(name) => {
                writer.u8(3);
                writer.string("content case", name)?;
            }
            ContentPlaceSegment::Field(name) => {
                writer.u8(1);
                writer.string("content field", name)?;
            }
            ContentPlaceSegment::FixedIndex(index) => {
                writer.u8(2);
                writer.u64(*index);
            }
        }
    }
    Ok(())
}

fn encode_scalar_term(
    writer: &mut Writer,
    term: &ScalarTerm,
    depth: usize,
) -> Result<(), CodecError> {
    if depth > MAX_SCALAR_TERM_DEPTH {
        return Err(CodecError::ScalarTermNestingTooDeep);
    }
    match term {
        ScalarTerm::Value { id, scalar_type } => {
            writer.u8(1);
            writer.id(*id);
            encode_scalar_type(writer, *scalar_type);
        }
        ScalarTerm::Boolean(value) => {
            writer.u8(2);
            writer.u8(u8::from(*value));
        }
        ScalarTerm::BooleanNot { operand } => {
            writer.u8(10);
            encode_scalar_term(writer, operand, depth + 1)?;
        }
        ScalarTerm::BooleanEqual { left, right } => {
            writer.u8(11);
            encode_scalar_term(writer, left, depth + 1)?;
            encode_scalar_term(writer, right, depth + 1)?;
        }
        ScalarTerm::IntegerEqual {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(12);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1)?;
            encode_scalar_term(writer, right, depth + 1)?;
        }
        ScalarTerm::IntegerLessThan {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(13);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1)?;
            encode_scalar_term(writer, right, depth + 1)?;
        }
        ScalarTerm::IntegerLessOrEqual {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(14);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1)?;
            encode_scalar_term(writer, right, depth + 1)?;
        }
        ScalarTerm::IntegerBitwiseNot {
            scalar_type,
            operand,
        } => {
            writer.u8(20);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, operand, depth + 1)?;
        }
        ScalarTerm::IntegerWiden {
            source_type,
            target_type,
            operand,
        } => {
            writer.u8(21);
            encode_integer_type(writer, *source_type);
            encode_integer_type(writer, *target_type);
            encode_scalar_term(writer, operand, depth + 1)?;
        }
        ScalarTerm::IntegerExactCast {
            source_type,
            target_type,
            operand,
        } => {
            writer.u8(22);
            encode_integer_type(writer, *source_type);
            encode_integer_type(writer, *target_type);
            encode_scalar_term(writer, operand, depth + 1)?;
        }
        ScalarTerm::IntegerBitwiseAnd {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(15);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1)?;
            encode_scalar_term(writer, right, depth + 1)?;
        }
        ScalarTerm::IntegerBitwiseOr {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(16);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1)?;
            encode_scalar_term(writer, right, depth + 1)?;
        }
        ScalarTerm::IntegerBitwiseXor {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(17);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1)?;
            encode_scalar_term(writer, right, depth + 1)?;
        }
        ScalarTerm::WrappingIntegerShiftLeft {
            value_type,
            count_type,
            value,
            count,
        } => {
            writer.u8(18);
            encode_integer_type(writer, *value_type);
            encode_integer_type(writer, *count_type);
            encode_scalar_term(writer, value, depth + 1)?;
            encode_scalar_term(writer, count, depth + 1)?;
        }
        ScalarTerm::WrappingIntegerShiftRight {
            value_type,
            count_type,
            value,
            count,
        } => {
            writer.u8(19);
            encode_integer_type(writer, *value_type);
            encode_integer_type(writer, *count_type);
            encode_scalar_term(writer, value, depth + 1)?;
            encode_scalar_term(writer, count, depth + 1)?;
        }
        ScalarTerm::ExactIntegerShiftRight {
            value_type,
            count_type,
            value,
            count,
        } => {
            writer.u8(23);
            encode_integer_type(writer, *value_type);
            encode_integer_type(writer, *count_type);
            encode_scalar_term(writer, value, depth + 1)?;
            encode_scalar_term(writer, count, depth + 1)?;
        }
        ScalarTerm::ExactIntegerShiftLeft {
            value_type,
            count_type,
            value,
            count,
        } => {
            writer.u8(24);
            encode_integer_type(writer, *value_type);
            encode_integer_type(writer, *count_type);
            encode_scalar_term(writer, value, depth + 1)?;
            encode_scalar_term(writer, count, depth + 1)?;
        }
        ScalarTerm::ExactIntegerAdd {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(25);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1)?;
            encode_scalar_term(writer, right, depth + 1)?;
        }
        ScalarTerm::ExactIntegerSubtract {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(26);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1)?;
            encode_scalar_term(writer, right, depth + 1)?;
        }
        ScalarTerm::ExactIntegerMultiply {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(27);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1)?;
            encode_scalar_term(writer, right, depth + 1)?;
        }
        ScalarTerm::ExactIntegerDivide {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(28);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1)?;
            encode_scalar_term(writer, right, depth + 1)?;
        }
        ScalarTerm::ExactIntegerRemainder {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(29);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1)?;
            encode_scalar_term(writer, right, depth + 1)?;
        }
        ScalarTerm::WrappingIntegerDivide {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(30);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1)?;
            encode_scalar_term(writer, right, depth + 1)?;
        }
        ScalarTerm::WrappingIntegerRemainder {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(31);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1)?;
            encode_scalar_term(writer, right, depth + 1)?;
        }
        ScalarTerm::SaturatingIntegerDivide {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(32);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1)?;
            encode_scalar_term(writer, right, depth + 1)?;
        }
        ScalarTerm::SaturatingIntegerRemainder {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(33);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1)?;
            encode_scalar_term(writer, right, depth + 1)?;
        }
        ScalarTerm::Integer { scalar_type, value } => {
            writer.u8(3);
            encode_integer_type(writer, *scalar_type);
            encode_integer_value(writer, *value);
        }
        ScalarTerm::WrappingIntegerAdd {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(4);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1)?;
            encode_scalar_term(writer, right, depth + 1)?;
        }
        ScalarTerm::SaturatingIntegerAdd {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(5);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1)?;
            encode_scalar_term(writer, right, depth + 1)?;
        }
        ScalarTerm::WrappingIntegerSubtract {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(6);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1)?;
            encode_scalar_term(writer, right, depth + 1)?;
        }
        ScalarTerm::SaturatingIntegerSubtract {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(7);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1)?;
            encode_scalar_term(writer, right, depth + 1)?;
        }
        ScalarTerm::WrappingIntegerMultiply {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(8);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1)?;
            encode_scalar_term(writer, right, depth + 1)?;
        }
        ScalarTerm::SaturatingIntegerMultiply {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(9);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1)?;
            encode_scalar_term(writer, right, depth + 1)?;
        }
    }
    Ok(())
}

fn encode_scalar_type(writer: &mut Writer, scalar_type: ScalarType) {
    match scalar_type {
        ScalarType::Boolean => writer.u8(1),
        ScalarType::Integer(integer_type) => {
            writer.u8(2);
            encode_integer_type(writer, integer_type);
        }
    }
}

fn encode_integer_type(writer: &mut Writer, integer_type: IntegerType) {
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

fn encode_integer_value(writer: &mut Writer, value: IntegerValue) {
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

fn decode_module_body(reader: &mut Reader<'_>) -> Result<TerminalModule, CodecError> {
    let semantic_version_raw = reader.u16()?;
    let semantic_version =
        SemanticVersion::new(semantic_version_raw).ok_or(CodecError::ZeroSemanticVersion)?;
    let entry = reader.id("MachineId")?;
    let proposition_declarations = if semantic_version >= SemanticVersion::V16 {
        let count = reader.count()?;
        let mut declarations = Vec::with_capacity(count as usize);
        for _ in 0..count {
            declarations.push(decode_proposition_declaration(reader)?);
        }
        declarations
    } else {
        Vec::new()
    };
    let proposition_applications = if semantic_version >= SemanticVersion::V16 {
        let count = reader.count()?;
        let mut applications = Vec::with_capacity(count as usize);
        for _ in 0..count {
            applications.push(decode_proposition_application(reader)?);
        }
        applications
    } else {
        Vec::new()
    };
    let machine_count = reader.count()?;
    let mut machines = Vec::new();
    for _ in 0..machine_count {
        machines.push(decode_machine(reader, semantic_version)?);
    }
    Ok(TerminalModule {
        semantic_version,
        entry,
        proposition_declarations,
        proposition_applications,
        machines,
    })
}

fn decode_proposition_declaration(
    reader: &mut Reader<'_>,
) -> Result<PropositionDeclaration, CodecError> {
    let id = reader.id("PropositionId")?;
    let name = reader.string("proposition name")?;
    let binder_count = reader.count()?;
    let mut binders = Vec::with_capacity(binder_count as usize);
    for _ in 0..binder_count {
        let name = reader.string("proposition binder name")?;
        let kind = match reader.u8()? {
            1 => PropositionBinderKind::Type,
            2 => PropositionBinderKind::Const {
                type_identity: reader.string("proposition const binder type")?,
            },
            3 => PropositionBinderKind::Machine,
            tag => return Err(CodecError::InvalidTag("PropositionBinderKind", tag)),
        };
        binders.push(PropositionBinderDeclaration { name, kind });
    }
    let parameter_count = reader.count()?;
    let mut parameter_types = Vec::with_capacity(parameter_count as usize);
    for _ in 0..parameter_count {
        parameter_types.push(reader.string("proposition parameter type")?);
    }
    let evidence = match reader.u8()? {
        1 => PropositionEvidence::FactOnly,
        2 => PropositionEvidence::Witness {
            evidence_type: reader.string("proposition evidence type")?,
        },
        tag => return Err(CodecError::InvalidTag("PropositionEvidence", tag)),
    };
    Ok(PropositionDeclaration {
        id,
        name,
        binders,
        parameter_types,
        evidence,
    })
}

fn decode_proposition_application(
    reader: &mut Reader<'_>,
) -> Result<PropositionApplicationIdentity, CodecError> {
    let id = reader.id("PropositionId")?;
    let declaration = reader.id("PropositionId")?;
    let binder_count = reader.count()?;
    let mut binder_arguments = Vec::with_capacity(binder_count as usize);
    for _ in 0..binder_count {
        let kind = match reader.u8()? {
            1 => PropositionBinderArgumentKind::Type,
            2 => PropositionBinderArgumentKind::Const,
            3 => PropositionBinderArgumentKind::Machine,
            tag => {
                return Err(CodecError::InvalidTag("PropositionBinderArgumentKind", tag));
            }
        };
        binder_arguments.push(PropositionBinderArgumentIdentity {
            kind,
            identity: reader.string("proposition binder argument")?,
        });
    }
    let argument_count = reader.count()?;
    let mut arguments = Vec::with_capacity(argument_count as usize);
    for _ in 0..argument_count {
        arguments.push(reader.string("proposition argument")?);
    }
    Ok(PropositionApplicationIdentity {
        id,
        declaration,
        binder_arguments,
        arguments,
    })
}

fn decode_machine(
    reader: &mut Reader<'_>,
    semantic_version: SemanticVersion,
) -> Result<TerminalMachine, CodecError> {
    let id = reader.id("MachineId")?;
    let parameters = decode_declarations(reader)?;
    let result = decode_declaration(reader)?;
    let structural_places = if semantic_version >= SemanticVersion::V9 {
        let count = reader.count()?;
        let mut places = Vec::new();
        for _ in 0..count {
            places.push(StructuralPlaceDeclaration {
                id: reader.id("PlaceId")?,
                kind: decode_structural_place_kind(reader)?,
            });
        }
        places
    } else {
        Vec::new()
    };
    let content_entry_claims = if semantic_version >= SemanticVersion::V14 {
        let count = reader.count()?;
        let mut bindings = Vec::new();
        for _ in 0..count {
            bindings.push(decode_content_entry_claim(reader)?);
        }
        bindings
    } else {
        Vec::new()
    };
    let content_identity_reshuffles = if semantic_version >= SemanticVersion::V10 {
        let count = reader.count()?;
        let mut reshuffles = Vec::new();
        for _ in 0..count {
            reshuffles.push(decode_content_identity_reshuffle(reader)?);
        }
        reshuffles
    } else {
        Vec::new()
    };
    let content_partition_compositions = if semantic_version >= SemanticVersion::V12 {
        let count = reader.count()?;
        let mut compositions = Vec::new();
        for _ in 0..count {
            compositions.push(decode_content_partition_composition(reader)?);
        }
        compositions
    } else {
        Vec::new()
    };
    let entry = reader.id("BlockId")?;
    let block_count = reader.count()?;
    let mut blocks = Vec::new();
    for _ in 0..block_count {
        blocks.push(decode_block(reader, semantic_version)?);
    }
    let contract = decode_contract(reader, semantic_version)?;
    Ok(TerminalMachine {
        id,
        parameters,
        result,
        structural_places,
        content_entry_claims,
        content_identity_reshuffles,
        content_partition_compositions,
        entry,
        blocks,
        contract,
    })
}

fn decode_content_entry_claim(reader: &mut Reader<'_>) -> Result<ContentEntryClaim, CodecError> {
    Ok(ContentEntryClaim {
        claim: reader.id("ClaimId")?,
        input: decode_content_structural_place(reader)?,
        projections: decode_claim_content_projections(reader)?,
    })
}

fn decode_content_partition_composition(
    reader: &mut Reader<'_>,
) -> Result<ContentPartitionComposition, CodecError> {
    let source_fingerprint = reader.u64()?;
    let source_place_count = reader.count()?;
    let mut source_structural_places = Vec::new();
    for _ in 0..source_place_count {
        source_structural_places.push(StructuralPlaceDeclaration {
            id: reader.id("PlaceId")?,
            kind: decode_structural_place_kind(reader)?,
        });
    }
    let source = decode_content_conservation(reader)?;
    let input_claim_count = reader.count()?;
    let mut input_claims = Vec::new();
    for _ in 0..input_claim_count {
        input_claims.push(reader.id("ClaimId")?);
    }
    let substitution_count = reader.count()?;
    let mut substitutions = Vec::new();
    for _ in 0..substitution_count {
        substitutions.push(ContentPlaceSubstitution {
            source: decode_content_structural_place(reader)?,
            target: decode_content_structural_place(reader)?,
        });
    }
    let derived = decode_content_conservation(reader)?;
    Ok(ContentPartitionComposition {
        source_fingerprint,
        source_structural_places,
        source,
        input_claims,
        substitutions,
        derived,
    })
}

fn decode_content_conservation(reader: &mut Reader<'_>) -> Result<ContentConservation, CodecError> {
    Ok(ContentConservation::new(
        decode_content_algebra(reader)?,
        decode_content_term(reader, 0)?,
        decode_content_term(reader, 0)?,
    ))
}

fn decode_content_identity_reshuffle(
    reader: &mut Reader<'_>,
) -> Result<ContentIdentityReshuffle, CodecError> {
    let claim = reader.id::<ClaimId>("ClaimId")?;
    let input = decode_content_structural_place(reader)?;
    let output = decode_content_structural_place(reader)?;
    let projections = decode_claim_content_projections(reader)?;
    Ok(ContentIdentityReshuffle {
        claim,
        input,
        output,
        projections,
    })
}

fn decode_claim_content_projections(
    reader: &mut Reader<'_>,
) -> Result<Vec<ClaimContentProjection>, CodecError> {
    let count = reader.count()?;
    let mut projections = Vec::new();
    for _ in 0..count {
        projections.push(ClaimContentProjection {
            projection: ContentProjectionIdentity {
                domain: reader.id("ContentDomainId")?,
                projection_fingerprint: reader.u64()?,
            },
            algebra: decode_content_algebra(reader)?,
        });
    }
    Ok(projections)
}

fn decode_declarations(reader: &mut Reader<'_>) -> Result<Vec<ValueDeclaration>, CodecError> {
    let count = reader.count()?;
    let mut declarations = Vec::new();
    for _ in 0..count {
        declarations.push(decode_declaration(reader)?);
    }
    Ok(declarations)
}

fn decode_declaration(reader: &mut Reader<'_>) -> Result<ValueDeclaration, CodecError> {
    Ok(ValueDeclaration {
        id: reader.id("ValueId")?,
        scalar_type: decode_scalar_type(reader)?,
    })
}

fn decode_block(
    reader: &mut Reader<'_>,
    semantic_version: SemanticVersion,
) -> Result<Block, CodecError> {
    let id = reader.id("BlockId")?;
    let parameters = decode_declarations(reader)?;
    let operation_count = reader.count()?;
    let mut operations = Vec::new();
    for _ in 0..operation_count {
        let operation_id = reader.id("OperationId")?;
        let result = decode_declaration(reader)?;
        let kind = match reader.u8()? {
            1 => OperationKind::IntegerConstant {
                value: decode_integer_value(reader)?,
            },
            2 => OperationKind::BooleanConstant {
                value: reader.boolean()?,
            },
            3 => OperationKind::WrappingIntegerAdd {
                left: reader.id("ValueId")?,
                right: reader.id("ValueId")?,
            },
            4 => OperationKind::SaturatingIntegerAdd {
                left: reader.id("ValueId")?,
                right: reader.id("ValueId")?,
            },
            5 => OperationKind::WrappingIntegerSubtract {
                left: reader.id("ValueId")?,
                right: reader.id("ValueId")?,
            },
            6 => OperationKind::SaturatingIntegerSubtract {
                left: reader.id("ValueId")?,
                right: reader.id("ValueId")?,
            },
            7 => OperationKind::WrappingIntegerMultiply {
                left: reader.id("ValueId")?,
                right: reader.id("ValueId")?,
            },
            8 => OperationKind::SaturatingIntegerMultiply {
                left: reader.id("ValueId")?,
                right: reader.id("ValueId")?,
            },
            9 => OperationKind::BooleanNot {
                operand: reader.id("ValueId")?,
            },
            10 => OperationKind::BooleanEqual {
                left: reader.id("ValueId")?,
                right: reader.id("ValueId")?,
            },
            11 => OperationKind::IntegerEqual {
                left: reader.id("ValueId")?,
                right: reader.id("ValueId")?,
            },
            12 => OperationKind::IntegerLessThan {
                left: reader.id("ValueId")?,
                right: reader.id("ValueId")?,
            },
            13 => OperationKind::IntegerLessOrEqual {
                left: reader.id("ValueId")?,
                right: reader.id("ValueId")?,
            },
            14 => OperationKind::IntegerBitwiseAnd {
                left: reader.id("ValueId")?,
                right: reader.id("ValueId")?,
            },
            15 => OperationKind::IntegerBitwiseOr {
                left: reader.id("ValueId")?,
                right: reader.id("ValueId")?,
            },
            16 => OperationKind::IntegerBitwiseXor {
                left: reader.id("ValueId")?,
                right: reader.id("ValueId")?,
            },
            17 => OperationKind::WrappingIntegerShiftLeft {
                value: reader.id("ValueId")?,
                count: reader.id("ValueId")?,
            },
            18 => OperationKind::WrappingIntegerShiftRight {
                value: reader.id("ValueId")?,
                count: reader.id("ValueId")?,
            },
            19 => OperationKind::IntegerBitwiseNot {
                operand: reader.id("ValueId")?,
            },
            20 => OperationKind::IntegerWiden {
                operand: reader.id("ValueId")?,
            },
            21 => OperationKind::IntegerExactCast {
                operand: reader.id("ValueId")?,
                obligation: reader.id("ObligationId")?,
            },
            22 => OperationKind::ExactIntegerShiftRight {
                value: reader.id("ValueId")?,
                count: reader.id("ValueId")?,
                obligation: reader.id("ObligationId")?,
            },
            23 => OperationKind::ExactIntegerShiftLeft {
                value: reader.id("ValueId")?,
                count: reader.id("ValueId")?,
                obligation: reader.id("ObligationId")?,
            },
            24 => OperationKind::ExactIntegerAdd {
                left: reader.id("ValueId")?,
                right: reader.id("ValueId")?,
                obligation: reader.id("ObligationId")?,
            },
            25 => OperationKind::ExactIntegerSubtract {
                left: reader.id("ValueId")?,
                right: reader.id("ValueId")?,
                obligation: reader.id("ObligationId")?,
            },
            26 => OperationKind::ExactIntegerMultiply {
                left: reader.id("ValueId")?,
                right: reader.id("ValueId")?,
                obligation: reader.id("ObligationId")?,
            },
            27 => OperationKind::ExactIntegerDivide {
                left: reader.id("ValueId")?,
                right: reader.id("ValueId")?,
                obligation: reader.id("ObligationId")?,
            },
            28 => OperationKind::ExactIntegerRemainder {
                left: reader.id("ValueId")?,
                right: reader.id("ValueId")?,
                obligation: reader.id("ObligationId")?,
            },
            29 => OperationKind::WrappingIntegerDivide {
                left: reader.id("ValueId")?,
                right: reader.id("ValueId")?,
                obligation: reader.id("ObligationId")?,
            },
            30 => OperationKind::WrappingIntegerRemainder {
                left: reader.id("ValueId")?,
                right: reader.id("ValueId")?,
                obligation: reader.id("ObligationId")?,
            },
            31 => OperationKind::SaturatingIntegerDivide {
                left: reader.id("ValueId")?,
                right: reader.id("ValueId")?,
                obligation: reader.id("ObligationId")?,
            },
            32 => OperationKind::SaturatingIntegerRemainder {
                left: reader.id("ValueId")?,
                right: reader.id("ValueId")?,
                obligation: reader.id("ObligationId")?,
            },
            tag => return Err(CodecError::InvalidTag("OperationKind", tag)),
        };
        operations.push(Operation {
            id: operation_id,
            result,
            kind,
        });
    }
    let terminator = match reader.u8()? {
        1 => {
            let edge = reader.id("EdgeId")?;
            let target = reader.id("BlockId")?;
            let argument_count = reader.count()?;
            let mut arguments = Vec::new();
            for _ in 0..argument_count {
                arguments.push(reader.id("ValueId")?);
            }
            Terminator::Jump {
                edge,
                target,
                arguments,
            }
        }
        2 => Terminator::Return {
            edge: reader.id("EdgeId")?,
            value: reader.id("ValueId")?,
        },
        3 => Terminator::Conditional {
            condition: reader.id("ValueId")?,
            when_true: decode_successor_edge(reader)?,
            when_false: decode_successor_edge(reader)?,
        },
        4 => {
            let edge = reader.id("EdgeId")?;
            let cause = match reader.u8()? {
                1 => CrashCause::Trap,
                2 => CrashCause::Abort,
                tag => return Err(CodecError::InvalidTag("CrashCause", tag)),
            };
            let damage_minimum = if semantic_version >= SemanticVersion::V23 {
                reader.string("crash damage minimum")?
            } else {
                reader.string("crash damage scope")?
            };
            let containment_demand = if semantic_version >= SemanticVersion::V23 {
                reader.string("crash containment demand")?
            } else {
                damage_minimum.clone()
            };
            let claim_count = reader.count()?;
            let mut frontier_lower_bound = Vec::with_capacity(claim_count as usize);
            for _ in 0..claim_count {
                frontier_lower_bound.push(reader.id("ClaimId")?);
            }
            Terminator::Crash {
                edge,
                cause,
                damage_minimum,
                containment_demand,
                frontier_lower_bound,
            }
        }
        tag => return Err(CodecError::InvalidTag("Terminator", tag)),
    };
    Ok(Block {
        id,
        parameters,
        operations,
        terminator,
    })
}

fn decode_successor_edge(reader: &mut Reader<'_>) -> Result<SuccessorEdge, CodecError> {
    let edge = reader.id("EdgeId")?;
    let target = reader.id("BlockId")?;
    let argument_count = reader.count()?;
    let mut arguments = Vec::new();
    for _ in 0..argument_count {
        arguments.push(reader.id("ValueId")?);
    }
    Ok(SuccessorEdge {
        edge,
        target,
        arguments,
    })
}

fn decode_contract(
    reader: &mut Reader<'_>,
    semantic_version: SemanticVersion,
) -> Result<MachineContract, CodecError> {
    let id = reader.id("ContractId")?;
    let crash_context = if semantic_version >= SemanticVersion::V24 {
        let count = reader.count()?;
        let mut maxima = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let cause = match reader.u8()? {
                1 => CrashCause::Trap,
                2 => CrashCause::Abort,
                tag => return Err(CodecError::InvalidTag("CrashCause", tag)),
            };
            maxima.push(CrashContextMaximum {
                cause,
                maximum_scope: reader.string("crash context maximum")?,
            });
        }
        maxima
    } else {
        Vec::new()
    };
    let requires_count = reader.count()?;
    let mut requires = Vec::new();
    for _ in 0..requires_count {
        requires.push(decode_proposition(reader, 0)?);
    }
    let ensures_count = reader.count()?;
    let mut ensures = Vec::new();
    for _ in 0..ensures_count {
        ensures.push(ContractClause {
            obligation: reader.id("ObligationId")?,
            proposition: decode_proposition(reader, 0)?,
        });
    }
    Ok(MachineContract {
        id,
        crash_context,
        requires,
        ensures,
    })
}

fn decode_proposition(reader: &mut Reader<'_>, depth: usize) -> Result<Proposition, CodecError> {
    if depth > MAX_PROPOSITION_DEPTH {
        return Err(CodecError::PropositionNestingTooDeep);
    }
    Ok(match reader.u8()? {
        1 => Proposition::Truth,
        2 => Proposition::Falsehood,
        3 => Proposition::Atom(reader.id::<PropositionId>("PropositionId")?),
        4 => Proposition::Equal(
            decode_scalar_term(reader, 0)?,
            decode_scalar_term(reader, 0)?,
        ),
        5 => Proposition::LessThan(
            decode_scalar_term(reader, 0)?,
            decode_scalar_term(reader, 0)?,
        ),
        6 => Proposition::LessOrEqual(
            decode_scalar_term(reader, 0)?,
            decode_scalar_term(reader, 0)?,
        ),
        7 => {
            let count = reader.count()?;
            let mut conjuncts = Vec::new();
            for _ in 0..count {
                conjuncts.push(decode_proposition(reader, depth + 1)?);
            }
            Proposition::Conjunction(conjuncts)
        }
        8 => Proposition::Implication {
            premise: Box::new(decode_proposition(reader, depth + 1)?),
            conclusion: Box::new(decode_proposition(reader, depth + 1)?),
        },
        9 => {
            let algebra = decode_content_algebra(reader)?;
            let left = decode_content_term(reader, 0)?;
            let right = decode_content_term(reader, 0)?;
            Proposition::ContentConservation(ContentConservation::new(algebra, left, right))
        }
        tag => return Err(CodecError::InvalidTag("Proposition", tag)),
    })
}

fn decode_structural_place_kind(
    reader: &mut Reader<'_>,
) -> Result<StructuralPlaceKind, CodecError> {
    Ok(match reader.u8()? {
        1 => StructuralPlaceKind::Parameter {
            position: reader.u32()?,
            is_self: reader.boolean()?,
        },
        2 => StructuralPlaceKind::Result,
        tag => return Err(CodecError::InvalidTag("StructuralPlaceKind", tag)),
    })
}

fn decode_content_algebra(reader: &mut Reader<'_>) -> Result<ContentAlgebra, CodecError> {
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

fn decode_content_term(reader: &mut Reader<'_>, depth: usize) -> Result<ContentTerm, CodecError> {
    if depth > MAX_CONTENT_TERM_DEPTH {
        return Err(CodecError::ContentTermNestingTooDeep);
    }
    Ok(match reader.u8()? {
        1 => {
            let projection = ContentProjectionIdentity {
                domain: reader.id::<ContentDomainId>("ContentDomainId")?,
                projection_fingerprint: reader.u64()?,
            };
            ContentTerm::Projection {
                projection,
                subject: decode_content_structural_place(reader)?,
            }
        }
        2 => {
            let count = reader.count()?;
            let mut terms = Vec::new();
            for _ in 0..count {
                terms.push(decode_content_term(reader, depth + 1)?);
            }
            ContentTerm::separate(terms).map_err(CodecError::MalformedProposition)?
        }
        tag => return Err(CodecError::InvalidTag("ContentTerm", tag)),
    })
}

fn decode_content_structural_place(
    reader: &mut Reader<'_>,
) -> Result<ContentStructuralPlace, CodecError> {
    let version = match reader.u8()? {
        1 => ContentPlaceVersion::Entry,
        2 => ContentPlaceVersion::Current,
        tag => return Err(CodecError::InvalidTag("ContentPlaceVersion", tag)),
    };
    let root = reader.id("PlaceId")?;
    let count = reader.count()?;
    let mut segments = Vec::new();
    for _ in 0..count {
        segments.push(match reader.u8()? {
            1 => ContentPlaceSegment::Field(reader.string("content field")?),
            2 => ContentPlaceSegment::FixedIndex(reader.u64()?),
            3 => ContentPlaceSegment::Case(reader.string("content case")?),
            tag => return Err(CodecError::InvalidTag("ContentPlaceSegment", tag)),
        });
    }
    Ok(ContentStructuralPlace {
        version,
        root,
        segments,
    })
}

fn decode_scalar_term(reader: &mut Reader<'_>, depth: usize) -> Result<ScalarTerm, CodecError> {
    if depth > MAX_SCALAR_TERM_DEPTH {
        return Err(CodecError::ScalarTermNestingTooDeep);
    }
    Ok(match reader.u8()? {
        1 => ScalarTerm::value(reader.id("ValueId")?, decode_scalar_type(reader)?),
        2 => ScalarTerm::boolean(reader.boolean()?),
        3 => {
            let scalar_type = decode_integer_type(reader)?;
            let value = decode_integer_value(reader)?;
            ScalarTerm::integer(scalar_type, value).map_err(CodecError::MalformedProposition)?
        }
        4 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1)?;
            let right = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::wrapping_integer_add(scalar_type, left, right)
                .map_err(CodecError::MalformedProposition)?
        }
        5 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1)?;
            let right = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::saturating_integer_add(scalar_type, left, right)
                .map_err(CodecError::MalformedProposition)?
        }
        6 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1)?;
            let right = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::wrapping_integer_subtract(scalar_type, left, right)
                .map_err(CodecError::MalformedProposition)?
        }
        7 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1)?;
            let right = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::saturating_integer_subtract(scalar_type, left, right)
                .map_err(CodecError::MalformedProposition)?
        }
        8 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1)?;
            let right = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::wrapping_integer_multiply(scalar_type, left, right)
                .map_err(CodecError::MalformedProposition)?
        }
        9 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1)?;
            let right = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::saturating_integer_multiply(scalar_type, left, right)
                .map_err(CodecError::MalformedProposition)?
        }
        10 => ScalarTerm::boolean_not(decode_scalar_term(reader, depth + 1)?)
            .map_err(CodecError::MalformedProposition)?,
        11 => {
            let left = decode_scalar_term(reader, depth + 1)?;
            let right = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::boolean_equal(left, right).map_err(CodecError::MalformedProposition)?
        }
        12 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1)?;
            let right = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::integer_equal(scalar_type, left, right)
                .map_err(CodecError::MalformedProposition)?
        }
        13 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1)?;
            let right = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::integer_less_than(scalar_type, left, right)
                .map_err(CodecError::MalformedProposition)?
        }
        14 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1)?;
            let right = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::integer_less_or_equal(scalar_type, left, right)
                .map_err(CodecError::MalformedProposition)?
        }
        15 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1)?;
            let right = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::integer_bitwise_and(scalar_type, left, right)
                .map_err(CodecError::MalformedProposition)?
        }
        16 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1)?;
            let right = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::integer_bitwise_or(scalar_type, left, right)
                .map_err(CodecError::MalformedProposition)?
        }
        17 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1)?;
            let right = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::integer_bitwise_xor(scalar_type, left, right)
                .map_err(CodecError::MalformedProposition)?
        }
        18 => {
            let value_type = decode_integer_type(reader)?;
            let count_type = decode_integer_type(reader)?;
            let value = decode_scalar_term(reader, depth + 1)?;
            let count = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::wrapping_integer_shift_left(value_type, count_type, value, count)
                .map_err(CodecError::MalformedProposition)?
        }
        19 => {
            let value_type = decode_integer_type(reader)?;
            let count_type = decode_integer_type(reader)?;
            let value = decode_scalar_term(reader, depth + 1)?;
            let count = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::wrapping_integer_shift_right(value_type, count_type, value, count)
                .map_err(CodecError::MalformedProposition)?
        }
        20 => {
            let scalar_type = decode_integer_type(reader)?;
            let operand = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::integer_bitwise_not(scalar_type, operand)
                .map_err(CodecError::MalformedProposition)?
        }
        21 => {
            let source_type = decode_integer_type(reader)?;
            let target_type = decode_integer_type(reader)?;
            let operand = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::integer_widen(source_type, target_type, operand)
                .map_err(CodecError::MalformedProposition)?
        }
        22 => {
            let source_type = decode_integer_type(reader)?;
            let target_type = decode_integer_type(reader)?;
            let operand = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::integer_exact_cast(source_type, target_type, operand)
                .map_err(CodecError::MalformedProposition)?
        }
        23 => {
            let value_type = decode_integer_type(reader)?;
            let count_type = decode_integer_type(reader)?;
            let value = decode_scalar_term(reader, depth + 1)?;
            let count = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::exact_integer_shift_right(value_type, count_type, value, count)
                .map_err(CodecError::MalformedProposition)?
        }
        24 => {
            let value_type = decode_integer_type(reader)?;
            let count_type = decode_integer_type(reader)?;
            let value = decode_scalar_term(reader, depth + 1)?;
            let count = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::exact_integer_shift_left(value_type, count_type, value, count)
                .map_err(CodecError::MalformedProposition)?
        }
        25 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1)?;
            let right = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::exact_integer_add(scalar_type, left, right)
                .map_err(CodecError::MalformedProposition)?
        }
        26 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1)?;
            let right = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::exact_integer_subtract(scalar_type, left, right)
                .map_err(CodecError::MalformedProposition)?
        }
        27 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1)?;
            let right = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::exact_integer_multiply(scalar_type, left, right)
                .map_err(CodecError::MalformedProposition)?
        }
        28 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1)?;
            let right = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::exact_integer_divide(scalar_type, left, right)
                .map_err(CodecError::MalformedProposition)?
        }
        29 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1)?;
            let right = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::exact_integer_remainder(scalar_type, left, right)
                .map_err(CodecError::MalformedProposition)?
        }
        30 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1)?;
            let right = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::wrapping_integer_divide(scalar_type, left, right)
                .map_err(CodecError::MalformedProposition)?
        }
        31 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1)?;
            let right = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::wrapping_integer_remainder(scalar_type, left, right)
                .map_err(CodecError::MalformedProposition)?
        }
        32 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1)?;
            let right = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::saturating_integer_divide(scalar_type, left, right)
                .map_err(CodecError::MalformedProposition)?
        }
        33 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1)?;
            let right = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::saturating_integer_remainder(scalar_type, left, right)
                .map_err(CodecError::MalformedProposition)?
        }
        tag => return Err(CodecError::InvalidTag("ScalarTerm", tag)),
    })
}

fn decode_scalar_type(reader: &mut Reader<'_>) -> Result<ScalarType, CodecError> {
    Ok(match reader.u8()? {
        1 => ScalarType::Boolean,
        2 => ScalarType::Integer(decode_integer_type(reader)?),
        tag => return Err(CodecError::InvalidTag("ScalarType", tag)),
    })
}

fn decode_integer_type(reader: &mut Reader<'_>) -> Result<IntegerType, CodecError> {
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

fn decode_integer_value(reader: &mut Reader<'_>) -> Result<IntegerValue, CodecError> {
    Ok(match reader.u8()? {
        1 => IntegerValue::Signed(i128::from_le_bytes(reader.array()?)),
        2 => IntegerValue::Unsigned(u128::from_le_bytes(reader.array()?)),
        tag => return Err(CodecError::InvalidTag("IntegerValue", tag)),
    })
}

#[derive(Default)]
struct Writer {
    bytes: Vec<u8>,
}

impl Writer {
    fn finish(self) -> Vec<u8> {
        self.bytes
    }

    fn bytes(&mut self, bytes: &[u8]) {
        self.bytes.extend_from_slice(bytes);
    }

    fn u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    fn u16(&mut self, value: u16) {
        self.bytes(&value.to_le_bytes());
    }

    fn u32(&mut self, value: u32) {
        self.bytes(&value.to_le_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.bytes(&value.to_le_bytes());
    }

    fn id(&mut self, id: impl PsiSemanticId) {
        self.bytes(&id.get().to_le_bytes());
    }

    fn len(&mut self, label: &'static str, len: usize) -> Result<(), CodecError> {
        self.u32(u32::try_from(len).map_err(|_| CodecError::CollectionTooLong(label))?);
        Ok(())
    }

    fn string(&mut self, label: &'static str, value: &str) -> Result<(), CodecError> {
        if value.len() > MAX_CONTENT_IDENTITY_BYTES {
            return Err(CodecError::StringTooLong(label));
        }
        self.len(label, value.len())?;
        self.bytes(value.as_bytes());
        Ok(())
    }
}

struct Reader<'bytes> {
    bytes: &'bytes [u8],
    offset: usize,
}

impl<'bytes> Reader<'bytes> {
    const fn new(bytes: &'bytes [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn remaining(&self) -> usize {
        self.bytes.len() - self.offset
    }

    fn take(&mut self, len: usize) -> Result<&'bytes [u8], CodecError> {
        let end = self
            .offset
            .checked_add(len)
            .ok_or(CodecError::UnexpectedEnd)?;
        let bytes = self
            .bytes
            .get(self.offset..end)
            .ok_or(CodecError::UnexpectedEnd)?;
        self.offset = end;
        Ok(bytes)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], CodecError> {
        self.take(N)?
            .try_into()
            .map_err(|_| CodecError::UnexpectedEnd)
    }

    fn u8(&mut self) -> Result<u8, CodecError> {
        Ok(self.array::<1>()?[0])
    }

    fn u16(&mut self) -> Result<u16, CodecError> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    fn u32(&mut self) -> Result<u32, CodecError> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    fn u64(&mut self) -> Result<u64, CodecError> {
        Ok(u64::from_le_bytes(self.array()?))
    }

    fn count(&mut self) -> Result<u32, CodecError> {
        self.u32()
    }

    fn boolean(&mut self) -> Result<bool, CodecError> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            value => Err(CodecError::InvalidBoolean(value)),
        }
    }

    fn string(&mut self, label: &'static str) -> Result<String, CodecError> {
        let len = usize::try_from(self.count()?).map_err(|_| CodecError::StringTooLong(label))?;
        if len > MAX_CONTENT_IDENTITY_BYTES {
            return Err(CodecError::StringTooLong(label));
        }
        let bytes = self.take(len)?;
        std::str::from_utf8(bytes)
            .map(str::to_owned)
            .map_err(|_| CodecError::InvalidUtf8(label))
    }

    fn id<T: PsiSemanticId>(&mut self, label: &'static str) -> Result<T, CodecError> {
        let raw = self.u64()?;
        T::new(raw).ok_or(CodecError::ZeroIdentity(label))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodecError {
    InvalidMagic,
    UnsupportedFormatVersion(u16),
    ZeroSemanticVersion,
    UnexpectedEnd,
    TrailingBytes(usize),
    InvalidBoolean(u8),
    InvalidTag(&'static str, u8),
    ZeroIdentity(&'static str),
    CollectionTooLong(&'static str),
    NonCanonicalOrder(&'static str),
    NonCanonicalEncoding,
    PropositionVocabularyRequiresV16,
    NestedConjunction,
    PropositionNestingTooDeep,
    ScalarTermNestingTooDeep,
    ContentTermNestingTooDeep,
    StringTooLong(&'static str),
    InvalidUtf8(&'static str),
    MalformedProposition(PropositionError),
    InvalidModule(ModuleError),
}

impl std::fmt::Display for CodecError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for CodecError {}
