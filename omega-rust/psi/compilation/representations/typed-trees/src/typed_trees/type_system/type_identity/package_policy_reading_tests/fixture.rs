//! The program a witness is rendered in: four packages, a toolchain source, and
//! the declarations that the witnesses name.

use crate::TypedTrees;
use crate::data::{DataDefinition, TypeParameter, TypeParameterKind};
use crate::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, NamePath, StaticMachineArgument,
    TableBinaryExpression, TableCallExpression,
};
use crate::name::Identifier;
use crate::type_identity::ExactOwnerTypeIdentityRequest;
use crate::typed_trees::{AccessFieldPlan, AccessViewPlan, AccessorTarget};
use crate::types::{FixedArrayLength, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};
use access_plans::{AccessFields, AccessPlan, BoundaryReach, FieldAccess, validate_access_plan};
use layout_plans::{LayoutFieldEntryReport, LayoutPlacementReport, LayoutPlanReport};
use numerics::arithmetic::ArithmeticDomain;
use numerics::literals::{IntegerLanding, IntegerLiteral, LandedIntegerType};
use semantic_vocabulary::PackageKeyIdentity;
use source::{SourceId, SourceMap, SourceOrigin, SourceSpan, Span};
use std::path::PathBuf;
use std::sync::Arc;
use symbols::{BuiltinTypeAtom, SymbolHandle, SymbolKind, SymbolNameRef, SymbolTableBuilder};

/// Each declaration's owner: a managed package by digit, or the toolchain at 0.
/// Names are unique across packages so a witness names its symbol by spelling.
const DECLARATIONS: &[(u8, SymbolKind, &str)] = &[
    (1, SymbolKind::Data, "Payload"),
    (1, SymbolKind::Data, "Container"),
    (2, SymbolKind::Data, "Record"),
    (2, SymbolKind::Data, "Count"),
    (2, SymbolKind::Const, "LIMIT"),
    (2, SymbolKind::Field, "left"),
    (3, SymbolKind::Trait, "Hooks"),
    (3, SymbolKind::Machine, "size"),
    (3, SymbolKind::Const, "WIDTH"),
    (3, SymbolKind::Field, "right"),
    (4, SymbolKind::Machine, "Selected"),
    (4, SymbolKind::Domain, "Aligned"),
    (4, SymbolKind::Field, "spare"),
    (0, SymbolKind::Data, "Source"),
    // `Window<Policy, Schema>` produced the view `Placed`; `Accessor` and `Cursor`
    // are its generated accessors, and `read_machine` with `read_state` one operation.
    (1, SymbolKind::Data, "Window"),
    (2, SymbolKind::Data, "Policy"),
    (3, SymbolKind::Data, "Schema"),
    (1, SymbolKind::Data, "Placed"),
    (1, SymbolKind::Data, "Accessor"),
    (1, SymbolKind::Data, "Cursor"),
    (3, SymbolKind::Machine, "read_machine"),
    (3, SymbolKind::State, "read_state"),
];

/// The type parameters of `Container`, which belongs to package 1.
const PARAMETERS: [&str; 3] = ["T", "N", "V"];

const TOOLCHAIN_DIGEST: [u8; 32] = [9; 32];

pub(super) struct Fixture {
    pub(super) program: TypedTrees,
    toolchain: [(SourceId, [u8; 32]); 1],
    symbols: Vec<(&'static str, SymbolHandle)>,
    pub(super) u64_type: TypeReferenceHandle,
}

impl Fixture {
    pub(super) fn new() -> Self {
        let names = DECLARATIONS
            .iter()
            .map(|(_, _, name)| *name)
            .chain(PARAMETERS)
            .collect::<Vec<_>>();
        let mut text = String::new();
        let mut spans = Vec::new();
        for name in &names {
            let start = text.len();
            text.push_str(name);
            spans.push((*name, Span::new(start, text.len())));
            text.push(' ');
        }
        let mut sources = SourceMap::default();
        let package_sources = [1u8, 2, 3, 4].map(|package| {
            let file = sources.add_with_metadata(
                PathBuf::from(format!("package{package}.omg")),
                text.clone(),
                PathBuf::from("package"),
                PackageKeyIdentity::from_digest([package; 32]),
                SourceOrigin::User,
            );
            (package, file.source_id)
        });
        let toolchain_source = sources
            .add_with_metadata(
                PathBuf::from("toolchain.omg"),
                text.clone(),
                PathBuf::from("toolchain"),
                None,
                SourceOrigin::Toolchain,
            )
            .source_id;
        let span_of = |name: &str, package: u8| {
            let source = if package == 0 {
                toolchain_source
            } else {
                package_sources
                    .iter()
                    .find(|(candidate, _)| *candidate == package)
                    .expect("a declared package has a source")
                    .1
            };
            let span = spans
                .iter()
                .find(|(candidate, _)| *candidate == name)
                .expect("every name is in the source text")
                .1;
            SymbolNameRef::Source(SourceSpan::new(source, span))
        };

        let mut builder = SymbolTableBuilder::with_sources(Some(Arc::new(sources)));
        let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        // The hierarchy takes one child range per parent: the compiler's builtin
        // slots lead it, and the declarations follow in the same range.
        let mut children = SymbolTableBuilder::child_handles(
            builder.insert_children(
                root,
                BuiltinTypeAtom::ALL
                    .iter()
                    .map(|atom| {
                        (
                            SymbolKind::BuiltinType,
                            SymbolNameRef::Static(atom.symbol_name()),
                        )
                    })
                    .chain(
                        DECLARATIONS
                            .iter()
                            .map(|(package, kind, name)| (*kind, span_of(name, *package))),
                    ),
            ),
        )
        .collect::<Vec<_>>();
        let declared = children.split_off(BuiltinTypeAtom::ALL.len());
        let u64_symbol = children[BuiltinTypeAtom::U64.ordinal()];
        let mut symbols = DECLARATIONS
            .iter()
            .map(|(_, _, name)| *name)
            .zip(declared)
            .collect::<Vec<_>>();
        let container = symbols
            .iter()
            .find(|(name, _)| *name == "Container")
            .expect("Container is declared")
            .1;
        let parameters = SymbolTableBuilder::child_handles(
            builder.insert_children(
                container,
                PARAMETERS
                    .iter()
                    .map(|name| (SymbolKind::TypeParameter, span_of(name, 1))),
            ),
        )
        .collect::<Vec<_>>();
        symbols.extend(PARAMETERS.iter().copied().zip(parameters));

        let mut program = TypedTrees {
            symbols: builder.finish(),
            ..TypedTrees::default()
        };
        let u64_type = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: u64_symbol,
                name: Identifier::generated("u64"),
            });
        let kinds = [
            TypeParameterKind::Type,
            TypeParameterKind::Const {
                type_reference: u64_type,
            },
            TypeParameterKind::Value {
                type_reference: u64_type,
            },
        ];
        let type_parameters =
            program
                .data_type_parameters
                .insert_many(PARAMETERS.iter().zip(&kinds).map(|(name, kind)| {
                    TypeParameter {
                        symbol: symbols
                            .iter()
                            .find(|(candidate, _)| candidate == name)
                            .expect("a parameter is declared")
                            .1,
                        name: Identifier::generated(*name),
                        kind: kind.clone(),
                        ..TypeParameter::default()
                    }
                }));
        program.push_data_definition(DataDefinition {
            symbol: container,
            name: Identifier::generated("Container"),
            type_parameters,
            ..DataDefinition::default()
        });
        let toolchain = [(toolchain_source, TOOLCHAIN_DIGEST)];
        let mut fixture = Self {
            program,
            toolchain,
            symbols,
            u64_type,
        };
        fixture.install_placed_view();
        fixture
    }

    /// The compiler derives declarations from a generic application. The renderer
    /// names them by that producer, so the view, its accessors and its operation
    /// carry the application in place of their generated spelling.
    fn install_placed_view(&mut self) {
        let (policy, schema) = (self.named("Policy"), self.named("Schema"));
        let origin = self.generic("Window", vec![policy, schema]);
        let entry = |field: &str, offset| LayoutFieldEntryReport {
            field: field.into(),
            member_identity: None,
            placement: LayoutPlacementReport::At { offset },
        };
        let layout = LayoutPlanReport {
            schema_report_fingerprint: 1,
            entries: vec![entry("words", 0), entry("flags", 8)],
            offsets: None,
            size: Some(16),
            align: 8,
        };
        let access = AccessFields::inaccessible(&layout).expect("schema slots");
        let keys = [
            access.key_at(0).expect("first field"),
            access.key_at(1).expect("second field"),
        ];
        let placement = validate_access_plan(AccessPlan {
            layout,
            access,
            reach: BoundaryReach::default(),
        })
        .expect("an inaccessible placement validates");
        let field = |key, name: &str, member_identity, accessor: &str, targets| AccessFieldPlan {
            access_key: key,
            view_field_symbol: SymbolHandle::invalid(),
            field_name: name.to_owned(),
            member_identity,
            field_symbol: SymbolHandle::invalid(),
            accessor_name: accessor.to_owned(),
            accessor_type: self.u64_type,
            accessor_data_symbol: self.symbol(accessor),
            accessor_targets: targets,
            value_type: self.u64_type,
            access: FieldAccess::Inaccessible,
            atomic_resident: None,
        };
        let read = AccessorTarget {
            operation: "read".to_owned(),
            machine_symbol: self.symbol("read_machine"),
            state_symbol: self.symbol("read_state"),
        };
        let fields = vec![
            field(keys[0], "words", Some(7), "Accessor", vec![read]),
            field(keys[1], "flags", None, "Cursor", Vec::new()),
        ];
        self.program.access_view_plans.push(AccessViewPlan {
            template_symbol: self.symbol("Window"),
            data_name: "Placed".to_owned(),
            data_symbol: self.symbol("Placed"),
            policy_name: "Policy".to_owned(),
            policy_symbol: self.symbol("Policy"),
            policy_plan_machine_symbol: SymbolHandle::invalid(),
            schema_name: "Schema".to_owned(),
            schema_symbol: self.symbol("Schema"),
            placement,
            fields,
        });
        self.program.push_data_definition(DataDefinition {
            symbol: self.symbol("Placed"),
            name: Identifier::generated("Placed"),
            generic_instance: Some(origin),
            ..DataDefinition::default()
        });
    }

    pub(super) fn symbol(&self, name: &str) -> SymbolHandle {
        self.symbols
            .iter()
            .find(|(candidate, _)| *candidate == name)
            .unwrap_or_else(|| panic!("`{name}` is not declared"))
            .1
    }

    pub(super) fn type_node(&mut self, node: TypeReferenceNode) -> TypeReferenceHandle {
        self.program.type_reference_table.insert(node)
    }

    pub(super) fn named(&mut self, name: &str) -> TypeReferenceHandle {
        let symbol = self.symbol(name);
        self.type_node(TypeReferenceNode::Named {
            symbol,
            name: Identifier::generated(name),
        })
    }

    /// A leaf with no declaration: an integer spelling or a canonical constant.
    pub(super) fn spelled(&mut self, spelling: &str) -> TypeReferenceHandle {
        self.type_node(TypeReferenceNode::Named {
            symbol: SymbolHandle::invalid(),
            name: Identifier::generated(spelling),
        })
    }

    pub(super) fn generic(
        &mut self,
        base: &str,
        arguments: Vec<TypeReferenceHandle>,
    ) -> TypeReferenceHandle {
        let base_symbol = self.symbol(base);
        let arguments = self
            .program
            .type_reference_table
            .insert_type_reference_handles(arguments);
        self.type_node(TypeReferenceNode::Generic {
            base_symbol,
            base_name: Identifier::generated(base),
            lifetime_arguments: Vec::new(),
            arguments,
        })
    }

    pub(super) fn constrained(
        &mut self,
        constraints: Vec<TypeConstraintNode>,
    ) -> TypeReferenceHandle {
        let constraints = self
            .program
            .type_reference_table
            .insert_constraints(constraints);
        self.type_node(TypeReferenceNode::Constrained {
            base_type: self.u64_type,
            constraints,
        })
    }

    pub(super) fn array(&mut self, length: FixedArrayLength) -> TypeReferenceHandle {
        self.type_node(TypeReferenceNode::FixedArray {
            element_type: self.u64_type,
            length,
        })
    }

    pub(super) fn index(&mut self, expression: ExpressionHandle) -> TypeReferenceHandle {
        self.type_node(TypeReferenceNode::ConstExpression(expression))
    }

    pub(super) fn expression(&mut self, node: ExpressionNode) -> ExpressionHandle {
        self.program.expression_table.insert(node)
    }

    pub(super) fn integer(&mut self, value: i64) -> ExpressionHandle {
        self.expression(ExpressionNode::Integer(
            IntegerLiteral::from_value(value).with_landing(IntegerLanding {
                landed_type: LandedIntegerType::U64,
                domain: ArithmeticDomain::Exact,
            }),
        ))
    }

    /// A resolved name of a declared symbol.
    pub(super) fn name(&mut self, name: &str) -> ExpressionHandle {
        let symbol = self.symbol(name);
        self.program
            .expression_table
            .insert_tree(&crate::expression::Expression::Name(NamePath::resolved(
                vec![Identifier::generated(name)],
                symbol,
                symbol,
            )))
    }

    pub(super) fn binary(
        &mut self,
        left: ExpressionHandle,
        operator: BinaryOperator,
        right: ExpressionHandle,
    ) -> ExpressionHandle {
        self.expression(ExpressionNode::Binary(TableBinaryExpression {
            left,
            operator,
            right,
        }))
    }

    pub(super) fn call(
        &mut self,
        receiver: ExpressionHandle,
        arguments: Vec<ExpressionHandle>,
        machine_arguments: Vec<StaticMachineArgument>,
    ) -> ExpressionHandle {
        self.call_to("size", receiver, arguments, machine_arguments)
    }

    pub(super) fn call_to(
        &mut self,
        target: &str,
        receiver: ExpressionHandle,
        arguments: Vec<ExpressionHandle>,
        machine_arguments: Vec<StaticMachineArgument>,
    ) -> ExpressionHandle {
        let arguments = self
            .program
            .expression_table
            .insert_expression_handles(arguments);
        self.expression(ExpressionNode::Call(TableCallExpression {
            authored_call_selection: None,
            authored_call_binding: None,
            build_selection: Default::default(),
            authored_machine_application: Default::default(),
            exact_coercion_target: Default::default(),
            requirement_application: Default::default(),
            receiver,
            target_symbol: self.symbol(target),
            static_machine_parameter: Default::default(),
            target: Identifier::generated(target),
            static_requirement_dispatch: None,
            machine_arguments: machine_arguments.into_boxed_slice(),
            quotient_operation: None,
            private_layout_operation: None,
            layout_query: None,
            native_code_layout: None,
            arguments,
            evidence_arguments: Box::new([]),
            operational_acknowledgement: Default::default(),
        }))
    }

    pub(super) fn render(
        &self,
        reference: TypeReferenceHandle,
        binders: &[(SymbolHandle, String)],
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    ) -> String {
        let identity = self
            .program
            .exact_owner_type_identity(ExactOwnerTypeIdentityRequest {
                type_reference: reference,
                binders,
                substitutions,
                exact_toolchain_sources: &self.toolchain,
            })
            .expect("every nominal of a witness has an exact owner");
        self.program
            .type_identity_text(identity)
            .expect("an interned identity has text")
    }
}
