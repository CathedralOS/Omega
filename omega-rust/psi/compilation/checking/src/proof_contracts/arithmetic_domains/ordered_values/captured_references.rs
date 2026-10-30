//! Ordered facts use the completed reference's actual storage identity.
//! A lexical initializer cannot stand in for a captured selector or pointer.

use super::{ExpressionHandle, Machine, Operand, State};
use facts::PlaceSegment;

pub(super) fn operand(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    frames: Option<&crate::CallFrameResolver<'_>>,
) -> Option<Operand> {
    let prepared;
    let frames = match frames {
        Some(frames) => frames,
        None => {
            prepared = crate::CallFrameResolver::new(program)?;
            &prepared
        }
    };
    let sources = frames.completed_reference_read_sources(machine, state, expression);
    if std::env::var_os("OMEGA_STRUCT_TRACE").is_some() {
        eprintln!(
            "ORDERED captured-reference machine={} expression={expression:?} node={:?} type={:?} sources={sources:?}",
            program.symbols.name(machine.symbol),
            program.expression_table.expression(expression),
            crate::expression_result_type_reference(program, machine, state, expression)
                .map(|reference| program.type_reference_table.type_reference(reference))
        );
    }
    let sources = sources?;
    let mut sources = sources.iter();
    let first_source = sources.next()?;
    let first = source_operand(program, state, first_source);
    if std::env::var_os("OMEGA_STRUCT_TRACE").is_some() {
        eprintln!(
            "ORDERED captured-reference-identity machine={} expression={expression:?} source={first_source:?} operand={first:?}",
            program.symbols.name(machine.symbol)
        );
    }
    let first = first?;
    // Alternative descriptors can share an identity only when every exact
    // supplier names that same typed storage. Unavailable routes stay opaque.
    for source in sources {
        if source_operand(program, state, source)? != first {
            return None;
        }
    }
    Some(first)
}

fn source_operand(
    program: &dyn crate::CheckingInput,
    state: &State,
    source: &crate::LocalWriteSource,
) -> Option<Operand> {
    if source.collection_coarse
        || !source.root.is_valid()
        || source.invocation_output.is_some()
        || source.is_image_storage
        || !source.recasts.is_empty()
        || source.representation.is_some()
    {
        return None;
    }
    let root_type = crate::state_root_type(program, state, source.root)?;
    let mut value = Operand::Place {
        root: source.root,
        path: program.symbols.name(source.root).to_owned(),
    };
    for (ordinal, segment) in source.segments.iter().enumerate() {
        let selected = crate::project_type_reference_from_segments(
            program,
            root_type,
            &source.segments[..=ordinal],
        )?;
        match segment {
            PlaceSegment::Case { .. } => {}
            PlaceSegment::Field { symbol } if symbol.is_valid() => {
                value = Operand::Field {
                    receiver: Box::new(value),
                    field: *symbol,
                    name: program.symbols.name(*symbol).to_owned(),
                    primitive: program.primitive_type_reference(selected),
                };
            }
            PlaceSegment::FixedIndex { index } => {
                value = Operand::Indexed {
                    collection: Box::new(value),
                    index: Box::new(Operand::Integer(
                        numerics::literals::IntegerLiteral::from_parts(
                            false,
                            numerics::literals::IntegerRadix::Decimal,
                            &index.to_string(),
                        )
                        .ok()?,
                        None,
                    )),
                    primitive: program.primitive_type_reference(selected),
                };
            }
            // A runtime selector is not its captured value. The origin owner
            // must publish an exact coordinate before it can be an identity.
            _ => return None,
        }
    }
    Some(value)
}
