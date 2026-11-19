//! The name resolver and typer consume the same library declarations. Each
//! machine describes what callers may use. Its implementation belongs to the library.
use super::source_package::TOOLCHAIN_PACKAGE;
use crate::contracts::effect_clauses::DeclaredPauseBounds;
use crate::declarations::{declaration_catalog::DeclarationCatalog, machine_signature::MachineSignature};
use crate::{Access, DataProperties, InterfaceContract, Multiplicity, Site, TypedSignature, TypedStage, ValueType};
use omega_lexer::SourceSpan;
use omega_parser::NodeKind;
use omega_resolver::{DeclarationId, InterfaceExport};
use std::sync::OnceLock;

pub(super) fn identity(
    path: String,
    kind: NodeKind,
) -> DeclarationId {
    DeclarationId {
        package: TOOLCHAIN_PACKAGE.into(),
        path,
        kind,
        operands: vec![],
        result_domains: vec![],
        signature_owners: vec![],
        enclosing_machines: vec![],
    }
}
pub(super) fn interface_site(id: &DeclarationId) -> Site {
    Site {
        package: id.package.clone(),
        file: id.path.clone(),
        span: SourceSpan::new(0, 0),
    }
}
#[derive(Clone, Copy)]
pub(super) enum FieldType {
    Boolean,
    Natural,
    Text,
    Named(&'static str),
    Applied(&'static str),
    Shared(&'static FieldType),
    Slice(&'static FieldType),
    Array(&'static FieldType, u64),
}
impl FieldType {
    fn ty(
        self,
        core: &CoreInterface,
        binder: &DeclarationId,
    ) -> ValueType {
        let parameter = || ValueType::Parameter {
            owner: Box::new(binder.clone()),
            index: 0,
        };
        match self {
            Self::Boolean => ValueType::Bool,
            Self::Natural => ValueType::integer("u64").unwrap(),
            Self::Named(name) => ValueType::Nominal(Box::new(core.id(name).clone())),
            Self::Applied(name) => ValueType::Applied(Box::new(core.id(name).clone()), vec![parameter()]),
            Self::Shared(ty) => ValueType::Reference(Access::Shared, Box::new(ty.ty(core, binder))),
            Self::Slice(ty) => ValueType::Slice(Box::new(ty.ty(core, binder))),
            Self::Array(ty, len) => ValueType::Array(Box::new(ty.ty(core, binder)), len),
            Self::Text => ValueType::AtLifetime(
                Box::new(ValueType::Reference(
                    Access::Shared,
                    Box::new(ValueType::Slice(Box::new(ValueType::integer("u8").unwrap()))),
                )),
                Box::new(parameter()),
            ),
        }
    }
    fn operand(
        self,
        id: &mut DeclarationId,
    ) -> String {
        match self {
            Self::Natural => "u64".into(),
            Self::Applied(name) => {
                id.signature_owners
                    .push((TOOLCHAIN_PACKAGE.into(), format!("language::core::{name}")));
                format!("{} < ' #LifetimeParam0 >", name.rsplit("::").next().unwrap())
            }
            Self::Shared(ty) => format!("@ {}", ty.operand(id)),
            _ => unreachable!("core machine operand"),
        }
    }
}
type Fields = &'static [(&'static str, FieldType)];
pub(super) enum Shape {
    Data { copy: bool, fields: Fields },
    Case { parent: &'static str, fields: Fields },
    Member,
    Trait,
    Machine { params: &'static [FieldType], result: FieldType },
}
#[derive(Default)]
pub(crate) struct CoreInterface {
    pub(super) exports: Vec<InterfaceExport>,
    shapes: Vec<Shape>,
}
impl CoreInterface {
    pub(super) fn add(
        &mut self,
        name: &str,
        kind: NodeKind,
        lifetime: bool,
        shape: Shape,
    ) {
        let module = if name.starts_with("reflect::") {
            "language::core::reflect"
        } else if name.starts_with("access::") {
            "language::core::access"
        } else {
            "language::core"
        };
        let mut id = identity(format!("language::core::{name}"), kind);
        if let Shape::Machine { params, .. } = &shape {
            id.operands = params.iter().map(|p| p.operand(&mut id)).collect();
        }
        self.exports.push(InterfaceExport {
            id,
            module,
            generic_roles: if lifetime { vec![NodeKind::LifetimeParam] } else { vec![] },
        });
        self.shapes.push(shape);
    }
    pub(super) fn data(
        &mut self,
        name: &str,
        lifetime: bool,
        copy: bool,
        fields: Fields,
    ) {
        self.add(name, NodeKind::Data, lifetime, Shape::Data { copy, fields });
        for (field, _) in fields {
            self.add(&format!("{name}::{field}"), NodeKind::Field, false, Shape::Member);
        }
    }
    pub(super) fn case(
        &mut self,
        parent: &'static str,
        name: &str,
        fields: Fields,
    ) {
        let path = format!("{parent}::{name}");
        self.add(&path, NodeKind::Case, false, Shape::Case { parent, fields });
        for (field, _) in fields {
            self.add(&format!("{path}::{field}"), NodeKind::CaseField, false, Shape::Member);
        }
    }
    pub(super) fn machine(
        &mut self,
        name: &str,
        lifetime: bool,
        params: &'static [FieldType],
        result: FieldType,
    ) {
        self.add(name, NodeKind::Machine, lifetime, Shape::Machine { params, result });
    }
    pub(super) fn id(
        &self,
        name: &str,
    ) -> &DeclarationId {
        &self
            .exports
            .iter()
            .find(|e| e.id.path.strip_prefix("language::core::") == Some(name))
            .expect("declared core interface name")
            .id
    }
    pub(crate) fn install<'a>(
        &'a self,
        catalog: &mut DeclarationCatalog<'a>,
        output: &mut TypedStage,
    ) {
        for (export, shape) in self.exports.iter().zip(&self.shapes) {
            let id = &export.id;
            catalog.types.arities.insert(id, export.generic_roles.len());
            if !export.generic_roles.is_empty() {
                catalog.types.lifetimes.insert((id, 0));
            }
            let (binder, fields) = match shape {
                Shape::Data { copy, fields } => {
                    catalog.types.scopes.insert(id, vec![id]);
                    if *copy {
                        let props = DataProperties {
                            multiplicity: Multiplicity::Copy,
                            carry: None,
                        };
                        catalog.types.multiplicities.insert(id, Multiplicity::Copy);
                        catalog.data_properties.insert(id, props.clone());
                        output.data_properties.push((id.clone(), props));
                    }
                    (id, *fields)
                }
                Shape::Case { parent, fields } => {
                    let parent = self.id(parent);
                    catalog.case_owner.insert(id, parent);
                    catalog.cases.entry(parent).or_default().push(id);
                    (parent, *fields)
                }
                Shape::Machine { params, result } => {
                    let params: Vec<_> = params
                        .iter()
                        .map(|ty| (SourceSpan::new(0, 0), ty.ty(self, id)))
                        .collect();
                    let result = result.ty(self, id);
                    let plan = id == self.id("access::new_plan");
                    let requirement = id == self.id("AccessPolicy::plan");
                    output.signatures.push(TypedSignature {
                        site: interface_site(id),
                        machine: id.clone(),
                        parameters: params.clone(),
                        result: result.clone(),
                    });
                    catalog
                        .overloads
                        .entry((&id.package, &id.path))
                        .or_default()
                        .push(id);
                    catalog.signatures.insert(
                        id,
                        MachineSignature {
                            params,
                            result,
                            requires: false,
                            terminates: plan,
                            site: interface_site(id),
                            public: true,
                            satisfies: vec![],
                            crash_ceiling: plan.then(Vec::new),
                            reach: plan.then(Vec::new),
                            pause: DeclaredPauseBounds::interface(!requirement),
                        },
                    );
                    continue;
                }
                Shape::Trait => {
                    catalog
                        .requirements
                        .insert(id, vec![self.id("AccessPolicy::plan")]);
                    continue;
                }
                Shape::Member => continue,
            };
            catalog.fields.insert(
                id,
                fields
                    .iter()
                    .map(|(name, ty)| (name.to_string(), ty.ty(self, binder)))
                    .collect(),
            );
            catalog
                .field_order
                .insert(id, fields.iter().map(|(name, _)| name.to_string()).collect());
        }
        use crate::ContractPredicate::{Equal, LessEqual};
        use crate::ContractTerm::{Field, Natural, Result, Subject};
        for (name, requires) in [
            ("BoundaryReach", &[LessEqual(Field(&Subject, "service_count"), Natural(32))][..]),
            (
                "AccessPlan",
                &[
                    LessEqual(Field(&Subject, "field_count"), Natural(32)),
                    LessEqual(Field(&Subject, "reader_count"), Natural(32)),
                    LessEqual(Field(&Subject, "writer_count"), Natural(32)),
                ][..],
            ),
        ] {
            let id = self.id(name);
            catalog.predicates.insert(id);
            output.interface_contracts.push(InterfaceContract {
                declaration: id.clone(),
                requires,
                ensures: &[],
                copy_parameters: &[],
            });
        }
        let constructor = self.id("access::new_plan");
        output.obligations.push(crate::TypingObligation {
            site: interface_site(constructor),
            kind: crate::ObligationKind::VerifyLibraryImplementation(constructor.clone()),
        });
        output.interface_contracts.push(InterfaceContract {
            declaration: constructor.clone(),
            requires: &[],
            ensures: &[
                Equal(Field(&Result, "field_count"), Natural(0)),
                Equal(Field(&Result, "reader_count"), Natural(0)),
                Equal(Field(&Result, "writer_count"), Natural(0)),
                Equal(Field(&Field(&Result, "reach"), "service_count"), Natural(0)),
            ],
            copy_parameters: &[],
        });
    }
}
pub(crate) fn core_interface() -> &'static CoreInterface {
    static CORE: OnceLock<CoreInterface> = OnceLock::new();
    CORE.get_or_init(|| {
        let mut interface = CoreInterface::default();
        super::core_schema::define(&mut interface);
        interface
    })
}
