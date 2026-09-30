use lib::{LINTS, Report, Severity};

fn reports(rule_name: &str, source: &str) -> Vec<Report> {
    let parsed = rnix::Root::parse(source);
    assert!(parsed.errors().is_empty(), "invalid fixture: {source}");
    let rule = LINTS
        .iter()
        .find(|rule| rule.name() == rule_name)
        .expect("requested lint is registered");
    parsed
        .syntax()
        .descendants()
        .filter(|node| rule.match_with(&node.kind()))
        .filter_map(|node| rule.validate(&node.into()))
        .collect()
}

#[test]
fn deprecated_to_path_respects_the_closest_lexical_scope() {
    for (source, detected) in [
        ("toPath x", true),
        ("builtins.toPath x", true),
        ("((toPath)) x", true),
        ("(builtins).toPath x", true),
        ("toPath: toPath x", false),
        ("builtins: builtins.toPath x", false),
        ("{ toPath, ... }: toPath x", false),
        ("{ builtins ? {} }: builtins.toPath x", false),
        ("builtins@{ ... }: builtins.toPath x", false),
        ("{ ... }@toPath: toPath x", false),
        ("{ toPath, x ? toPath y }: x", false),
        ("let toPath = f; in toPath x", false),
        ("let result = toPath x; toPath = f; in result", false),
        ("let builtins.toPath = f; in builtins.toPath x", false),
        ("let inherit toPath; in toPath x", false),
        ("let inherit (custom) builtins; in builtins.toPath x", false),
        ("rec { toPath = f; result = toPath x; }", false),
        (
            "rec { inherit (custom) builtins; result = builtins.toPath x; }",
            false,
        ),
        ("rec { \"toPath\" = f; result = toPath x; }", false),
        ("let { toPath = f; body = toPath x; }", false),
        ("{ toPath = f; result = toPath x; }", true),
        (
            "{ inherit (custom) builtins; result = builtins.toPath x; }",
            true,
        ),
        (
            "{ nested = let toPath = f; in toPath; result = toPath x; }",
            true,
        ),
        ("{ nested = toPath: toPath; result = toPath x; }", true),
        ("rec { result = { toPath = f; nested = toPath x; }; }", true),
        (
            "let builtins = custom; in { result = builtins.toPath x; }",
            false,
        ),
        (
            "let inherited = rec { inherit (toPath x) value; }; in inherited",
            true,
        ),
        (
            "let inherited = rec { toPath = f; inherit (toPath x) value; }; in inherited",
            false,
        ),
        ("builtins.toPath", false),
        ("map builtins.toPath xs", false),
        ("map toPath xs", false),
        ("(builtins.toPath or f) x", false),
        ("custom.toPath x", false),
    ] {
        assert_eq!(
            !reports("deprecated_to_path", source).is_empty(),
            detected,
            "{source}"
        );
    }
}

#[test]
fn negated_is_null_rejects_shadowed_values_and_higher_order_uses() {
    for (source, detected) in [
        ("!(builtins.isNull x)", true),
        ("!(isNull x)", true),
        ("!(((builtins.isNull) x))", true),
        ("builtins: !(builtins.isNull x)", false),
        ("isNull: !(isNull x)", false),
        ("{ builtins, ... }: !(builtins.isNull x)", false),
        ("{ isNull ? f }: !(isNull x)", false),
        ("{ null ? 0 }: !(builtins.isNull x)", false),
        ("let builtins = custom; in !(builtins.isNull x)", false),
        ("let isNull = f; in !(isNull x)", false),
        ("let null = 0; in !(builtins.isNull x)", false),
        ("let inherit (custom) isNull; in !(isNull x)", false),
        ("rec { isNull = f; result = !(isNull x); }", false),
        ("rec { null = 0; result = !(builtins.isNull x); }", false),
        ("{ isNull = f; result = !(isNull x); }", true),
        (
            "{ builtins = custom; result = !(builtins.isNull x); }",
            true,
        ),
        ("{ null = 0; result = !(builtins.isNull x); }", true),
        ("let isNull = f; in !(builtins.isNull x)", true),
        ("let builtins = custom; in !(isNull x)", true),
        ("builtins.isNull x", false),
        ("isNull x", false),
        ("map builtins.isNull xs", false),
        ("builtins.filter isNull xs", false),
        ("!(f builtins.isNull)", false),
        ("!(builtins.isNull x y)", false),
        ("!builtins.isNull", false),
        ("!(custom.isNull x)", false),
        ("!((builtins.isNull or f) x)", false),
    ] {
        assert_eq!(
            !reports("negated_is_null", source).is_empty(),
            detected,
            "{source}"
        );
    }
}

#[test]
fn negated_is_null_fixes_preserve_expression_grouping() {
    for (source, expected) in [
        ("!(builtins.isNull x)", "x != null"),
        ("!(isNull (a + b))", "(a + b) != null"),
        ("!(builtins.isNull (f x))", "(f x) != null"),
        (
            "!(isNull (if c then a else b))",
            "(if c then a else b) != null",
        ),
        (
            "!(builtins.isNull (x.value or fallback))",
            "(x.value or fallback) != null",
        ),
        (
            "!(isNull x.value or fallback)",
            "(x.value or fallback) != null",
        ),
        ("!!(builtins.isNull x)", "!(x != null)"),
        ("!(isNull x) == true", "(x != null) == true"),
        ("f (!(isNull x))", "f (x != null)"),
        ("[ (!(isNull x)) ]", "[ (x != null) ]"),
        ("!(isNull x) && ready", "x != null && ready"),
    ] {
        let mut found = reports("negated_is_null", source);
        assert_eq!(found.len(), 1, "{source}");
        let report = found.pop().unwrap();
        assert!(matches!(report.severity, Severity::Hint), "{source}");
        let mut fixed = source.to_owned();
        report.apply(&mut fixed);
        assert_eq!(fixed, expected, "{source}");
        assert!(rnix::Root::parse(&fixed).errors().is_empty(), "{fixed}");
    }
}

#[test]
fn negated_is_null_keeps_comments_outside_the_argument() {
    let source = "!(builtins.isNull /* intent */ x)";
    let found = reports("negated_is_null", source);
    assert_eq!(found.len(), 1);
    assert!(
        found[0]
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.suggestion.is_none())
    );
    let mut fixed = source.to_owned();
    found[0].apply(&mut fixed);
    assert_eq!(fixed, source);
}
