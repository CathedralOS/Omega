//! Receive typed Id formation without granting identity truth.
//!
//! The catalog abstracts checked source subjects into ordinary mathematical
//! lambdas. The receiver rechecks their signature and requires actual Id
//! bodies; neither an opaque structural-equality row nor a Boolean comparison
//! can stand in for this kernel type constructor.

use crate::{CodecError, decode_mathematical_certificate};
use proof_admission::{
    Budget, Signature, Term, verify_mathematical_certificate, weak_head_normalize,
};
use terminal_psi::TerminalModule;

pub(crate) fn validate_identity_formations(
    module: &TerminalModule,
    budget: &mut Budget,
) -> Result<(), CodecError> {
    receive_catalog(module, budget).map(|_| ())
}

fn receive_catalog(
    module: &TerminalModule,
    budget: &mut Budget,
) -> Result<Option<crate::DecodedMathematicalCertificate>, CodecError> {
    let catalog = &module.identity_formations;
    let fail = |reason: &str| CodecError::InvalidIdentityFormation(reason.to_owned());
    if catalog.rows.is_empty() {
        return if catalog.certificate.is_empty() {
            Ok(None)
        } else {
            Err(fail("empty Id roster carries a certificate"))
        };
    }
    let mut decoded = decode_mathematical_certificate(&catalog.certificate)?;
    if decoded.certificate.level_arity != 0
        || !decoded.certificate.context.is_empty()
        || !matches!(decoded.arena.get(decoded.certificate.term), Term::TwoZero)
        || !matches!(decoded.arena.get(decoded.certificate.expected), Term::Two)
    {
        return Err(fail(
            "Id formation catalog is not a closed signature judgment",
        ));
    }
    verify_mathematical_certificate(&mut decoded.arena, &decoded.certificate, budget)
        .map_err(|error| fail(&format!("Id formation fails kernel checking: {error:?}")))?;
    let signature = Signature::from_declarations(decoded.certificate.signature.clone());
    let mut previous = None;
    for row in &catalog.rows {
        if previous.is_some_and(|previous| previous >= row.declaration) {
            return Err(fail("Id formation declarations are not strictly ordered"));
        }
        previous = Some(row.declaration);
        if row.subject_origins.len() != row.subject_carriers.len()
            || row.subject_origins.iter().any(|(start, end)| start > end)
        {
            return Err(fail("Id formation lost its ordered source subject origins"));
        }
        for identity in [&row.source_owner, &row.source_carrier]
            .into_iter()
            .chain(row.subject_carriers.iter())
        {
            if identity.is_absent() || module.identities.text(*identity).is_none_or(str::is_empty) {
                return Err(fail("Id formation has no exact source carrier identity"));
            }
        }
        let declaration = decoded
            .certificate
            .signature
            .get(row.declaration as usize)
            .ok_or_else(|| fail("Id formation declaration is absent"))?;
        // The shared signature check quantified this declaration's exact
        // universe telescope. Source-bound verification below compares it with
        // the independently checked source judgment, including its arity.
        let mut body = declaration
            .body
            .ok_or_else(|| fail("Id formation is an assumption rather than a definition"))?;
        for _ in &row.subject_carriers {
            let Term::Lambda { body: next, .. } = decoded.arena.get(body) else {
                return Err(fail("Id formation lost a source subject binder"));
            };
            body = next;
        }
        let body = weak_head_normalize(&mut decoded.arena, &signature, body, budget)
            .map_err(|error| fail(&format!("Id formation reduction failed: {error:?}")))?;
        if !matches!(decoded.arena.get(body), Term::Id { .. }) {
            return Err(fail(
                "Id formation does not retain the kernel Id constructor",
            ));
        }
    }
    Ok(Some(decoded))
}

/// A receiving authority obtains these roots from its own checked source,
/// before untrusted publication. They describe formation, never Id truth.
#[derive(Clone, Debug)]
pub struct ExpectedIdentityFormation {
    pub source_owner: String,
    pub source_carrier: String,
    pub subject_carriers: Vec<String>,
    pub subject_origins: Vec<(u32, u32)>,
    pub judgment: Vec<u8>,
}

/// One shared source signature and its ordered application judgments.
#[derive(Clone, Debug)]
pub struct ExpectedIdentityFormations {
    pub signature: Vec<u8>,
    pub rows: Vec<ExpectedIdentityFormation>,
}

/// Receive the exact checked source Id values and their capture associations.
/// A producer cannot authorize itself by changing a carrier label, swapping
/// subjects, or supplying a different well-typed Id in the same universe.
pub fn verify_identity_formations(
    module: &TerminalModule,
    expected: &ExpectedIdentityFormations,
    budget: &mut Budget,
) -> Result<(), CodecError> {
    let fail = |reason: &str| CodecError::InvalidIdentityFormation(reason.to_owned());
    if module.identity_formations.rows.len() != expected.rows.len() {
        return Err(fail("Id formation source roster differs from the receiver"));
    }
    let Some(decoded) = receive_catalog(module, budget)? else {
        return Ok(());
    };
    let rows = &module.identity_formations.rows;
    let prefix = rows[0].declaration as usize;
    if prefix.checked_add(rows.len()) != Some(decoded.certificate.signature.len()) {
        return Err(fail("Id formation signature has unassociated declarations"));
    }
    let shared = crate::encode_mathematical_certificate(
        &decoded.arena,
        &proof_admission::MathematicalCertificate {
            signature: decoded.certificate.signature[..prefix].to_vec(),
            level_arity: 0,
            context: Vec::new(),
            term: decoded.certificate.term,
            expected: decoded.certificate.expected,
        },
    )?;
    if shared != expected.signature {
        return Err(fail(
            "Id formation signature differs from the receiver's checked source",
        ));
    }
    for (position, (row, trusted)) in rows.iter().zip(&expected.rows).enumerate() {
        if row.declaration as usize != prefix + position
            || module.identities.text(row.source_owner) != Some(trusted.source_owner.as_str())
            || module.identities.text(row.source_carrier) != Some(trusted.source_carrier.as_str())
            || row.subject_origins != trusted.subject_origins
            || row.subject_carriers.len() != trusted.subject_carriers.len()
            || !row
                .subject_carriers
                .iter()
                .zip(&trusted.subject_carriers)
                .all(|(actual, expected)| {
                    module.identities.text(*actual) == Some(expected.as_str())
                })
        {
            return Err(fail(
                "Id formation source carrier or subject association differs from the receiver",
            ));
        }
        let declaration = &decoded.certificate.signature[row.declaration as usize];
        let mut term = declaration
            .body
            .ok_or_else(|| fail("Id formation has no checked body"))?;
        let mut ty = declaration.ty;
        let mut context = Vec::with_capacity(row.subject_carriers.len());
        for _ in &row.subject_carriers {
            let (
                Term::Lambda { domain, body },
                Term::Pi {
                    domain: expected,
                    codomain,
                },
            ) = (decoded.arena.get(term), decoded.arena.get(ty))
            else {
                return Err(fail("Id formation source telescope differs from its type"));
            };
            if !decoded.arena.structurally_equal(domain, expected) {
                return Err(fail("Id formation changed an exact source subject domain"));
            }
            context.push(domain);
            term = body;
            ty = codomain;
        }
        let actual = crate::encode_mathematical_certificate(
            &decoded.arena,
            &proof_admission::MathematicalCertificate {
                signature: Vec::new(),
                level_arity: declaration.level_arity,
                context,
                term,
                expected: ty,
            },
        )?;
        if actual != trusted.judgment {
            return Err(fail(
                "Id value differs from the receiver's checked source formation",
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use proof_admission::{Declaration, Level, MathematicalCertificate, Sort, TermArena};

    #[test]
    fn quantified_identity_formation_retains_its_exact_source_levels() {
        let mut arena = TermArena::new();
        let sort = arena.insert(Term::Sort(Sort::Type(Level::Parameter(0))));
        let carrier = arena.insert(Term::Constant {
            declaration: 0,
            levels: vec![Level::Parameter(0)],
        });
        let left = arena.insert(Term::Variable(1));
        let right = arena.insert(Term::Variable(0));
        let identity = arena.insert(Term::Id {
            ty: carrier,
            left,
            right,
        });
        let body = arena.insert(Term::Lambda {
            domain: carrier,
            body: identity,
        });
        let body = arena.insert(Term::Lambda {
            domain: carrier,
            body,
        });
        let ty = arena.insert(Term::Pi {
            domain: carrier,
            codomain: sort,
        });
        let ty = arena.insert(Term::Pi {
            domain: carrier,
            codomain: ty,
        });
        let prefix = vec![Declaration::assumption(1, sort)];
        let mut signature = prefix.clone();
        signature.push(Declaration::definition(1, ty, body));
        let zero = arena.insert(Term::TwoZero);
        let two = arena.insert(Term::Two);
        let mut certificate = MathematicalCertificate {
            signature,
            level_arity: 0,
            context: Vec::new(),
            term: zero,
            expected: two,
        };
        let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
        let owner = identities.intern("quantified-owner");
        let source_carrier = identities.intern("source-carrier");
        let entry = semantic_vocabulary::MachineId::new(1).unwrap();
        let mut module = TerminalModule::for_entry(entry);
        module.identities = identities.build();
        module
            .identity_formations
            .rows
            .push(terminal_psi::TerminalIdentityFormation {
                source_owner: owner,
                source_carrier,
                subject_carriers: vec![source_carrier; 2],
                subject_origins: vec![(1, 2), (3, 4)],
                declaration: 1,
            });
        let expected = ExpectedIdentityFormations {
            signature: crate::encode_mathematical_certificate(
                &arena,
                &MathematicalCertificate {
                    signature: prefix,
                    level_arity: 0,
                    context: Vec::new(),
                    term: zero,
                    expected: two,
                },
            )
            .unwrap(),
            rows: vec![ExpectedIdentityFormation {
                source_owner: "quantified-owner".into(),
                source_carrier: "source-carrier".into(),
                subject_carriers: vec!["source-carrier".into(); 2],
                subject_origins: vec![(1, 2), (3, 4)],
                judgment: crate::encode_mathematical_certificate(
                    &arena,
                    &MathematicalCertificate {
                        signature: Vec::new(),
                        level_arity: 1,
                        context: vec![carrier; 2],
                        term: identity,
                        expected: sort,
                    },
                )
                .unwrap(),
            }],
        };
        module.identity_formations.certificate =
            crate::encode_mathematical_certificate(&arena, &certificate).unwrap();
        verify_identity_formations(&module, &expected, &mut Budget::default()).unwrap();

        // A quantified function forms in its declared universe scope, but it
        // is not a closed erased actual, even with an exact independent key.
        // Constructor schemas and scalar/proof function actuals use this same
        // closed receiving boundary after source specialization.
        let mut quantified = MathematicalCertificate {
            signature: certificate.signature.clone(),
            level_arity: 1,
            context: Vec::new(),
            term: body,
            expected: ty,
        };
        proof_admission::verify_mathematical_certificate(
            &mut arena,
            &quantified,
            &mut Budget::default(),
        )
        .unwrap();
        let key = crate::mathematical_carrier_key(&arena, &quantified).unwrap();
        let actual = semantic_vocabulary::ProofTerm::Mathematical {
            type_identity: "source-carrier".into(),
            certificate: crate::encode_mathematical_certificate(&arena, &quantified).unwrap(),
        };
        assert!(matches!(
            crate::verify_mathematical_proof_actual(
                &actual,
                "source-carrier",
                &key,
                &std::collections::BTreeSet::new(),
                &mut Budget::default(),
            ),
            Err(crate::MathematicalProofActualError::OpenContext)
        ));
        // Dropping metadata cannot make that same function closed.
        quantified.level_arity = 0;
        let key = crate::mathematical_carrier_key(&arena, &quantified).unwrap();
        let actual = semantic_vocabulary::ProofTerm::Mathematical {
            type_identity: "source-carrier".into(),
            certificate: crate::encode_mathematical_certificate(&arena, &quantified).unwrap(),
        };
        assert!(matches!(
            crate::verify_mathematical_proof_actual(
                &actual,
                "source-carrier",
                &key,
                &std::collections::BTreeSet::new(),
                &mut Budget::default(),
            ),
            Err(crate::MathematicalProofActualError::Kernel(
                proof_admission::CoreError::UnboundLevelParameter { .. }
            ))
        ));

        // Dropping the quantified scope leaves the level application unbound.
        certificate.signature[1].level_arity = 0;
        module.identity_formations.certificate =
            crate::encode_mathematical_certificate(&arena, &certificate).unwrap();
        let error = validate_identity_formations(&module, &mut Budget::default()).unwrap_err();
        assert!(format!("{error:?}").contains("UnboundLevelParameter"));

        // An extra unused level forms, but changes the independently supplied source judgment.
        certificate.signature[1].level_arity = 2;
        module.identity_formations.certificate =
            crate::encode_mathematical_certificate(&arena, &certificate).unwrap();
        validate_identity_formations(&module, &mut Budget::default()).unwrap();
        let error =
            verify_identity_formations(&module, &expected, &mut Budget::default()).unwrap_err();
        assert!(format!("{error:?}").contains("receiver's checked source formation"));

        // A malformed application cannot borrow the declaration's level scope.
        certificate.signature[1].level_arity = 1;
        for (levels, reason) in [
            (Vec::new(), "DeclarationArityMismatch"),
            (vec![Level::Parameter(1)], "UnboundLevelParameter"),
        ] {
            let wrong = arena.insert(Term::Constant {
                declaration: 0,
                levels,
            });
            let identity = arena.insert(Term::Id {
                ty: wrong,
                left,
                right,
            });
            let body = arena.insert(Term::Lambda {
                domain: carrier,
                body: identity,
            });
            let body = arena.insert(Term::Lambda {
                domain: carrier,
                body,
            });
            certificate.signature[1].body = Some(body);
            module.identity_formations.certificate =
                crate::encode_mathematical_certificate(&arena, &certificate).unwrap();
            let error = validate_identity_formations(&module, &mut Budget::default()).unwrap_err();
            assert!(format!("{error:?}").contains(reason));
        }
    }
}
