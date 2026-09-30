use lib::{LINTS, Report};

const HELPERS: &[&str] = &["optionals_string", "optional_list_in_flat_context"];

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
                .filter(move |lint| HELPERS.contains(&lint.name()) && lint.match_with(&kind))
                .filter_map(move |lint| lint.validate(&node.clone().into()))
        })
        .collect()
}

fn assert_fix(source: &str, expected: &str, name: &str, code: u32) {
    let reports = reports(source);
    assert_eq!(
        reports
            .iter()
            .map(|report| (report.name, report.code))
            .collect::<Vec<_>>(),
        vec![(name, code)],
        "{source}",
    );
    let mut fixed = source.to_owned();
    reports[0].apply(&mut fixed);
    assert_eq!(fixed, expected);
    assert!(rnix::Root::parse(&fixed).errors().is_empty(), "{fixed}");
    assert!(
        self::reports(&fixed).is_empty(),
        "fix must remove the helper mismatch"
    );
}

#[test]
fn optionals_string_fixes_only_the_callee() {
    for (source, expected) in [
        (
            r#"{ lib, needsPatch }: "echo start\n" + lib.optionals needsPatch ''echo patch''"#,
            r#"{ lib, needsPatch }: "echo start\n" + lib.optionalString needsPatch ''echo patch''"#,
        ),
        (
            r#"lib.optionals true "left" + "right""#,
            r#"lib.optionalString true "left" + "right""#,
        ),
        (
            r#"{ lib }: ("start") + (((lib /* select */ . optionals) /* condition */ true) /* payload */ (''end''))"#,
            r#"{ lib }: ("start") + (((lib /* select */ . optionalString) /* condition */ true) /* payload */ (''end''))"#,
        ),
        (
            r#"let lib = {}; in lib: "start" + lib.optionals true "end""#,
            r#"let lib = {}; in lib: "start" + lib.optionalString true "end""#,
        ),
        (
            r#"{ lib }: { lib = {}; value = "start" + lib.optionals true "end"; }"#,
            r#"{ lib }: { lib = {}; value = "start" + lib.optionalString true "end"; }"#,
        ),
    ] {
        assert_fix(source, expected, "optionals_string", 24);
    }
}

#[test]
fn optionals_string_leaves_ambiguous_or_list_contexts_alone() {
    for source in [
        r#"lib.optionals true "end""#,
        r#""start" + lib.optionals true"#,
        r#""start" + lib.optionals true "end" extra"#,
        r#""start" + lib.optionals true payload"#,
        r#"prefix + lib.optionals true "end""#,
        r#"[] ++ lib.optionals true "end""#,
        r#""start" + optionals true "end""#,
        r#""start" + other.optionals true "end""#,
        r#""start" + lib.strings.optionals true "end""#,
        r#""start" + (lib.optionals or fallback) true "end""#,
        r#""start" + lib.${"optionals"} true "end""#,
        r#"let lib = custom; in "start" + lib.optionals true "end""#,
        r#"{ lib }: let lib = custom; in "start" + lib.optionals true "end""#,
        r#"rec { lib = custom; value = "start" + lib.optionals true "end"; }"#,
        r#"let inherit (custom) lib; in "start" + lib.optionals true "end""#,
        r#"with custom; "start" + lib.optionals true "end""#,
        r#"{ lib }: [ (lib.optionals true "end") ]"#,
    ] {
        assert!(reports(source).is_empty(), "{source}");
    }
}

#[test]
fn optional_list_fixes_flat_make_bin_path_inputs() {
    for (source, expected) in [
        (
            "{ lib, enabled, package }: lib.makeBinPath (lib.optional enabled [ package ])",
            "{ lib, enabled, package }: lib.makeBinPath (lib.optionals enabled [ package ])",
        ),
        (
            "lib.makeBinPath ([ first ] ++ lib.optional true [ second ] ++ [ third ])",
            "lib.makeBinPath ([ first ] ++ lib.optionals true [ second ] ++ [ third ])",
        ),
        (
            "{ lib }: (lib /* consumer */ . makeBinPath) (((lib /* helper */ . optional) /* condition */ true) /* payload */ ([ package ]))",
            "{ lib }: (lib /* consumer */ . makeBinPath) (((lib /* helper */ . optionals) /* condition */ true) /* payload */ ([ package ]))",
        ),
        (
            "lib.makeBinPath (lib.optional false [])",
            "lib.makeBinPath (lib.optionals false [])",
        ),
    ] {
        assert_fix(source, expected, "optional_list_in_flat_context", 25);
    }
}

#[test]
fn optional_list_keeps_intentional_nesting_and_unknown_consumers() {
    for source in [
        "lib.optional true [ package ]",
        "[ (lib.optional true [ package ]) ]",
        "lib.makeBinPath [ (lib.optional true [ package ]) ]",
        "consume (lib.optional true [ package ])",
        "other.makeBinPath (lib.optional true [ package ])",
        "makeBinPath (lib.optional true [ package ])",
        "lib.makeBinPath (optional true [ package ])",
        "lib.makeBinPath (other.optional true [ package ])",
        "lib.makeBinPath (lib.optional true packageList)",
        "lib.makeBinPath (lib.optional true)",
        "lib.makeBinPath (lib.optional true [ package ] extra)",
        "lib.makeBinPath (map f (lib.optional true [ package ]))",
        "lib.makeBinPath (if enabled then lib.optional true [ package ] else [])",
        "lib.makeBinPath ((lib.optional or fallback) true [ package ])",
        "(lib.makeBinPath or fallback) (lib.optional true [ package ])",
        "let lib = custom; in lib.makeBinPath (lib.optional true [ package ])",
        "{ lib }: let lib = custom; in lib.makeBinPath (lib.optional true [ package ])",
        "rec { lib = custom; value = lib.makeBinPath (lib.optional true [ package ]); }",
        "let inherit lib; in lib.makeBinPath (lib.optional true [ package ])",
        "with custom; lib.makeBinPath (lib.optional true [ package ])",
        "{ buildInputs = lib.optional true [ package ]; }",
    ] {
        assert!(reports(source).is_empty(), "{source}");
    }
}

#[test]
fn custom_default_library_implementations_are_not_rewritten() {
    for source in [
        r#"({ lib ? { optionals = c: s: s; optionalString = c: s: "changed"; } }: "prefix" + lib.optionals true "value") {}"#,
        r"{ lib ? custom }: lib.makeBinPath (lib.optional true [ package ])",
        r#"let lib = canonical; in ({ lib ? custom }: "prefix" + lib.optionals true "value") {}"#,
    ] {
        assert!(reports(source).is_empty(), "{source}");
    }
}

#[test]
fn local_helper_library_parameters_are_not_conventional_inputs() {
    for source in [
        r#"{ lib }: let helper = lib: "prefix" + lib.optionals true "value"; in helper { optionals = c: s: s; optionalString = c: s: "changed"; }"#,
        r#"{ lib }: let helper = { lib }: "prefix" + lib.optionals true "value"; in helper { lib = custom; }"#,
        r#"{ lib }: { helper = lib: "prefix" + lib.optionals true "value"; }"#,
        r#"{ optionals, optionalString }@lib: "prefix" + lib.optionals true "value""#,
        r"{ lib }: let helper = lib: lib.makeBinPath (lib.optional true [ package ]); in helper custom",
    ] {
        assert!(reports(source).is_empty(), "{source}");
    }
}

#[test]
fn outer_library_inputs_remain_visible_inside_helper_closures() {
    assert_fix(
        r#"{ lib }: let helper = enabled: "prefix" + lib.optionals enabled "value"; in helper true"#,
        r#"{ lib }: let helper = enabled: "prefix" + lib.optionalString enabled "value"; in helper true"#,
        "optionals_string",
        24,
    );
    assert_fix(
        r#"inputs: ({ lib }: "prefix" + lib.optionals true "value")"#,
        r#"inputs: ({ lib }: "prefix" + lib.optionalString true "value")"#,
        "optionals_string",
        24,
    );
}
