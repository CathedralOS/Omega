//! Shared syntax-only inventory helpers for the L3 and L5 binaries.
use quote::ToTokens;
use std::{
    fs,
    path::{Path, PathBuf},
};
use syn::{Attribute, Expr, ExprLit, Lit, Meta, Token, punctuated::Punctuated};

pub type Args = Punctuated<Expr, Token![,]>;

/// `#[test]`, `#[cfg(test)]` and `#[cfg(all(test, ..))]`; `not` or `any` keeps the item for review.
pub fn test_only(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| match &attr.meta {
        Meta::Path(path) => path.is_ident("test"),
        Meta::List(list) if list.path.is_ident("cfg") => {
            let text = list.tokens.to_string();
            let words: Vec<&str> = text
                .split(|c: char| !(c.is_alphanumeric() || c == '_'))
                .collect();
            words.contains(&"test") && !words.iter().any(|w| *w == "not" || *w == "any")
        }
        _ => false,
    })
}

pub fn message(expr: &Expr) -> (&'static str, String) {
    let dynamic = || ("dynamic", expr.to_token_stream().to_string());
    match expr {
        Expr::Lit(ExprLit { lit: Lit::Str(text), .. }) => ("literal", text.value()),
        Expr::Paren(inner) => message(&inner.expr),
        Expr::MethodCall(call) if call.args.is_empty() && ["into", "to_string", "to_owned"].contains(&call.method.to_string().as_str()) => {
            match message(&call.receiver) {
                ("literal", text) => ("literal", text),
                _ => dynamic(),
            }
        }
        Expr::Macro(mac)
            if mac
                .mac
                .path
                .segments
                .last()
                .is_some_and(|s| s.ident == "format") =>
        {
            match mac
                .mac
                .parse_body_with(Args::parse_terminated)
                .ok()
                .as_ref()
                .and_then(|a| a.first())
            {
                Some(Expr::Lit(ExprLit { lit: Lit::Str(text), .. })) => ("template", text.value()),
                _ => dynamic(),
            }
        }
        _ => dynamic(),
    }
}

pub fn rust_files(
    dir: &Path,
    out: &mut Vec<PathBuf>,
) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let (kind, path) = (entry.file_type()?, entry.path());
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if fs::symlink_metadata(&path)?.file_attributes() & 0x400 != 0 {
                continue;
            } // reparse point
        }
        if kind.is_dir() {
            rust_files(&path, out)?;
        } else if kind.is_file() && path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
    Ok(())
}
