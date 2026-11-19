use std::collections::{HashMap, HashSet};

use omega_lexer::{SourceSpan, Token, TokenKind};
use omega_parser::{NodeKind, TreeEvent};

use crate::{MachineIdentity, ParsedSource, ResolutionError, ResolvedCall};

struct CallSite<'a> {
    logical_path: &'a str,
    directory: &'a str,
    name: &'a str,
    span: SourceSpan,
}

/// Binds direct, unqualified machine calls in the supplied package. Declarations
/// are collected before any call is bound, so forward and sibling calls behave
/// identically. Each pass visits files by logical path and occurrences in source order.
/// See CONTRACT.md for the supported syntax and the admission boundary.
pub fn resolve_machine_calls<'a>(
    package: &'a str,
    sources: &[ParsedSource<'a, '_>],
) -> Result<Vec<ResolvedCall<'a>>, ResolutionError<'a>> {
    let mut files: Vec<_> = sources.iter().collect();
    files.sort_unstable_by_key(|source| source.logical_path);
    let mut declarations = HashMap::new();
    let mut call_sites = Vec::new();
    let mut previous_path = None;
    for file in files {
        if previous_path == Some(file.logical_path) {
            return Err(ResolutionError::DuplicateFile {
                logical_path: file.logical_path,
            });
        }
        previous_path = Some(file.logical_path);
        collect_names(file, &mut declarations, &mut call_sites)?;
    }
    call_sites
        .into_iter()
        .map(|call| match declarations.get(&(call.directory, call.name)) {
            Some(1) => Ok(ResolvedCall {
                logical_path: call.logical_path,
                span: call.span,
                target: MachineIdentity {
                    package,
                    directory: call.directory,
                    name: call.name,
                },
            }),
            Some(_) => Err(ResolutionError::CompetingMachines {
                logical_path: call.logical_path,
                span: call.span,
                name: call.name,
            }),
            None => Err(ResolutionError::Unresolved {
                logical_path: call.logical_path,
                span: call.span,
                name: call.name,
            }),
        })
        .collect()
}

fn collect_names<'a>(
    source: &ParsedSource<'a, '_>,
    declarations: &mut HashMap<(&'a str, &'a str), usize>,
    calls: &mut Vec<CallSite<'a>>,
) -> Result<(), ResolutionError<'a>> {
    use NodeKind::*;
    let path = source.logical_path;
    let directory = path.rsplit_once('/').map_or("", |(directory, _)| directory);
    let events = source.tree.get_events();
    let mut token_indices = source.tree.get_meaningful_token_indices().iter();
    let mut parents = Vec::new();
    let mut machine_parameters = HashSet::new();
    for (position, event) in events.iter().enumerate() {
        match *event {
            TreeEvent::Open(kind) => {
                let parent = parents.last().copied();
                // A plain path has exactly one meaningful token between Open and Close.
                let single_name = events.get(position + 2) == Some(&TreeEvent::Close);
                let supported = match kind {
                    File => parent.is_none(),
                    Machine => {
                        machine_parameters.clear();
                        parent == Some(File)
                    }
                    Path => parent == Some(Machine) && single_name,
                    Call => events.get(position + 1) == Some(&TreeEvent::Open(PathExpression)),
                    PathExpression => single_name,
                    Params | Param | Result | PathType | Body | Tail | Literal | Args | Paren | ExpressionStatement | Binary | Unary => true,
                    _ => false,
                };
                if !supported {
                    let span = match token_indices.as_slice().first() {
                        Some(&index) => source
                            .tokens
                            .get(index as usize)
                            .ok_or(ResolutionError::InconsistentParse { logical_path: path })?
                            .get_span(),
                        None => SourceSpan::new(source.text.len(), source.text.len()),
                    };
                    return Err(ResolutionError::Unsupported {
                        logical_path: path,
                        span,
                        syntax: kind,
                    });
                }
                parents.push(kind);
            }
            TreeEvent::Close => {
                parents.pop();
            }
            TreeEvent::Token => {
                let token: &Token = token_indices
                    .next()
                    .and_then(|&index| source.tokens.get(index as usize))
                    .ok_or(ResolutionError::InconsistentParse { logical_path: path })?;
                // The parameter owns its name and colon. Its type is a child node.
                let is_parameter_name = parents.last() == Some(&Param) && token.get_kind() == TokenKind::Word;
                let is_declaration = parents.last() == Some(&Path);
                let is_callee = parents.ends_with(&[Call, PathExpression]);
                if !(is_declaration || is_callee || is_parameter_name) {
                    continue;
                }
                let span = token.get_span();
                let name = source
                    .text
                    .get(span.get_start_offset()..span.get_end_offset())
                    .filter(|_| token.get_kind() == TokenKind::Word)
                    .ok_or(ResolutionError::InconsistentParse { logical_path: path })?;
                if is_parameter_name {
                    machine_parameters.insert(name);
                } else if is_declaration {
                    *declarations.entry((directory, name)).or_default() += 1;
                } else {
                    if machine_parameters.contains(name) {
                        return Err(ResolutionError::Unsupported {
                            logical_path: path,
                            span,
                            syntax: Call,
                        });
                    }
                    calls.push(CallSite {
                        logical_path: path,
                        directory,
                        name,
                        span,
                    });
                }
            }
        }
    }
    Ok(())
}
