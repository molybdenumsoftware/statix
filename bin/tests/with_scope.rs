use lib::{LINTS, Report, Severity};

fn reports(source: &str) -> Vec<Report> {
    let parsed = rnix::Root::parse(source);
    assert!(
        parsed.errors().is_empty(),
        "invalid fixture: {:?}",
        parsed.errors()
    );
    parsed
        .syntax()
        .descendants()
        .flat_map(|node| {
            let kind = node.kind();
            LINTS
                .iter()
                .filter(move |lint| {
                    matches!(lint.name(), "broad_with_lib" | "with_lexical_collision")
                        && lint.match_with(&kind)
                })
                .filter_map(move |lint| lint.validate(&node.clone().into()))
        })
        .collect()
}

fn assert_hint(source: &str, name: &str, code: u32, highlighted: &str) {
    let reports = reports(source);
    assert_eq!(
        reports
            .iter()
            .map(|report| (report.name, report.code))
            .collect::<Vec<_>>(),
        vec![(name, code)],
        "{source}"
    );
    let report = &reports[0];
    assert!(matches!(report.severity, Severity::Hint));
    assert_eq!(report.diagnostics.len(), 1);
    let diagnostic = &report.diagnostics[0];
    assert!(diagnostic.suggestion.is_none());
    assert_eq!(
        &source[usize::from(diagnostic.at.start())..usize::from(diagnostic.at.end())],
        highlighted
    );
    let mut unchanged = source.to_owned();
    report.apply(&mut unchanged);
    assert_eq!(unchanged, source, "advice must not change legal resolution");
}

#[test]
fn broad_meta_scope_is_only_advice() {
    for (source, highlighted) in [
        (
            "{ lib }: { meta = with lib; { license = licenses.mit; }; }",
            "with lib; { license = licenses.mit; }",
        ),
        (
            "lib: { meta = ((with (lib); { license = licenses.mit; })); }",
            "with (lib); { license = licenses.mit; }",
        ),
        (
            "{ lib }: { meta = with lib; { changelog = version; }; }",
            "with lib; { changelog = version; }",
        ),
        (
            "let lib = {}; in lib: { meta = with lib; {}; }",
            "with lib; {}",
        ),
    ] {
        assert_hint(source, "broad_with_lib", 30, highlighted);
    }
}

#[test]
fn broad_rule_preserves_narrow_and_unproven_namespaces() {
    for source in [
        "{ lib }: { meta = { maintainers = with lib.maintainers; [ alice bob ]; }; }",
        "{ lib }: { meta = with lib.maintainers; [ alice bob ]; }",
        "{ lib }: with lib; { license = licenses.mit; }",
        "{ lib }: { other = with lib; {}; }",
        "{ lib }: { meta.license = with lib; licenses.mit; }",
        "{ lib }: { meta = with lib; licenses.mit; }",
        "{ lib }: { meta = with other; {}; }",
        "{ meta = with lib; {}; }",
        "let lib = {}; in { meta = with lib; {}; }",
        "{ lib }: let lib = {}; in { meta = with lib; {}; }",
        "{ lib }: rec { lib = {}; meta = with lib; {}; }",
        "with environment; { meta = with lib; {}; }",
        "{ lib }: { meta = with lib.helpers; {}; }",
    ] {
        assert!(reports(source).is_empty(), "{source}");
    }
}

#[test]
fn literal_namespace_collision_identifies_the_existing_lexical_reference() {
    for source in [
        "let foo = \"local\"; in with { foo = \"package\"; bar = \"other\"; }; [ foo bar ]",
        "foo: with { foo = 2; }; foo",
        "{ foo }: with { foo = 2; }; foo",
        "let inherit (outer) foo; in with { foo = 2; }; foo",
        "rec { foo = 1; value = with { foo = 2; }; foo; }",
        "let foo = 1; in with { \"foo\" = 2; }; foo",
        "let foo = 1; in with { foo.member = 2; }; foo.member",
        "let foo = 1; in with { foo = 2; }; let bar = 3; in foo",
    ] {
        assert_hint(source, "with_lexical_collision", 31, "foo");
    }
    assert_hint(
        "args@{ foo }: with { args = {}; }; args",
        "with_lexical_collision",
        31,
        "args",
    );
}

#[test]
fn collision_requires_an_outer_binding_and_a_body_variable() {
    for source in [
        "let foo = 1; in with { inherit foo; }; foo",
        "with { foo = 2; }; foo",
        "let foo = 1; in with { bar = 2; }; foo",
        "let foo = 1; in with pkgs; foo",
        "let foo = 1; in with ({ foo = 2; } // other); foo",
        "let foo = 1; in with { foo = 2; }; { foo = 3; }",
        "let foo = 1; in with { foo = 2; }; other.foo",
        "let foo = 1; in with { foo = 2; }; other ? foo",
        "let foo = 1; in with { foo = 2; }; \"foo\"",
        "let foo = 1; in with { foo = 2; }; let foo = 3; in foo",
        "let foo = 1; in with { foo = 2; }; foo: foo",
        "let foo = 1; in with { foo = 2; }; { foo }: foo",
        "let foo = 1; in with { foo = 2; }; foo@{}: foo",
        "let foo = 1; in with { foo = 2; }; rec { foo = 3; value = foo; }",
        "let foo = 1; in with { foo = 2; }; let inherit (other) foo; in foo",
        "let foo = 1; in with { foo = 2; }; with pkgs; foo",
        "let foo = 1; in with { foo = 2; }; [ (with pkgs; foo) bar ]",
        "let foo = 1; in with { ${name} = 2; }; foo",
    ] {
        assert!(reports(source).is_empty(), "{source}");
    }
}
