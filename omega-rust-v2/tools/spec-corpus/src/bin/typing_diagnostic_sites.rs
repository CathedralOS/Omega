//! Test tooling: list explicit `TypeError` constructor calls in the L3 sources as a sorted TSV.
//! Run from the V2 workspace root:
//! `mbx run --locked -p spec-corpus --features typing-sites --bin typing_diagnostic_sites`.
//!
//! A row is an explicit call site, not a distinct reason and not runtime coverage. Limits: names are not
//! resolved (aliases, `Self::` calls and imported constructors are missed), `cfg` is only read for `test`,
//! macro bodies are inspected only when they parse as comma-separated expressions, and a forwarded or
//! computed message is reported as dynamic. Other `TypeError::` constructors and constructor references
//! without a call are listed as unclassified for review.
use std::{fs, path::Path, process::ExitCode};

use proc_macro2::Span;
#[path = "../site_scan.rs"]
mod site_scan;
use site_scan::{Args, message, rust_files, test_only};
use syn::{
    Attribute, Expr, Type,
    spanned::Spanned,
    visit::{self, Visit},
};

const SOURCE: &str = "layers/03_symbol-resolved-trees-to-typed-trees/src";
const HEADER: &str = "#typing-diagnostic-sites\tv1\npath\tline\tcolumn\tscope\tconstructor\tclass\ttext";

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Site {
    path: String,
    line: usize,
    column: usize,
    scope: String,
    constructor: String,
    class: &'static str,
    text: String,
}

#[derive(Default)]
struct Sites {
    path: String,
    scope: Vec<String>,
    rows: Vec<Site>,
    excluded: usize,
}

fn type_name(ty: &Type) -> Option<String> {
    match ty {
        Type::Path(path) => path.path.segments.last().map(|s| s.ident.to_string()),
        _ => None,
    }
}

/// The constructor name for `TypeError::name`, `a::TypeError::name` or `<TypeError as T>::name`.
fn constructor(path: &syn::ExprPath) -> Option<String> {
    let segments: Vec<String> = path.path.segments.iter().map(|s| s.ident.to_string()).collect();
    let owner = match &path.qself {
        Some(qself) => type_name(&qself.ty),
        None => segments.len().checked_sub(2).map(|i| segments[i].clone()),
    };
    (owner.as_deref() == Some("TypeError"))
        .then(|| segments.last().cloned())
        .flatten()
}

fn classify(
    name: &str,
    args: &Args,
) -> (&'static str, String) {
    match (name, args.len()) {
        ("at", 3) | ("for_site", 2) | ("for_reason", 2) | ("new", 4) => message(args.last().expect("argument count checked")),
        // The production fallback template is not a rule; it is reported once per call site.
        ("unsupported", 2) => ("unsupported", String::new()),
        ("waiting", 2) => ("pending", String::new()),
        _ => ("unclassified", format!("{} argument(s) to an unrecognised constructor form", args.len())),
    }
}

impl Sites {
    fn push(
        &mut self,
        span: Span,
        constructor: &str,
        class: &'static str,
        text: String,
    ) {
        let start = span.start();
        let scope = if self.scope.is_empty() { "-".into() } else { self.scope.join("::") };
        let (path, constructor) = (self.path.clone(), constructor.into());
        self.rows.push(Site {
            path,
            line: start.line,
            column: start.column + 1,
            scope,
            constructor,
            class,
            text,
        });
    }

    fn scoped(
        &mut self,
        attrs: &[Attribute],
        name: &syn::Ident,
        walk: impl FnOnce(&mut Self),
    ) {
        if test_only(attrs) {
            self.excluded += 1;
            return;
        }
        self.scope.push(name.to_string());
        walk(self);
        self.scope.pop();
    }
}

impl<'ast> Visit<'ast> for Sites {
    fn visit_item_fn(
        &mut self,
        item: &'ast syn::ItemFn,
    ) {
        self.scoped(&item.attrs, &item.sig.ident, |s| visit::visit_item_fn(s, item));
    }
    fn visit_impl_item_fn(
        &mut self,
        item: &'ast syn::ImplItemFn,
    ) {
        self.scoped(&item.attrs, &item.sig.ident, |s| visit::visit_impl_item_fn(s, item));
    }
    fn visit_trait_item_fn(
        &mut self,
        item: &'ast syn::TraitItemFn,
    ) {
        self.scoped(&item.attrs, &item.sig.ident, |s| visit::visit_trait_item_fn(s, item));
    }
    fn visit_item_mod(
        &mut self,
        item: &'ast syn::ItemMod,
    ) {
        self.scoped(&item.attrs, &item.ident, |s| visit::visit_item_mod(s, item));
    }
    fn visit_item_impl(
        &mut self,
        item: &'ast syn::ItemImpl,
    ) {
        // Constructor definitions (`impl .. for TypeError`) are not call sites of diagnostics.
        if test_only(&item.attrs) || type_name(&item.self_ty).as_deref() == Some("TypeError") {
            self.excluded += 1;
        } else {
            visit::visit_item_impl(self, item);
        }
    }
    fn visit_expr_call(
        &mut self,
        call: &'ast syn::ExprCall,
    ) {
        let found = match &*call.func {
            Expr::Path(path) => constructor(path).map(|name| (path, name)),
            _ => None,
        };
        let Some((path, name)) = found else {
            return visit::visit_expr_call(self, call);
        };
        let (class, text) = classify(&name, &call.args);
        self.push(path.span(), &name, class, text);
        call.args.iter().for_each(|arg| self.visit_expr(arg));
    }
    fn visit_expr_path(
        &mut self,
        path: &'ast syn::ExprPath,
    ) {
        if let Some(name) = constructor(path) {
            self.push(path.span(), &name, "unclassified", "constructor reference without a direct call".into());
        }
        visit::visit_expr_path(self, path);
    }
    fn visit_macro(
        &mut self,
        mac: &'ast syn::Macro,
    ) {
        match mac.parse_body_with(Args::parse_terminated) {
            Ok(args) => args.iter().for_each(|arg| self.visit_expr(arg)),
            Err(_) if mac.tokens.to_string().contains("TypeError") => {
                self.push(
                    mac.path.span(),
                    "-",
                    "unclassified",
                    "macro body mentions TypeError but was not inspected".into(),
                );
            }
            Err(_) => {}
        }
        visit::visit_macro(self, mac);
    }
}

/// Rows and the number of excluded test-only items or constructor definitions in one file.
fn inventory(
    path: &str,
    text: &str,
) -> (Vec<Site>, usize) {
    let mut sites = Sites {
        path: path.into(),
        ..Sites::default()
    };
    match syn::parse_file(text) {
        Ok(file) if test_only(&file.attrs) => return (Vec::new(), 1),
        Ok(file) => sites.visit_file(&file),
        Err(error) => sites.push(error.span(), "-", "malformed", error.to_string()),
    }
    (sites.rows, sites.excluded)
}

use spec_corpus::diagnostic_field as field;

fn main() -> ExitCode {
    let root = Path::new(SOURCE);
    let mut files = Vec::new();
    if let Err(error) = rust_files(root, &mut files) {
        eprintln!("cannot list {SOURCE}: {error}");
        return ExitCode::FAILURE;
    }
    let (mut rows, mut excluded) = (Vec::new(), 0);
    for file in &files {
        let relative = file
            .strip_prefix(root)
            .expect("listed below the root")
            .to_string_lossy()
            .replace('\\', "/");
        let Ok(text) = fs::read_to_string(file) else {
            eprintln!("cannot read {SOURCE}/{relative}");
            return ExitCode::FAILURE;
        };
        let (found, skipped) = inventory(&relative, &text);
        rows.extend(found);
        excluded += skipped;
    }
    rows.sort();
    println!("{HEADER}");
    for r in &rows {
        let (scope, text) = (field(&r.scope), field(&r.text));
        println!("{}\t{}\t{}\t{scope}\t{}\t{}\t{text}", field(&r.path), r.line, r.column, r.constructor, r.class);
    }
    println!("#end\tsites={}\tfiles={}\texcluded_items={excluded}", rows.len(), files.len());
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::inventory;

    fn classes(text: &str) -> Vec<(String, &'static str, String)> {
        inventory("x.rs", text)
            .0
            .into_iter()
            .map(|s| (s.constructor, s.class, s.text))
            .collect()
    }

    #[test]
    fn classifies_explicit_calls() {
        let rows = classes(concat!(
            "fn f() {\n",
            "    // TypeError::at(file, n, \"comment\")\n",
            "    let _ = r#\"TypeError::at(file, n, \"raw\")\"#;\n",
            "    TypeError::at(file, n, \"plain\");\n",
            "    omega_typed::TypeError::for_site(site, format!(\"bad {x}\"));\n",
            "    <TypeError as SourceTypeError>::at(file, n, r#\"raw \"msg\"\"#.to_string());\n",
            "    e.map_err(|e| TypeError::new(e.package, e.file, e.span, e.message));\n",
            "    TypeError::unsupported(file, n); TypeError::waiting(file, n);\n",
            "    TypeError::other(file); x.map(TypeError::unsupported);\n",
            "    vec![TypeError::at(file, n, \"in macro\")];\n",
            "}\n",
        ));
        let expected = [
            ("at", "literal", "plain"),
            ("for_site", "template", "bad {x}"),
            ("at", "literal", "raw \"msg\""),
            ("new", "dynamic", "e . message"),
            ("unsupported", "unsupported", ""),
            ("waiting", "pending", ""),
            ("other", "unclassified", "1 argument(s) to an unrecognised constructor form"),
            ("unsupported", "unclassified", "constructor reference without a direct call"),
            ("at", "literal", "in macro"),
        ];
        let rows: Vec<_> = rows
            .iter()
            .map(|(c, k, t)| (c.as_str(), *k, t.as_str()))
            .collect();
        assert_eq!(rows, expected);
        let site = &inventory("x.rs", "fn f() {\n    TypeError::at(a, b, \"m\");\n}").0[0];
        assert_eq!((site.line, site.column, site.scope.as_str()), (2, 5, "f"));
    }

    #[test]
    fn excludes_tests_definitions_and_reports_limits() {
        let text = concat!(
            "#[cfg(test)] mod tests { fn t() { TypeError::at(f, n, \"t\"); } }\n",
            "#[test] fn u() { TypeError::at(f, n, \"u\"); }\n",
            "impl SourceTypeError for TypeError { fn at() { TypeError::new(a, b, c, d) } }\n",
            "#[cfg(not(test))] fn p() { TypeError::waiting(f, n); }\n",
            "macro_rules! m { () => { TypeError::at(f, n, \"m\") }; }\n",
        );
        let (rows, excluded) = inventory("x.rs", text);
        assert_eq!(excluded, 3);
        let found: Vec<_> = rows
            .iter()
            .map(|s| (s.constructor.as_str(), s.class, s.scope.as_str()))
            .collect();
        assert_eq!(found, [("waiting", "pending", "p"), ("-", "unclassified", "-")]);
        assert_eq!(inventory("x.rs", "#![cfg(test)]\nfn f() { TypeError::at(f, n, \"x\"); }").1, 1);
        let malformed = inventory("bad.rs", "fn f( {").0;
        assert_eq!((malformed.len(), malformed[0].class), (1, "malformed"));
        assert_eq!(super::field("a\tb%\n"), "a%09b%25%0A");
    }
}
