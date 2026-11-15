/*
Carry the guest `Build` value from one stage of a Build to the next
(`CarriedBuild::capture`, `CarriedBuild::contents`).

A staged Build runs as several programs, one after another, against one
activation (`admission/build_stages.rs`). Each stage is its own accepted
module, so it has its own instance and its own storage for the `Build` it was
lent. The settings an earlier stage wrote must be the ones a later stage reads
and the ones the finished Build publishes
(`wiki/spec/build/scoped_execution.md`, "Checked Build stages": the
interpreter "preserves the same mutable Build and live storage without
rerunning earlier statements").

`capture` reads every stored leaf of the returned stage's `Build` argument:
each scalar, each selected case and each byte view, named by the declared
field and case names that lead to it. `contents` writes those leaves for the
next stage's module, finding each field and case there by the same names. The
two modules lower one toolchain declaration of `Build`, so the names agree;
their field and case numbers need not, which is why nothing is copied by
number.

A leaf this cannot name or store is refused rather than dropped: a `Build`
whose later stage silently lost a setting would publish a different program.
*/

use semantic_vocabulary::{CanonicalStructuralPathSegment as Path, StructuralTypeId};
use std::sync::Arc;
use terminal_interpreter::{
    TerminalBoundaryStorageValue, TerminalInstance, TerminalStorageContents,
    TerminalStorageObservationValue,
};
use terminal_psi::{
    StructuralCaseDeclaration, StructuralFieldDeclaration, StructuralFieldType,
    StructuralTypeDeclaration, StructuralTypeShape, TerminalModule,
};

/// One step from the `Build` record towards a stored leaf, by declared name.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Step {
    Field(String),
    Case(String),
    Index(u64),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Leaf {
    /// A scalar with the scalar type its storage holds.
    Scalar(TerminalStorageContents),
    /// The selected case of a sum, by its declared name.
    Case(String),
    Bytes(Arc<[u8]>),
}

pub(crate) struct CarriedBuild {
    leaves: Vec<(Vec<Step>, Leaf)>,
}

fn refused(detail: &str) -> String {
    format!(
        "not supported yet: carrying a Build value across `Build::prepare_dependency` when it holds {detail}"
    )
}

fn label(
    module: &TerminalModule,
    identity: terminal_psi::TerminalIdentityIndex,
) -> Result<&str, String> {
    module
        .identities
        .text(identity)
        .ok_or_else(|| "Build value declaration has no descriptor label".to_owned())
}

fn declaration(
    module: &TerminalModule,
    structural_type: StructuralTypeId,
) -> Result<&StructuralTypeDeclaration, String> {
    module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == structural_type)
        .ok_or_else(|| "Build value lost a structural declaration".to_owned())
}

impl CarriedBuild {
    /// Reads the `Build` argument of a stage that returned normally.
    pub(crate) fn capture(instance: &TerminalInstance<'_, '_>) -> Result<Self, String> {
        let entry = instance
            .program()
            .entry()
            .map_err(|error| format!("Build entry: {error:?}"))?;
        let [parameter] = entry.structural_parameters else {
            return Err("Build value requires one original structural argument".to_owned());
        };
        let mut capture = Capture {
            instance,
            module: instance.program().module(),
            path: Vec::new(),
            steps: Vec::new(),
            leaves: Vec::new(),
        };
        capture.structural(parameter.structural_type)?;
        Ok(Self {
            leaves: capture.leaves,
        })
    }

    /// The same leaves as complete contents of the `Build` argument of
    /// `module`, in the canonical path order the interpreter binds them in.
    pub(crate) fn contents(
        &self,
        module: &TerminalModule,
        build: StructuralTypeId,
    ) -> Result<Vec<TerminalBoundaryStorageValue>, String> {
        let mut establish = Establish {
            carried: self,
            module,
            path: Vec::new(),
            steps: Vec::new(),
            values: Vec::new(),
        };
        establish.structural(build)?;
        establish
            .values
            .sort_by(|left, right| left.path.cmp(&right.path));
        Ok(establish.values)
    }

    fn leaf(&self, steps: &[Step]) -> Result<&Leaf, String> {
        self.leaves
            .iter()
            .find(|(carried, _)| carried == steps)
            .map(|(_, leaf)| leaf)
            .ok_or_else(|| refused("a field the earlier stage's Build value does not have"))
    }
}

struct Capture<'instance, 'program, 'host> {
    instance: &'instance TerminalInstance<'program, 'host>,
    module: &'instance TerminalModule,
    path: Vec<Path>,
    steps: Vec<Step>,
    leaves: Vec<(Vec<Step>, Leaf)>,
}

impl Capture<'_, '_, '_> {
    fn observe(&self) -> Result<TerminalStorageObservationValue<'_>, String> {
        self.instance
            .observe_borrowed_entry(0, &self.path)
            .map_err(|error| format!("Build value readback: {error:?}"))
    }

    fn leaf(&mut self) -> Result<(), String> {
        let leaf = match self.observe()? {
            TerminalStorageObservationValue::Bytes(bytes) => {
                let mut copied = vec![0; bytes.len()];
                bytes
                    .copy_to(&mut copied)
                    .map_err(|error| format!("Build value byte readback: {error:?}"))?;
                Leaf::Bytes(copied.into())
            }
            TerminalStorageObservationValue::Storage(
                contents @ TerminalStorageContents::Scalar { .. },
            ) => Leaf::Scalar(contents),
            TerminalStorageObservationValue::Storage(TerminalStorageContents::Bytes(bytes)) => {
                Leaf::Bytes(bytes)
            }
            _ => {
                return Err(refused(
                    "a value the host issued or a selected case read as a leaf",
                ));
            }
        };
        self.leaves.push((self.steps.clone(), leaf));
        Ok(())
    }

    fn fields(&mut self, fields: &[StructuralFieldDeclaration]) -> Result<(), String> {
        for field in fields {
            if field.relevance.is_erased()
                || matches!(field.field_type, StructuralFieldType::Erased { .. })
            {
                continue;
            }
            self.path.push(Path::Field(field.id));
            self.steps
                .push(Step::Field(label(self.module, field.identity)?.to_owned()));
            match field.field_type {
                StructuralFieldType::Structural(structural_type) => {
                    self.structural(structural_type)?
                }
                _ => self.leaf()?,
            }
            self.steps.pop();
            self.path.pop();
        }
        Ok(())
    }

    fn structural(&mut self, structural_type: StructuralTypeId) -> Result<(), String> {
        let declaration = declaration(self.module, structural_type)?;
        match &declaration.shape {
            StructuralTypeShape::Record { fields, .. } => self.fields(fields),
            StructuralTypeShape::Sum { cases } => {
                let TerminalStorageObservationValue::Storage(TerminalStorageContents::Case(
                    selected,
                )) = self.observe()?
                else {
                    return Err("Build value sum did not retain its selected case".to_owned());
                };
                let case = cases
                    .iter()
                    .find(|case| case.id == selected)
                    .ok_or_else(|| "Build value case is outside its declaration".to_owned())?;
                let name = label(self.module, case.identity)?.to_owned();
                self.leaves
                    .push((self.steps.clone(), Leaf::Case(name.clone())));
                self.path.push(Path::Case(selected));
                self.steps.push(Step::Case(name));
                let fields = self.fields(&case.fields);
                self.steps.pop();
                self.path.pop();
                fields
            }
            StructuralTypeShape::PrimitiveScalar(_) | StructuralTypeShape::ByteSequence(_) => {
                self.leaf()
            }
            StructuralTypeShape::FixedArray { element, length } => {
                let length = length
                    .literal()
                    .ok_or_else(|| refused("an array without a literal extent"))?;
                for index in 0..length {
                    self.path.push(Path::FixedIndex(index));
                    self.steps.push(Step::Index(index));
                    let element = self.structural(*element);
                    self.steps.pop();
                    self.path.pop();
                    element?;
                }
                Ok(())
            }
            _ => Err(refused(
                "a reference, a host-issued value, an element view, an atomic cell or a mixed record",
            )),
        }
    }
}

struct Establish<'carried> {
    carried: &'carried CarriedBuild,
    module: &'carried TerminalModule,
    path: Vec<Path>,
    steps: Vec<Step>,
    values: Vec<TerminalBoundaryStorageValue>,
}

impl Establish<'_> {
    fn leaf(&mut self) -> Result<(), String> {
        let contents = match self.carried.leaf(&self.steps)? {
            Leaf::Scalar(contents) => contents.clone(),
            Leaf::Bytes(bytes) => TerminalStorageContents::Bytes(Arc::clone(bytes)),
            Leaf::Case(_) => return Err(refused("a field that changed between a sum and a leaf")),
        };
        self.values.push(TerminalBoundaryStorageValue {
            path: self.path.clone(),
            contents,
        });
        Ok(())
    }

    fn fields(&mut self, fields: &[StructuralFieldDeclaration]) -> Result<(), String> {
        for field in fields {
            if field.relevance.is_erased()
                || matches!(field.field_type, StructuralFieldType::Erased { .. })
            {
                continue;
            }
            self.path.push(Path::Field(field.id));
            self.steps
                .push(Step::Field(label(self.module, field.identity)?.to_owned()));
            match field.field_type {
                StructuralFieldType::Structural(structural_type) => {
                    self.structural(structural_type)?
                }
                _ => self.leaf()?,
            }
            self.steps.pop();
            self.path.pop();
        }
        Ok(())
    }

    fn case<'cases>(
        &self,
        cases: &'cases [StructuralCaseDeclaration],
    ) -> Result<&'cases StructuralCaseDeclaration, String> {
        let Leaf::Case(name) = self.carried.leaf(&self.steps)? else {
            return Err(refused("a field that changed between a leaf and a sum"));
        };
        for case in cases {
            if label(self.module, case.identity)? == name {
                return Ok(case);
            }
        }
        Err(refused(
            "a case the later stage's declaration does not have",
        ))
    }

    fn structural(&mut self, structural_type: StructuralTypeId) -> Result<(), String> {
        let declaration = declaration(self.module, structural_type)?;
        match &declaration.shape {
            StructuralTypeShape::Record { fields, .. } => self.fields(fields),
            StructuralTypeShape::Sum { cases } => {
                let case = self.case(cases)?;
                self.values.push(TerminalBoundaryStorageValue {
                    path: self.path.clone(),
                    contents: TerminalStorageContents::Case(case.id),
                });
                self.path.push(Path::Case(case.id));
                self.steps
                    .push(Step::Case(label(self.module, case.identity)?.to_owned()));
                let fields = self.fields(&case.fields);
                self.steps.pop();
                self.path.pop();
                fields
            }
            StructuralTypeShape::PrimitiveScalar(_) | StructuralTypeShape::ByteSequence(_) => {
                self.leaf()
            }
            StructuralTypeShape::FixedArray { element, length } => {
                let length = length
                    .literal()
                    .ok_or_else(|| refused("an array without a literal extent"))?;
                for index in 0..length {
                    self.path.push(Path::FixedIndex(index));
                    self.steps.push(Step::Index(index));
                    let element = self.structural(*element);
                    self.steps.pop();
                    self.path.pop();
                    element?;
                }
                Ok(())
            }
            _ => Err(refused(
                "a reference, a host-issued value, an element view, an atomic cell or a mixed record",
            )),
        }
    }
}
