//! Where scripts appear in Nix code, and which language they're in.

use super::{Kind, Lang, current_dir, nixstr::Script};
use crate::utils;
use rnix::{
    SyntaxKind, SyntaxNode, TextRange,
    ast::{self, AttrpathValue, Expr},
};
use rowan::ast::AstNode as _;
use std::path::{Path, PathBuf};

/// How deep `${snippet}` references are followed.
const MAX_DEPTH: usize = 4;

/// stdenv phases and hooks, all run by bash inside stdenv's setup script.
const STDENV_HOOKS: &[&str] = &[
    "unpackPhase",
    "patchPhase",
    "configurePhase",
    "buildPhase",
    "checkPhase",
    "installPhase",
    "fixupPhase",
    "installCheckPhase",
    "distPhase",
    "preUnpack",
    "postUnpack",
    "prePatch",
    "postPatch",
    "preConfigure",
    "postConfigure",
    "preBuild",
    "postBuild",
    "preCheck",
    "postCheck",
    "preInstall",
    "postInstall",
    "preFixup",
    "postFixup",
    "preInstallCheck",
    "postInstallCheck",
    "preDist",
    "postDist",
    "shellHook",
];

const SYSTEMD_SCRIPTS: &[&str] = &[
    "script",
    "preStart",
    "postStart",
    "preStop",
    "postStop",
    "reload",
];

const SCRIPT_EXTENSIONS: &[&str] = &["sh", "bash", "py"];

/// Last name of `f` / `pkgs.writers.writeBash`.
pub fn fn_name(e: &Expr) -> Option<String> {
    match e {
        Expr::Ident(i) => Some(i.syntax().text().to_string()),
        Expr::Select(s) => s.attrpath()?.attrs().last().map(|a| a.syntax().to_string()),
        Expr::Paren(p) => fn_name(&p.expr()?),
        _ => None,
    }
}

fn shell_dialect(prog: &str) -> Option<&'static str> {
    match prog {
        "bash" => Some("bash"),
        "sh" => Some("sh"),
        "dash" => Some("dash"),
        "ksh" | "mksh" => Some("ksh"),
        _ => None,
    }
}

fn program_lang(prog: &str) -> Option<Lang> {
    if let Some(shell) = shell_dialect(prog) {
        Some(Lang::Shell(shell))
    } else if prog.starts_with("python") {
        Some(Lang::Python)
    } else {
        None
    }
}

/// Language from a `#!` line, including `#!nix-shell -i <interpreter>`.
pub fn shebang_lang(text: &str) -> Option<Lang> {
    let mut lines = text.lines().filter(|l| !l.trim().is_empty());
    let first = lines.next()?.trim().strip_prefix("#!")?;
    let mut words = first.split_whitespace();
    let mut prog = words.next()?.rsplit('/').next()?;
    if prog == "env" {
        prog = words.find(|w| !w.starts_with('-'))?;
    }
    if prog == "nix-shell" {
        let directive = lines.next()?.trim().strip_prefix("#!")?;
        let mut words = directive.split_whitespace();
        words.find(|w| *w == "-i")?;
        return program_lang(words.next()?);
    }
    program_lang(prog)
}

/// Language of a script file, from its extension or shebang.
pub fn file_lang(path: &Path) -> Option<Lang> {
    let ext = path.extension().and_then(|e| e.to_str());
    if ext.is_some_and(|e| !SCRIPT_EXTENSIONS.contains(&e)) {
        return None;
    }
    let head: String = std::fs::read_to_string(path)
        .ok()?
        .lines()
        .take(3)
        .collect::<Vec<_>>()
        .join("\n");
    match (ext, shebang_lang(&head)) {
        (_, Some(lang)) => Some(lang),
        (Some("py"), None) => Some(Lang::Python),
        (Some(_), None) => Some(Lang::Shell("bash")),
        (None, None) => None,
    }
}

/// Language run by a devenv `package`, e.g. `pkgs.python3`.
fn package_lang(package: &str) -> Option<Lang> {
    let name = package.trim().rsplit('.').next()?;
    match name {
        "bash" | "bashInteractive" => Some(Lang::Shell("bash")),
        n => program_lang(n),
    }
}

/// devenv options whose value is a script, with the language they run in.
fn devenv_context(apv: &AttrpathValue, path: &[&str]) -> Option<(Lang, Kind)> {
    let is_script = matches!(
        path,
        ["scripts" | "processes", _, "exec"]
            | ["tasks", _, "exec" | "status"]
            | ["enterShell" | "enterTest"]
    );
    if !is_script {
        return None;
    }
    let lang = match utils::sibling_package(apv) {
        None => Lang::Shell("bash"),
        Some(package) => package_lang(&package)?,
    };
    Some((lang, Kind::Script))
}

/// Options whose value is a script: devenv, NixOS systemd and activation
/// scripts, stdenv phases.
fn option_context(node: &SyntaxNode) -> Option<(Lang, Kind)> {
    let apv = node.ancestors().find_map(AttrpathValue::cast)?;
    // `let x = ...` bindings aren't options
    if apv.syntax().parent()?.kind() == SyntaxKind::NODE_LET_IN {
        return None;
    }
    let path = utils::enclosing_attrpath(apv.syntax())?;
    let path: Vec<&str> = path.iter().map(String::as_str).collect();
    let path = path.strip_prefix(&["config"]).unwrap_or(&path);
    if let Some(context) = devenv_context(&apv, path) {
        return Some(context);
    }

    let n = path.len();
    let last = *path.last()?;
    let bash = Lang::Shell("bash");
    let systemd = n >= 4
        && SYSTEMD_SCRIPTS.contains(&last)
        && path[n - 3] == "services"
        && (path[n - 4] == "systemd"
            || (n >= 5 && path[n - 4] == "user" && path[n - 5] == "systemd"));
    if systemd {
        return Some((bash, Kind::Script));
    }
    if let Some(i) = path.iter().position(|k| *k == "activationScripts")
        && i > 0
        && path[i - 1] == "system"
        && (n == i + 2 || (n == i + 3 && last == "text"))
    {
        return Some((bash, Kind::Hook));
    }
    STDENV_HOOKS.contains(&last).then_some((bash, Kind::Hook))
}

/// `pkgs.writeShellScript "name" <node>`, `runCommand "name" { } <node>` and
/// friends. `text` is the script, for shebang based writers.
fn writer_context(node: &SyntaxNode, text: &str) -> Option<(Lang, Kind)> {
    let parent = node.parent()?;

    // writeShellApplication { text = <node>; }
    if let Some(apv) = AttrpathValue::cast(parent.clone()) {
        let keys: Vec<_> = apv.attrpath()?.attrs().collect();
        let [key] = keys.as_slice() else { return None };
        if utils::attr_name(key)? != "text" {
            return None;
        }
        let apply = ast::Apply::cast(apv.syntax().parent()?.parent()?)?;
        return (fn_name(&apply.lambda()?)? == "writeShellApplication")
            .then_some((Lang::Shell("bash"), Kind::Script));
    }

    // f arg... <node>
    let apply = ast::Apply::cast(parent)?;
    if apply.argument()?.syntax() != node {
        return None;
    }
    let mut args = 1;
    let mut f = apply.lambda()?;
    while let Expr::Apply(inner) = f {
        args += 1;
        f = inner.lambda()?;
    }
    let script = (Lang::Shell("bash"), Kind::Script);
    match (fn_name(&f)?.as_str(), args) {
        ("writeShellScript" | "writeShellScriptBin" | "writeBash" | "writeBashBin", 2) => {
            Some(script)
        }
        ("writeDash" | "writeDashBin", 2) => Some((Lang::Shell("dash"), Kind::Script)),
        ("writeScript" | "writeScriptBin", 2) => Some((shebang_lang(text)?, Kind::Script)),
        ("writePython3" | "writePython3Bin" | "writePyPy3" | "writePyPy3Bin", 3) => {
            Some((Lang::Python, Kind::Script))
        }
        (
            "runCommand"
            | "runCommandLocal"
            | "runCommandCC"
            | "runCommandNoCC"
            | "runCommandNoCCLocal",
            3,
        ) => Some((Lang::Shell("bash"), Kind::Hook)),
        _ => None,
    }
}

/// Where `node` (a string, path or `readFile` call) is used as a script.
fn usage_context(node: &SyntaxNode, text: &str) -> Option<(Lang, Kind)> {
    writer_context(node, text).or_else(|| option_context(node))
}

/// Language and kind of the script in `s`, if it is one.
pub fn string_context(s: &ast::Str, text: &str, depth: usize) -> Option<(Lang, Kind)> {
    if let Some(context) = usage_context(s.syntax(), text) {
        return Some(context);
    }
    if depth >= MAX_DEPTH {
        return None;
    }

    // let snippet = ''...''; used as a script, or interpolated into one
    let binding = AttrpathValue::cast(s.syntax().parent()?)?;
    let let_in = ast::LetIn::cast(binding.syntax().parent()?)?;
    let keys: Vec<_> = binding.attrpath()?.attrs().collect();
    let [key] = keys.as_slice() else { return None };
    let name = utils::attr_name(key)?;

    let references = let_in
        .syntax()
        .descendants()
        .filter_map(ast::Ident::cast)
        .filter(|i| i.syntax().text() == name.as_str())
        .filter(|i| i.syntax().parent().map(|p| p.kind()) != Some(SyntaxKind::NODE_ATTRPATH));
    for reference in references {
        let node = reference.syntax();
        if let Some(context) = usage_context(node, text) {
            return Some(context);
        }
        let outer = node
            .ancestors()
            .find(|a| a.kind() == SyntaxKind::NODE_INTERPOL)
            .and_then(|interp| interp.parent())
            .and_then(ast::Str::cast);
        if let Some(outer) = outer
            && let Some((lang, _)) = string_context(&outer, &Script::new(&outer).text, depth + 1)
        {
            return Some((lang, Kind::Fragment));
        }
    }
    None
}

/// A script file referenced from Nix code.
#[derive(Debug, Clone)]
pub struct Reference {
    /// Where the reference is in the Nix file.
    pub at: TextRange,
    /// The reference as written.
    pub written: String,
    pub path: PathBuf,
    pub lang: Lang,
    pub kind: Kind,
}

fn resolve(written: &str) -> Option<PathBuf> {
    let dir = current_dir()?;
    let relative = written
        .trim_start_matches("./")
        .trim_start_matches("$DEVENV_ROOT/")
        .trim_start_matches("${DEVENV_ROOT}/");
    let path = dir.join(relative);
    path.is_file().then_some(path)
}

fn kind_of(path: &Path) -> Kind {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default();
    if name.contains("hook") {
        Kind::Hook
    } else {
        Kind::Script
    }
}

/// `./script.sh`: a script file if it has a script extension or shebang, or
/// is used as a script (`exec = ./x;`, `builtins.readFile ./x`).
fn path_reference(node: &SyntaxNode) -> Option<Reference> {
    let written = node.text().to_string();
    if written.contains("${") {
        return None;
    }
    let path = resolve(&written)?;
    let lang = file_lang(&path).or_else(|| {
        // `readFile ./x` or the path itself as the value
        let parent = node.parent()?;
        let used = match ast::Apply::cast(parent) {
            Some(apply) if fn_name(&apply.lambda()?)? == "readFile" => apply.syntax().clone(),
            _ => node.clone(),
        };
        let text = std::fs::read_to_string(&path).ok()?;
        usage_context(&used, &text).map(|(lang, _)| lang)
    })?;
    let setup_hook = node
        .ancestors()
        .filter_map(ast::Apply::cast)
        .take(3)
        .any(|a| {
            let mut f = a.lambda();
            while let Some(Expr::Apply(inner)) = f {
                f = inner.lambda();
            }
            f.and_then(|f| fn_name(&f)).as_deref() == Some("makeSetupHook")
        });
    Some(Reference {
        at: node.text_range(),
        written,
        kind: if setup_hook {
            Kind::Hook
        } else {
            kind_of(&path)
        },
        path,
        lang,
    })
}

/// Script files run by a devenv script: `bun ./x.ts`, `python scripts/x.py`,
/// `./deploy.sh`. devenv runs scripts from the project root.
fn runtime_references(s: &ast::Str) -> Vec<Reference> {
    let is_devenv = s
        .syntax()
        .ancestors()
        .find_map(AttrpathValue::cast)
        .and_then(|apv| {
            let path = utils::enclosing_attrpath(apv.syntax())?;
            let path: Vec<&str> = path.iter().map(String::as_str).collect();
            let path = path.strip_prefix(&["config"]).unwrap_or(&path).to_vec();
            devenv_context(&apv, &path)
        })
        .is_some_and(|(lang, _)| matches!(lang, Lang::Shell(_)));
    if !is_devenv {
        return Vec::new();
    }
    let script = Script::new(s);
    let text = &script.text;
    let is_separator = |c: char| c.is_whitespace() || "\"'`;|&()<>=".contains(c);
    let mut out = Vec::new();
    let mut start = 0;
    for (i, c) in text.char_indices().chain([(text.len(), ' ')]) {
        if !is_separator(c) {
            continue;
        }
        let word = &text[start..i];
        let word_start = start;
        start = i + c.len_utf8();
        let is_script = Path::new(word)
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| SCRIPT_EXTENSIONS.contains(&e));
        if !is_script || word.contains(super::nixstr::PLACEHOLDER) {
            continue;
        }
        let Some(path) = resolve(word) else { continue };
        let Some(lang) = file_lang(&path) else {
            continue;
        };
        let Some(at) = script.source_range(word_start, i) else {
            continue;
        };
        out.push(Reference {
            at,
            written: word.to_string(),
            kind: kind_of(&path),
            path,
            lang,
        });
    }
    out
}

/// Script files `node` refers to.
pub fn references(node: &SyntaxNode) -> Vec<Reference> {
    match node.kind() {
        SyntaxKind::NODE_PATH_REL => path_reference(node).into_iter().collect(),
        SyntaxKind::NODE_STRING => ast::Str::cast(node.clone())
            .map(|s| runtime_references(&s))
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

/// Whether a node before `node` in the file already refers to the same
/// script, so each file is reported once.
pub fn referenced_earlier(node: &SyntaxNode, reference: &Reference) -> bool {
    let Some(root) = node.ancestors().last() else {
        return false;
    };
    root.descendants()
        .take_while(|n| n != node)
        .filter(|n| {
            matches!(
                n.kind(),
                SyntaxKind::NODE_PATH_REL | SyntaxKind::NODE_STRING
            )
        })
        .any(|n| references(&n).iter().any(|r| r.path == reference.path))
}
