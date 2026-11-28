/*
Canonical wire encoders and decoders for propositions, scalar terms, integer math terms, content
transformations, callable contracts, proof declarations, proof terms, mathematical certificates,
and scalar qualifications.
*/

pub(crate) use crate::semantic_module::*;

pub(crate) use psi_declaration_encoding::terms::content as content_wire;

pub(crate) mod contract_wire {
    pub(crate) use psi_callable_encoding::contracts::{Contract, Guard, Membership};

    use super::wire_fields::{
        Arguments, CleanupAction, Id, Ids, List, ProofTerms, ScalarTerms, wire_record,
    };
    use terminal_psi::SuccessorEdge;

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
    }

    #[cfg(test)]
    pub(crate) use psi_declaration_encoding::crash_routes::RouteGuard;
    pub(crate) use psi_declaration_encoding::crash_routes::{Bucket, Cause, CrashPredicate};
}

#[cfg(test)]
pub(crate) use psi_declaration_encoding::terms::integer as integer_math_term_wire;

pub(crate) mod mathematical_certificate_wire {
    use std::collections::HashMap;

    use mathematical_kernel::{
        Declaration, DeclarationKind, Level, MathematicalCertificate, Sort, Term, TermArena,
        TermHandle,
    };

    use super::CodecError;
    use super::wire::{Reader, Writer, bounded_count, decode_canonical, decode_counted};
    use super::wire_fields::{Field, U32, wire_enum};

    const MAGIC: &[u8; 8] = b"PSICORE\0";
    const FORMAT_MARKER: u16 = 5;

    pub struct DecodedMathematicalCertificate {
        pub arena: TermArena,
        pub certificate: MathematicalCertificate,
    }

    wire_enum! {
        DeclKind: DeclarationKind, "declaration kind" {
            0 => Ordinary,
            1 => Inductive { num_mutual: U32, mutual_index: U32, num_params: U32, num_constructors: U32 },
            2 => Constructor { inductive: U32, constructor_index: U32 },
            3 => Recursor { inductive: U32 },
        }
    }

    /// Single table for all handle-only and unit `Term` constructors, generating
    /// `children`, row encoding, and row decoding from one definition.
    macro_rules! simple_math_terms {
        (
            unit: [$($u_tag:literal => $u_var:ident),* $(,)?],
            handles: [$($h_tag:literal => $h_var:ident { $($field:ident),+ }),* $(,)?],
        ) => {
            fn simple_children(term: &Term) -> Vec<TermHandle> {
                match *term {
                    $(Term::$h_var { $($field),+ } => vec![$($field),+],)*
                    _ => Vec::new(),
                }
            }

            fn encode_term_row(
                term: Term,
                index_of: impl Fn(TermHandle) -> u32,
                node: &mut Writer,
            ) -> Result<(), CodecError> {
                match term {
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
                        encode_sort(node, sort)?;
                    }
                    Term::Constant { declaration, levels } => {
                        node.u8(20);
                        node.u32(declaration);
                        node.len("constant level arguments", levels.len())?;
                        for level in levels {
                            encode_level(node, level)?;
                        }
                    }
                    $(Term::$u_var => node.u8($u_tag),)*
                    $(Term::$h_var { $($field),+ } => {
                        node.u8($h_tag);
                        $(node.u32(index_of($field));)+
                    })*
                }
                Ok(())
            }

            pub(crate) fn decode_term(
                reader: &mut Reader<'_>,
                handles: &[TermHandle],
            ) -> Result<Term, CodecError> {
                let child = |reader: &mut Reader<'_>| -> Result<TermHandle, CodecError> {
                    let index = usize::try_from(reader.u32()?).map_err(|_| {
                        CodecError::MalformedMathematicalCertificate("term child index too large")
                    })?;
                    handles.get(index).copied().ok_or(
                        CodecError::MalformedMathematicalCertificate(
                            "term child does not precede its parent",
                        ),
                    )
                };
                Ok(match reader.u8()? {
                    1 => Term::Variable(reader.u32()?),
                    2 => Term::Sort(decode_sort(reader)?),
                    20 => Term::Constant {
                        declaration: reader.u32()?,
                        levels: decode_counted(reader, decode_level)?,
                    },
                    $($u_tag => Term::$u_var,)*
                    $($h_tag => {
                        $(let $field = child(reader)?;)+
                        Term::$h_var { $($field),+ }
                    })*
                    tag => return Err(CodecError::InvalidTag("MathematicalTerm", tag)),
                })
            }
        };
    }

    simple_math_terms! {
        unit: [10 => Two, 11 => TwoZero, 12 => TwoOne, 21 => Empty],
        handles: [
            3 => Pi { domain, codomain },
            4 => Lambda { domain, body },
            5 => Apply { function, argument },
            6 => Sigma { domain, codomain },
            7 => Pair { first, second },
            8 => Fst { pair },
            9 => Snd { pair },
            13 => CaseTwo { motive, zero_branch, one_branch, scrutinee },
            14 => Id { ty, left, right },
            15 => Refl { ty, value },
            16 => IdElim { motive, base, endpoint, proof },
            22 => EmptyElim { ty, scrutinee },
            23 => Squash { ty },
            24 => SquashIntro { ty, value },
            25 => SquashElim { proposition, function, scrutinee },
            26 => Box { ty },
            27 => BoxIntro { ty, value },
            28 => BoxElim { motive, body, scrutinee },
        ],
    }

    pub fn encode_mathematical_certificate(
        arena: &TermArena,
        certificate: &MathematicalCertificate,
    ) -> Result<Vec<u8>, CodecError> {
        let mut writer = Writer::default();
        let mut term_writer = MathematicalTermWriter::new(arena, writer.nested());
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
            count,
            ..
        } = term_writer;

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
            writer.boolean(declaration.body.is_some());
            if let Some(body) = declaration.body {
                writer.u32(by_handle[&body]);
            }
            DeclKind::write(&mut writer, &declaration.kind)?;
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
                    signature.push(Declaration {
                        level_arity: reader.u32()?,
                        ty: decode_root(reader, &handles)?,
                        body: if reader.boolean()? {
                            Some(decode_root(reader, &handles)?)
                        } else {
                            None
                        },
                        kind: DeclKind::read(reader)?,
                    });
                }
                let context_count = bounded_count(reader, 1)?;
                let mut context = Vec::with_capacity(context_count);
                for _ in 0..context_count {
                    context.push(decode_root(reader, &handles)?);
                }
                let term = decode_root(reader, &handles)?;
                let expected = decode_root(reader, &handles)?;
                Ok(DecodedMathematicalCertificate {
                    arena,
                    certificate: MathematicalCertificate {
                        signature,
                        level_arity,
                        context,
                        term,
                        expected,
                    },
                })
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
        fn new(arena: &'arena TermArena, nodes: Writer) -> Self {
            Self {
                arena,
                nodes,
                by_handle: HashMap::new(),
                by_bytes: HashMap::new(),
                count: 0,
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
                        pending.extend(simple_children(&term).into_iter().rev().map(Step::Visit));
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
            let mut node = self.nodes.nested();
            encode_term_row(
                self.arena.get(handle),
                |child| self.by_handle[&child],
                &mut node,
            )?;
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
        writer.u8(match sort {
            Sort::Type(_) => 1,
            Sort::Strict(_) => 2,
        });
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

    pub(crate) fn decode_sort(reader: &mut Reader<'_>) -> Result<Sort, CodecError> {
        Ok(match reader.u8()? {
            1 => Sort::Type(decode_level(reader)?),
            2 => Sort::Strict(decode_level(reader)?),
            tag => return Err(CodecError::InvalidTag("MathematicalSort", tag)),
        })
    }

    /// Reads a prefix-order level with an explicit stack of unfinished
    /// constructors, so nesting is bounded only by the bytes present.
    pub(crate) fn decode_level(reader: &mut Reader<'_>) -> Result<Level, CodecError> {
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
        handles
            .get(index)
            .copied()
            .ok_or(CodecError::MalformedMathematicalCertificate(
                "root index outside the term table",
            ))
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

pub(crate) use psi_callable_encoding::proof_declarations as proof_declaration_wire;

pub(crate) mod proof_term_wire {
    use semantic_vocabulary::{MathematicalArgument, ProofFormalScope, ProofTerm, ProofTermField};

    use super::CodecError;
    use super::wire::{Reader, Writer, bounded_count, decode_bytes, decode_counted};
    use super::wire_fields::{Field, List, Optional, ScalarTermField, Text, wire_enum};
    use crate::term_table::{TableChildren, TermEncoding, decode_term_table, encode_term_table};

    pub(crate) use psi_callable_encoding::contracts::{decode_proof_formals, encode_proof_formals};

    const PROOF_TERM_CONSTRUCTION: u8 = 1;
    const PROOF_TERM_FORMAL: u8 = 2;
    const PROOF_TERM_SCALAR: u8 = 3;
    const PROOF_TERM_ARRAY: u8 = 4;
    const PROOF_TERM_MATHEMATICAL: u8 = 5;
    const PROOF_TERM_MATHEMATICAL_APPLICATION: u8 = 6;
    const PROOF_TERM_INVOCATION_FORMAL: u8 = 7;
    const PROOF_TERM_MIXED_MATHEMATICAL_APPLICATION: u8 = 8;
    const PROOF_TERM_PROJECTION: u8 = 9;

    wire_enum! {
        FormalScope: ProofFormalScope, "proof formal scope" {
            0 => Block,
            1 => Invocation,
        }
    }

    pub(crate) fn encode_proof_terms(
        writer: &mut Writer,
        terms: &[ProofTerm],
    ) -> Result<(), CodecError> {
        writer.counted("proof terms", terms, encode_term_table::<ProofTermTable>)
    }

    pub(crate) fn decode_proof_terms(
        reader: &mut Reader<'_>,
    ) -> Result<Vec<ProofTerm>, CodecError> {
        decode_counted(reader, decode_term_table::<ProofTermTable>)
    }

    struct ProofTermTable;

    impl TermEncoding for ProofTermTable {
        type Term = ProofTerm;
        const LABEL: &'static str = "proof term";

        fn children<'term>(term: &'term Self::Term, children: &mut Vec<&'term Self::Term>) {
            match term {
                ProofTerm::Construction { fields, .. } => {
                    children.extend(fields.iter().map(|field| &field.term))
                }
                ProofTerm::Array { elements, .. } => children.extend(elements),
                _ => {}
            }
        }

        fn write_row(term: &Self::Term, writer: &mut Writer) -> Result<(), CodecError> {
            write_proof_row(writer, term)
        }

        fn read_row(
            reader: &mut Reader<'_>,
            children: &mut TableChildren<'_, Self::Term>,
        ) -> Result<Self::Term, CodecError> {
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
                FormalScope::write(writer, formal_scope)?;
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
                            ScalarTermField::write(writer, term)?;
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
                Optional::<Text>::write(writer, case_identity)?;
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
                ScalarTermField::write(writer, scalar)?;
            }
            ProofTerm::Projection {
                type_identity,
                formal_position,
                fields,
            } => {
                writer.u8(PROOF_TERM_PROJECTION);
                writer.string("proof term type identity", type_identity)?;
                writer.u32(*formal_position);
                List::<Text>::write(writer, fields)?;
            }
        }
        Ok(())
    }

    fn read_proof_row(
        reader: &mut Reader<'_>,
        children: &mut TableChildren<'_, ProofTerm>,
    ) -> Result<ProofTerm, CodecError> {
        Ok(match reader.u8()? {
            PROOF_TERM_MATHEMATICAL => {
                let type_identity = reader.string("proof term type identity")?;
                let certificate = decode_bytes(reader)?;
                super::mathematical_certificate_wire::decode_mathematical_certificate(
                    &certificate,
                )?;
                ProofTerm::Mathematical {
                    type_identity,
                    certificate,
                }
            }
            tag @ (PROOF_TERM_MATHEMATICAL_APPLICATION
            | PROOF_TERM_MIXED_MATHEMATICAL_APPLICATION) => {
                let type_identity = reader.string("proof term type identity")?;
                let certificate = decode_bytes(reader)?;
                super::mathematical_certificate_wire::decode_mathematical_certificate(
                    &certificate,
                )?;
                let formal_scope = FormalScope::read(reader)?;
                let mixed = tag == PROOF_TERM_MIXED_MATHEMATICAL_APPLICATION;
                let count = bounded_count(reader, if mixed { 5 } else { 4 })?;
                let mut arguments = Vec::with_capacity(count);
                for _ in 0..count {
                    let kind = if mixed { reader.u8()? } else { 0 };
                    arguments.push(match kind {
                        0 => MathematicalArgument::Formal {
                            position: reader.u32()?,
                        },
                        1 => MathematicalArgument::Scalar {
                            type_identity: reader.string("mathematical scalar source carrier")?,
                            term: ScalarTermField::read(reader)?,
                        },
                        tag => {
                            return Err(CodecError::InvalidTag("mathematical argument kind", tag));
                        }
                    });
                }
                ProofTerm::MathematicalApplication {
                    type_identity,
                    certificate,
                    arguments,
                    formal_scope,
                }
            }
            PROOF_TERM_CONSTRUCTION => {
                let type_identity = reader.string("proof term type identity")?;
                let case_identity = Optional::<Text>::read(reader)?;
                let identities = List::<Text>::read(reader)?;
                let terms = children.take(reader, identities.len())?;
                let fields = identities
                    .into_iter()
                    .zip(terms)
                    .map(|(field_identity, term)| ProofTermField {
                        field_identity,
                        term,
                    })
                    .collect();
                ProofTerm::Construction {
                    type_identity,
                    case_identity,
                    fields,
                }
            }
            PROOF_TERM_ARRAY => {
                let type_identity = reader.string("proof term type identity")?;
                let count = bounded_count(reader, 4)?;
                ProofTerm::Array {
                    type_identity,
                    elements: children.take(reader, count)?,
                }
            }
            PROOF_TERM_FORMAL => ProofTerm::Formal {
                position: reader.u32()?,
            },
            PROOF_TERM_INVOCATION_FORMAL => ProofTerm::InvocationFormal {
                position: reader.u32()?,
            },
            PROOF_TERM_SCALAR => ProofTerm::Scalar(ScalarTermField::read(reader)?),
            PROOF_TERM_PROJECTION => ProofTerm::Projection {
                type_identity: reader.string("proof term type identity")?,
                formal_position: reader.u32()?,
                fields: List::<Text>::read(reader)?,
            },
            tag => return Err(CodecError::InvalidTag("proof term", tag)),
        })
    }

    #[cfg(test)]
    mod mathematical_tests {
        use super::super::mathematical_certificate_wire::encode_mathematical_certificate;

        use super::*;
        use mathematical_kernel::{Declaration, MathematicalCertificate, Term, TermArena};

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
            let marker = u16::from_le_bytes([bytes[8], bytes[9]]);
            let row = format!(r"| mathematical certificate | `PSICORE\0` | {marker} | no |");
            assert!(
                include_str!("../encoding.md")
                    .lines()
                    .any(|line| line == row)
            );
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

pub(crate) use psi_declaration_encoding::terms::proposition as proposition_wire;

pub(crate) mod scalar_qualification_wire {
    use super::CodecError;
    use super::module_wire::declaration_wire::{
        decode_establishment_routes, decode_required_memberships, encode_establishment_routes,
        encode_required_memberships,
    };
    use super::wire::{Reader, Writer, require_increasing, strictly_increasing};
    use super::wire_fields::{
        Field, Flag, Float, Id, Ids, Index, Integer, IntegerTypeField, List, Optional, Proposition,
        Scalar, U32, delegated_fields, wire_enum, wire_record,
    };
    use semantic_vocabulary::{QualifiedScalarType, ScalarQualificationSetId};
    use terminal_psi::{
        ScalarDomainDeclaration, ScalarDomainPredicate, ScalarFieldQualification, ScalarFloatRange,
        ScalarIntegerRange, ScalarPrimitiveQualification, ScalarQualificationCatalog,
        ScalarQualificationCoercion, ScalarQualificationSet, ScalarReturnOrigin,
        ScalarReturnQualification, ScalarReturnSite,
    };

    struct SetId;
    impl Field<ScalarQualificationSetId> for SetId {
        fn write(writer: &mut Writer, value: &ScalarQualificationSetId) -> Result<(), CodecError> {
            writer.u64(value.get());
            Ok(())
        }
        fn read(reader: &mut Reader<'_>) -> Result<ScalarQualificationSetId, CodecError> {
            Ok(ScalarQualificationSetId::new(reader.u64()?))
        }
    }

    #[cfg(test)]
    use terminal_psi::TerminalIdentityIndex;

    delegated_fields! {
        RequiredMemberships: Option<Vec<semantic_vocabulary::DomainSemanticId>>
            => encode_required_memberships, decode_required_memberships;
        EstablishmentRoutes: Option<Vec<terminal_psi::ScalarDomainEstablishmentRoute>>
            => encode_establishment_routes, decode_establishment_routes;
    }

    wire_enum! {
        ReturnSite: ScalarReturnSite, "ScalarReturnSite" {
            0 => NormalExit,
            1 => Completion { argument_ordinal: U32 },
        }
        ReturnOrigin: ScalarReturnOrigin, "ScalarReturnOrigin" {
            0 => Forwarded,
            1 => Established { route: U32 },
            2 => Vacuous,
            3 => Proved,
        }
    }

    wire_record! {
        DomainPredicate: ScalarDomainPredicate { subject: Id, proposition: Proposition }
        Domain: ScalarDomainDeclaration {
            id: Id,
            semantic_domain: Id,
            required_memberships: RequiredMemberships,
            identity: Index,
            carrier: Scalar,
            predicate: Optional<DomainPredicate>,
            establishment_routes: EstablishmentRoutes,
        }
        ReturnQualification: ScalarReturnQualification {
            machine: Id,
            edge: Id,
            source: Id,
            result: Id,
            site: ReturnSite,
            domain: Id,
            origins: List<ReturnOrigin>,
        }
        QualificationSet: ScalarQualificationSet { id: SetId, domains: Ids }
        QualifiedScalar: QualifiedScalarType { scalar_type: Scalar, qualifications: SetId }
        FieldQualification: ScalarFieldQualification { field: Id, value_type: QualifiedScalar }
        PrimitiveQualification: ScalarPrimitiveQualification {
            structural_type: Id,
            value_type: QualifiedScalar,
        }
        Coercion: ScalarQualificationCoercion {
            machine: Id,
            edge: Id,
            argument_ordinal: U32,
            source: Id,
            destination: Id,
        }
        FloatRange: ScalarFloatRange {
            machine: Id,
            parameter: Id,
            minimum: Float,
            maximum: Float,
            maximum_inclusive: Flag,
        }
        IntegerRange: ScalarIntegerRange {
            machine: Id,
            parameter: Id,
            integer_type: IntegerTypeField,
            minimum: Integer,
            maximum: Integer,
        }
        Catalog: ScalarQualificationCatalog {
            domains: List<Domain>,
            returns: List<ReturnQualification>,
            sets: List<QualificationSet>,
            fields: List<FieldQualification>,
            primitives: List<PrimitiveQualification>,
            coercions: List<Coercion>,
            float_entry_ranges: List<FloatRange>,
            integer_entry_ranges: List<IntegerRange>,
        }
    }

    pub(crate) fn encode(
        writer: &mut Writer,
        catalog: &ScalarQualificationCatalog,
    ) -> Result<(), CodecError> {
        Catalog::write(writer, catalog)
    }

    pub(crate) fn decode(
        reader: &mut Reader<'_>,
    ) -> Result<ScalarQualificationCatalog, CodecError> {
        Catalog::read(reader)
    }

    pub(crate) fn validate(catalog: &ScalarQualificationCatalog) -> Result<(), CodecError> {
        require_increasing(
            catalog.primitives.iter().map(|row| row.structural_type),
            "scalar primitive qualifications",
        )?;
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
    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn primitive_types_and_proved_completions_round_trip_without_erasure() {
            use semantic_vocabulary::{
                DomainSemanticId, EdgeId, IntegerSign, IntegerType, MachineId, QualifiedScalarType,
                ScalarDomainId, ScalarType, StructuralTypeId, ValueId,
            };
            let carrier = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 32).unwrap());
            let domain = ScalarDomainId::new(1).unwrap();
            let set = ScalarQualificationSetId::new(1);
            let catalog = ScalarQualificationCatalog {
                domains: vec![ScalarDomainDeclaration {
                    required_memberships: Some(Vec::new()),
                    predicate: Some(ScalarDomainPredicate {
                        subject: ValueId::new(1).unwrap(),
                        proposition: semantic_vocabulary::Proposition::Truth,
                    }),
                    id: domain,
                    semantic_domain: DomainSemanticId::new(1).unwrap(),
                    identity: TerminalIdentityIndex(1),
                    carrier,
                    establishment_routes: None,
                }],
                sets: vec![ScalarQualificationSet {
                    id: set,
                    domains: vec![domain],
                }],
                primitives: vec![terminal_psi::ScalarPrimitiveQualification {
                    structural_type: StructuralTypeId::new(1).unwrap(),
                    value_type: QualifiedScalarType {
                        scalar_type: carrier,
                        qualifications: set,
                    },
                }],
                returns: vec![ScalarReturnQualification {
                    site: ScalarReturnSite::Completion {
                        argument_ordinal: 0,
                    },
                    machine: MachineId::new(1).unwrap(),
                    edge: EdgeId::new(1).unwrap(),
                    source: ValueId::new(1).unwrap(),
                    result: ValueId::new(2).unwrap(),
                    domain,
                    origins: vec![ScalarReturnOrigin::Proved],
                }],
                ..ScalarQualificationCatalog::default()
            };
            validate(&catalog).unwrap();
            let mut writer = Writer::default();
            encode(&mut writer, &catalog).unwrap();
            let bytes = writer.finish();
            let mut reader = Reader::new(&bytes);
            assert_eq!(decode(&mut reader).unwrap(), catalog);
            assert_eq!(reader.remaining(), 0);
            let mut erased = catalog;
            erased.primitives[0].value_type.qualifications = ScalarQualificationSetId::ZERO;
            let mut writer = Writer::default();
            encode(&mut writer, &erased).unwrap();
            assert_ne!(
                writer.finish(),
                bytes,
                "primitive qualification participates in canonical bytes"
            );
        }
    }
}

pub(crate) use psi_declaration_encoding::terms::scalar as scalar_term_wire;

pub(crate) use psi_declaration_encoding::terms::scalar_types as scalar_wire;
