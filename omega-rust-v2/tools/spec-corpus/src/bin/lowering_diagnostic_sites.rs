//! Explicit L5 constructor syntax and calls to reviewed caller-forwarding functions.
//! This is a source inventory, not name resolution or refusal-policy coverage.
#[path = "../site_scan.rs"]
mod site_scan;

use site_scan::{Args, message, rust_files, test_only};
use spec_corpus::diagnostic_field as field;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};
use syn::{
    Attribute, Expr, ExprCall, File, Item, ItemFn, Meta, Token, UseTree,
    ext::IdentExt,
    punctuated::Punctuated,
    spanned::Spanned,
    visit::{self, Visit},
};

const SOURCE: &str = "layers/05_prepared-trees-to-terminal-psi/src";
type Name = Vec<String>;
type Body = (String, usize, usize); // same coordinates as the tail constructor row
#[derive(Clone)]
struct Definition {
    arity: usize,
    body: Body,
}
enum Binding {
    Function(Option<Definition>),
    Import(Name), // target qualified in the declaring module, not the caller's module
}
type Wrappers = BTreeMap<Name, Option<Binding>>;

fn names(path: &syn::Path) -> Name {
    path.segments
        .iter()
        .map(|s| s.ident.unraw().to_string())
        .collect()
}

fn direct(name: &[String]) -> bool {
    matches!(name, [.., owner, ctor] if owner == "LoweringError" && ctor == "new")
}

fn tracked(attr: &Attribute) -> bool {
    if attr.path().is_ident("track_caller") {
        return true;
    }
    if !attr.path().is_ident("cfg_attr") {
        return false;
    }
    let Ok(parts) = attr.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated) else {
        return false;
    };
    matches!(parts.first(), Some(Meta::NameValue(n)) if n.path.is_ident("feature") &&
        matches!(&n.value, Expr::Lit(s) if matches!(&s.lit, syn::Lit::Str(v) if v.value() == "diagnostic-origins")))
        && parts
            .iter()
            .skip(1)
            .any(|m| matches!(m, Meta::Path(p) if p.is_ident("track_caller")))
}

/// Identify the particular tail constructor, not every constructor inside the function.
fn wrapper(
    path: &str,
    f: &ItemFn,
) -> Option<Definition> {
    if !matches!(f.sig.ident.unraw().to_string().as_str(), "bad" | "unsupported_node")
        || test_only(&f.attrs)
        || !f.attrs.iter().any(tracked)
        || f.sig.asyncness.is_some()
    {
        return None;
    }
    let [syn::Stmt::Expr(Expr::Call(c), None)] = &f.block.stmts[..] else {
        return None;
    };
    let Expr::Path(p) = &*c.func else { return None };
    let start = p.path.span().start();
    (p.qself.is_none() && direct(&names(&p.path))).then(|| Definition {
        body: (path.into(), start.line, start.column + 1),
        arity: f.sig.inputs.len(),
    })
}

fn register<K: Ord, V>(
    known: &mut BTreeMap<K, Option<V>>,
    name: K,
    def: Option<V>,
) {
    // E.g. two cfg alternatives: don't guess which declaration this call reaches.
    known
        .entry(name)
        .and_modify(|previous| *previous = None)
        .or_insert(def);
}

fn module_wrappers(
    path: &str,
    module: &Name,
    items: &[Item],
    known: &mut Wrappers,
) {
    for item in items {
        let mut name = module.clone();
        match item {
            Item::Fn(f) if !test_only(&f.attrs) => {
                name.push(f.sig.ident.unraw().to_string());
                register(known, name, Some(Binding::Function(wrapper(path, f))));
            }
            Item::Use(u) if !test_only(&u.attrs) && u.leading_colon.is_none() => {
                let mut aliases = BTreeMap::new();
                imports(&u.tree, &mut vec![], &mut aliases);
                for (alias, target) in aliases {
                    let mut name = module.clone();
                    name.push(alias);
                    register(known, name, target.map(|p| Binding::Import(qualified(module, &p))));
                }
            }
            Item::Mod(m) if !test_only(&m.attrs) => {
                if let Some((_, items)) = &m.content {
                    name.push(m.ident.unraw().to_string());
                    module_wrappers(path, &name, items, known);
                }
            }
            _ => {}
        }
    }
}

fn module(relative: &str) -> Name {
    let mut name: Name = relative
        .trim_end_matches(".rs")
        .split('/')
        .map(str::to_owned)
        .collect();
    if name.last().is_some_and(|n| n == "lib" || n == "mod") {
        name.pop();
    }
    name
}

fn qualified(
    module: &[String],
    segments: &[String],
) -> Name {
    let mut base = module.to_vec();
    for word in segments {
        match word.as_str() {
            "crate" => base.clear(),
            "super" => {
                if base.pop().is_none() {
                    return vec![];
                }
            }
            "self" => {}
            _ => base.push(word.clone()),
        }
    }
    base
}

fn imports(
    tree: &UseTree,
    prefix: &mut Name,
    found: &mut BTreeMap<String, Option<Name>>,
) {
    match tree {
        UseTree::Group(g) => {
            for child in &g.items {
                imports(child, prefix, found);
            }
        }
        UseTree::Path(p) => {
            prefix.push(p.ident.unraw().to_string());
            imports(&p.tree, prefix, found);
            prefix.pop();
        }
        UseTree::Name(n) => {
            let mut path = prefix.clone();
            if n.ident != "self" {
                path.push(n.ident.unraw().to_string());
            }
            if let Some(name) = path.last() {
                register(found, name.clone(), Some(path));
            }
        }
        UseTree::Rename(r) => {
            let mut path = prefix.clone();
            if r.ident != "self" {
                path.push(r.ident.unraw().to_string());
            }
            register(found, r.rename.unraw().to_string(), Some(path));
        }
        UseTree::Glob(_) => {
            found.insert("*".into(), None);
        }
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Row {
    line: usize,
    column: usize,
    role: &'static str,
    callee: String,
    class: &'static str,
    text: String,
}

struct Scanner<'a> {
    path: &'a str,
    module: Option<Name>, // None for modules local to a block (not addressable from the file root)
    known: &'a Wrappers,
    // Inner declarations win. None is a shadowing item, pattern binding or block import.
    locals: Vec<(String, Option<Definition>)>,
    used: &'a mut BTreeSet<Body>,
    rows: Vec<Row>,
}

impl Scanner<'_> {
    fn scoped(
        &mut self,
        scan: impl FnOnce(&mut Self),
    ) {
        let depth = self.locals.len();
        scan(self);
        self.locals.truncate(depth);
    }

    fn module_items(
        &mut self,
        items: &[Item],
    ) {
        for item in items {
            if let Item::Mod(m) = item {
                let child = self
                    .module
                    .as_ref()
                    .map(|n| qualified(n, &[m.ident.unraw().to_string()]));
                self.inline_module(m, child);
            } else {
                self.visit_item(item);
            }
        }
    }

    fn inline_module(
        &mut self,
        m: &syn::ItemMod,
        child: Option<Name>,
    ) {
        if let Some((_, items)) = &m.content {
            if test_only(&m.attrs) {
                return;
            }
            let module = std::mem::replace(&mut self.module, child);
            let locals = std::mem::take(&mut self.locals);
            self.module_items(items);
            self.locals = locals;
            self.module = module;
        }
    }

    fn resolve(
        &self,
        name: &[String],
    ) -> Option<&Definition> {
        let first = name.first()?;
        if let Some((_, def)) = self
            .locals
            .iter()
            .rev()
            .find(|(n, _)| n == first || (n == "*" && !matches!(first.as_str(), "crate" | "self" | "super")))
        {
            return if name.len() == 1 { def.as_ref() } else { None };
        }
        let mut path = qualified(self.module.as_ref()?, name);
        let mut followed = BTreeSet::new();
        loop {
            // A prefix can itself be an imported module. Resolve one explicit binding
            // at a time; an ambiguous binding or import cycle is not a known caller.
            let (prefix, binding) = (1..=path.len())
                .filter_map(|n| self.known.get_key_value(&path[..n]))
                // A function and a module with the same name can coexist.
                .find(|(prefix, binding)| prefix.len() == path.len() || !matches!(binding, Some(Binding::Function(_))))?;
            match binding.as_ref()? {
                Binding::Function(def) => return def.as_ref(),
                Binding::Import(target) if followed.insert(prefix) => {
                    path.splice(..prefix.len(), target.iter().cloned());
                }
                Binding::Import(_) => return None,
            }
        }
    }
}

impl<'ast> Visit<'ast> for Scanner<'_> {
    fn visit_item(
        &mut self,
        item: &'ast Item,
    ) {
        self.scoped(|s| visit::visit_item(s, item));
    }
    fn visit_item_fn(
        &mut self,
        f: &'ast ItemFn,
    ) {
        if test_only(&f.attrs) {
            return;
        }
        visit::visit_item_fn(self, f);
    }
    fn visit_block(
        &mut self,
        block: &'ast syn::Block,
    ) {
        self.scoped(|s| {
            // Items are visible even before their declaration; lets only after their initializer.
            let mut items = BTreeMap::new();
            for stmt in &block.stmts {
                let syn::Stmt::Item(item) = stmt else { continue };
                let id = match item {
                    Item::Fn(f) if !test_only(&f.attrs) => {
                        register(&mut items, f.sig.ident.unraw().to_string(), wrapper(s.path, f));
                        continue;
                    }
                    Item::Const(i) => &i.ident,
                    Item::Static(i) => &i.ident,
                    Item::Struct(i) => &i.ident,
                    Item::Mod(i) => &i.ident,
                    Item::Enum(i) => &i.ident,
                    Item::Type(i) => &i.ident,
                    Item::Use(u) => {
                        let mut aliases = BTreeMap::new();
                        imports(&u.tree, &mut vec![], &mut aliases);
                        for name in aliases.into_keys() {
                            register(&mut items, name, None);
                        }
                        continue;
                    }
                    _ => continue,
                };
                register(&mut items, id.unraw().to_string(), None);
            }
            s.locals.extend(items);
            for stmt in &block.stmts {
                s.visit_stmt(stmt);
            }
        });
    }
    fn visit_local(
        &mut self,
        local: &'ast syn::Local,
    ) {
        if let Some(init) = &local.init {
            self.visit_local_init(init);
        }
        self.visit_pat(&local.pat);
    }
    fn visit_pat_ident(
        &mut self,
        pat: &'ast syn::PatIdent,
    ) {
        self.locals.push((pat.ident.unraw().to_string(), None));
        visit::visit_pat_ident(self, pat);
    }
    fn visit_generic_param(
        &mut self,
        param: &'ast syn::GenericParam,
    ) {
        match param {
            syn::GenericParam::Type(t) => self.locals.push((t.ident.unraw().to_string(), None)),
            syn::GenericParam::Const(c) => self.locals.push((c.ident.unraw().to_string(), None)),
            _ => {}
        }
        visit::visit_generic_param(self, param);
    }
    fn visit_arm(
        &mut self,
        arm: &'ast syn::Arm,
    ) {
        self.scoped(|s| visit::visit_arm(s, arm));
    }
    fn visit_expr(
        &mut self,
        expr: &'ast Expr,
    ) {
        match expr {
            Expr::If(e) => {
                self.scoped(|s| {
                    s.visit_expr(&e.cond);
                    s.visit_block(&e.then_branch);
                });
                if let Some((_, other)) = &e.else_branch {
                    self.visit_expr(other);
                }
            }
            Expr::Let(e) => {
                self.visit_expr(&e.expr);
                self.visit_pat(&e.pat);
            }
            Expr::ForLoop(e) => {
                self.visit_expr(&e.expr);
                self.scoped(|s| {
                    s.visit_pat(&e.pat);
                    s.visit_block(&e.body);
                });
            }
            Expr::Closure(_) | Expr::While(_) => self.scoped(|s| visit::visit_expr(s, expr)),
            _ => visit::visit_expr(self, expr),
        }
    }
    fn visit_item_mod(
        &mut self,
        m: &'ast syn::ItemMod,
    ) {
        // Module items go through module_items; this one was nested in a block.
        self.inline_module(m, None);
    }
    fn visit_item_impl(
        &mut self,
        i: &'ast syn::ItemImpl,
    ) {
        if !test_only(&i.attrs) {
            visit::visit_item_impl(self, i);
        }
    }
    fn visit_impl_item_fn(
        &mut self,
        f: &'ast syn::ImplItemFn,
    ) {
        if !test_only(&f.attrs) {
            self.scoped(|s| visit::visit_impl_item_fn(s, f));
        }
    }
    fn visit_macro(
        &mut self,
        m: &'ast syn::Macro,
    ) {
        if let Ok(args) = m.parse_body_with(Args::parse_terminated) {
            for arg in &args {
                self.visit_expr(arg);
            }
        }
    }
    fn visit_expr_call(
        &mut self,
        call: &'ast ExprCall,
    ) {
        if let Expr::Path(p) = &*call.func {
            let name = names(&p.path);
            let role = if direct(&name) && p.qself.is_none() {
                Some("constructor")
            } else if p.qself.is_none() && p.path.leading_colon.is_none() {
                let body = self
                    .resolve(&name)
                    .filter(|def| def.arity == call.args.len())
                    .map(|def| def.body.clone());
                body.map(|body| {
                    self.used.insert(body);
                    "wrapper"
                })
            } else {
                None
            };
            if let Some(role) = role {
                let start = p.path.span().start();
                let (class, text) = call
                    .args
                    .last()
                    .map(message)
                    .unwrap_or(("dynamic", String::new()));
                self.rows.push(Row {
                    line: start.line,
                    column: start.column + 1,
                    role,
                    callee: name.join("::"),
                    class,
                    text,
                });
            }
        }
        visit::visit_expr_call(self, call);
    }
}

fn inventory(files: &BTreeMap<String, File>) -> Vec<(&str, Row)> {
    let mut known = Wrappers::new();
    for (path, file) in files {
        module_wrappers(path, &module(path), &file.items, &mut known);
    }
    let mut rows = Vec::new();
    let mut used = BTreeSet::new();
    for (path, file) in files {
        let mut s = Scanner {
            path,
            module: Some(module(path)),
            known: &known,
            used: &mut used,
            locals: vec![],
            rows: vec![],
        };
        s.module_items(&file.items);
        s.rows.sort();
        rows.extend(s.rows.into_iter().map(|row| (path.as_str(), row)));
    }
    // Only replace a constructor when we have a caller candidate for that exact body.
    for (path, row) in &mut rows {
        if row.role == "constructor" && used.contains(&(path.to_string(), row.line, row.column)) {
            row.role = "forwarder-body";
        }
    }
    rows
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new(SOURCE);
    let mut paths = vec![];
    rust_files(root, &mut paths)?;
    let mut files = BTreeMap::new();
    for path in paths {
        let relative = path
            .strip_prefix(root)?
            .to_str()
            .ok_or("non-UTF8 path")?
            .replace('\\', "/");
        if relative.split('/').any(|p| p == "tests" || p == "tests.rs") {
            continue;
        }
        let file = syn::parse_file(&fs::read_to_string(&path)?).map_err(|e| format!("{relative}: {e}"))?;
        if !test_only(&file.attrs) {
            files.insert(relative, file);
        }
    }
    let rows = inventory(&files);
    println!("#lowering-diagnostic-sites\tv1\npath\tline\tcolumn\trole\tcallee\tclass\ttext");
    for (path, row) in &rows {
        println!(
            "{SOURCE}/{}\t{}\t{}\t{}\t{}\t{}\t{}",
            field(path),
            row.line,
            row.column,
            row.role,
            field(&row.callee),
            row.class,
            field(&row.text)
        );
    }
    println!("#end\tsites={}\tfiles={}", rows.len(), files.len());
    Ok(())
}

#[cfg(test)]
#[path = "lowering_diagnostic_sites/probe.rs"]
mod probe;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scanner_keys_match_real_track_caller_locations() {
        let files = BTreeMap::from([("probe.rs".into(), syn::parse_file(include_str!("lowering_diagnostic_sites/probe.rs")).unwrap())]);
        let rows = inventory(&files);
        let observed = probe::run().map(|e| {
            assert!(Path::new(e.0.file()).ends_with("lowering_diagnostic_sites/probe.rs"));
            (e.0.line() as usize, e.0.column() as usize)
        });
        let keys: Vec<_> = rows
            .iter()
            .filter(|(_, r)| r.role != "forwarder-body")
            .map(|(_, r)| (r.line, r.column))
            .collect();
        assert_eq!(keys, observed);
        assert_eq!(rows.iter().filter(|(_, r)| r.role == "forwarder-body").count(), 3);
        let wrappers: Vec<_> = rows
            .iter()
            .filter(|(_, r)| r.role == "wrapper")
            .map(|(_, r)| (r.callee.as_str(), r.text.as_str()))
            .collect();
        assert_eq!(
            wrappers,
            [
                ("bad", "within inline module"),
                ("forwarded", "through parent import"),
                ("self::bad", "qualified wrapper"),
                ("inner::bad", "qualified inline module helper"),
                ("bad", "block fn, before its declaration")
            ]
        );
    }

    #[test]
    fn unresolved_calls_do_not_replace_the_constructor() {
        let file = syn::parse_file(
            r#"
            mod inner {
                #[track_caller] fn bad(r: &str) -> E { LoweringError::new(s, r) }
            }
            use self::inner::bad as alias;
            fn shadows(r#alias: F) { alias("param"); }
            fn f() {
                { let (alias,) = callbacks(); alias("local"); }
                { use elsewhere::alias; alias("import"); }
                { use elsewhere::*; alias("glob"); }
                let _ = |alias: F| alias("closure input");
                match x { Some(alias) => alias("match binding"), _ => () }
                if let Some(alias) = x { alias("conditional"); }
                <inner as Other>::bad("associated function");
                ::inner::bad("external crate"); inner::badly("similar"); inner::bad();
                { mod inner { pub fn bad(r: &str) {} } inner::bad("block module"); }
                { #[track_caller] fn unsupported_node(r: &str) -> E { LoweringError::new(s, r) }
                  let callback = unsupported_node; callback("indirect"); }
            }
            fn other<inner: Trait>() { inner::bad("generic"); unsupported_node("out of scope"); }
            mod sibling { fn g() { alias("parent import not inherited"); } }
            mod cycle {
                use self::b as a;
                use self::a as b;
                fn f() { a("cyclic import"); }
            }
            mod uncertain {
                #[cfg(feature="alt")] use elsewhere::alias;
                #[cfg(not(feature="alt"))] use super::inner::bad as alias;
                fn g() { alias("ambiguous import"); }
            }
        "#,
        )
        .unwrap();
        let files = BTreeMap::from([("lib.rs".into(), file)]);
        let rows = inventory(&files);
        assert_eq!(rows.iter().map(|(_, r)| r.role).collect::<Vec<_>>(), ["constructor", "constructor"]);
    }

    #[test]
    fn follows_known_wrapper_import_but_not_strings_tests_or_plain_bad_closures() {
        let files = BTreeMap::from([
            (
                "machines/body.rs".into(),
                syn::parse_file(
                    r#"#[cfg_attr(feature = "diagnostic-origins", track_caller)]
             fn unsupported_node(s: Site, reason: &str) -> LoweringError { LoweringError::new(s, reason) }"#,
                )
                .unwrap(),
            ),
            (
                "machines/call.rs".into(),
                syn::parse_file(
                    r#"use super::body::{unsupported_node as known};
             fn f() { known(s, "bad"); let bad = || LoweringError::new(s, format!("{why}")); bad();
                      let _ = "LoweringError::new()"; } #[test] fn t() { known(s, "test"); }"#,
                )
                .unwrap(),
            ),
        ]);
        let rows = inventory(&files);
        assert_eq!(
            rows.iter().map(|(_, r)| (r.role, r.class)).collect::<Vec<_>>(),
            [
                ("forwarder-body", "dynamic"),
                ("wrapper", "literal"),
                ("constructor", "template")
            ]
        );
    }
}
