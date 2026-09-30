use lib::{LINTS, Report, Severity};

const ADVISORIES: &[&str] = &["module_optional_attrs", "module_mkif_update"];

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
                .filter(move |lint| ADVISORIES.contains(&lint.name()) && lint.match_with(&kind))
                .filter_map(move |lint| lint.validate(&node.clone().into()))
        })
        .collect()
}

fn assert_advisory(source: &str, name: &str, code: u32) {
    let reports = reports(source);
    assert_eq!(
        reports
            .iter()
            .map(|report| (report.name, report.code))
            .collect::<Vec<_>>(),
        vec![(name, code)],
        "{source}",
    );
    assert!(matches!(reports[0].severity, Severity::Hint), "{source}");
    assert_eq!(reports[0].diagnostics.len(), 1, "{source}");
    assert!(reports[0].diagnostics[0].suggestion.is_none(), "{source}");
    let mut unchanged = source.to_owned();
    reports[0].apply(&mut unchanged);
    assert_eq!(
        unchanged, source,
        "advisories must not rewrite configuration"
    );
}

fn assert_clean(source: &str) {
    assert!(reports(source).is_empty(), "{source}");
}

#[test]
fn optional_attrs_advises_on_direct_config_dependent_module_roots() {
    for source in [
        r#"{ config, lib, ... }: {
            options.demo.enable = lib.mkEnableOption "demo";
            options.demo.message = lib.mkOption { type = lib.types.str; default = "off"; };
            config = lib.optionalAttrs config.demo.enable { demo.message = "on"; };
        }"#,
        r#"{ config, lib, ... }: {
            imports = [ ./options.nix ];
            config = { demo.message = "off"; } // lib.optionalAttrs config.demo.enable { demo.message = "on"; };
        }"#,
        r"{ config, lib }: ({ options.demo = {}; config = (((((lib).optionalAttrs) (config.demo.enable)) ({ demo = true; }))); })",
        r"{ config, lib }: let unrelated = true; in ({ options.demo = {}; config = let other = {}; in lib.optionalAttrs (!config.demo.enable) { demo = true; }; })",
        r"let lib = custom; in { config, lib }: { options.demo = {}; config = lib.optionalAttrs config.demo.enable {}; }",
        r"{ config, lib }: { options.demo = {}; config = lib.optionalAttrs (let unrelated = true; in config.demo.enable && unrelated) {}; }",
        r"{ config, lib }: { options.demo = {}; config = lib.optionalAttrs ((other: config.demo.enable) {}) {}; }",
        r"{ config, lib }: { options.demo = {}; config = lib.optionalAttrs (config ? demo.enable) {}; }",
    ] {
        assert_advisory(source, "module_optional_attrs", 29);
    }
}

#[test]
fn optional_attrs_preserves_unrelated_conditions_and_ordinary_data() {
    for source in [
        r"{ config, lib }: { options.demo = {}; config = lib.optionalAttrs true { demo = true; }; }",
        r"{ config, lib, enabled }: { options.demo = {}; config = lib.optionalAttrs enabled { demo = true; }; }",
        r"{ config, lib, options }: { options.demo = {}; config = lib.optionalAttrs (options ? demo) { demo = true; }; }",
        r"{ config, lib }: { options.demo = {}; config = lib.optionalAttrs other.config.enable { demo = true; }; }",
        r"{ config, lib }: { options.demo = {}; config = lib.optionalAttrs (builtins.isAttrs { config = true; }) {}; }",
        r#"{ config, lib }: { options.demo = {}; config = lib.optionalAttrs (builtins.hasAttr "config" other) {}; }"#,
        r"lib.optionalAttrs config.demo.enable { demo = true; }",
        r"{ options.demo = {}; config = lib.optionalAttrs config.demo.enable {}; }",
        r"{ config, lib }: { value = lib.optionalAttrs config.demo.enable {}; }",
        r"{ config, lib }: { config = lib.optionalAttrs config.demo.enable {}; }",
        r"{ config, lib }: { options.demo = {}; config = { data = lib.optionalAttrs config.demo.enable {}; }; }",
        r"{ config, lib }: { options.demo = {}; config = {}; data = lib.optionalAttrs config.demo.enable {}; }",
        r"{ config, lib }: { options.demo.default = lib.optionalAttrs config.demo.enable {}; config = {}; }",
        r"{ config, lib }: { options.demo = {}; config.data = lib.optionalAttrs config.demo.enable {}; }",
        r"{ config, lib }: { options.demo = {}; nested = { config = lib.optionalAttrs config.demo.enable {}; options.demo = {}; }; config = {}; }",
        r"{ config, lib }: consume { options.demo = {}; config = lib.optionalAttrs config.demo.enable {}; }",
        r"{ config, lib }: { options.demo = {}; config = consume (lib.optionalAttrs config.demo.enable {}); }",
        r"{ config, lib }: { options.demo = {}; config = lib.mkMerge [ (lib.optionalAttrs config.demo.enable {}) ]; }",
        r"{ config, lib }: let cfg = config.demo; in { options.demo = {}; config = lib.optionalAttrs cfg.enable {}; }",
    ] {
        assert_clean(source);
    }
}

#[test]
fn optional_attrs_respects_the_exact_supplied_config_binding() {
    for source in [
        r"{ config, lib }: { options.demo = {}; config = let config = { demo.enable = true; }; in lib.optionalAttrs config.demo.enable {}; }",
        r"{ config, lib }: let config = { demo.enable = true; }; in { options.demo = {}; config = lib.optionalAttrs config.demo.enable {}; }",
        r"{ config, lib }: { options.demo = {}; config = lib.optionalAttrs (let config = { demo.enable = true; }; in config.demo.enable) {}; }",
        r"{ config, lib }: rec { options.demo = {}; config = lib.optionalAttrs config.demo.enable {}; }",
        r"{ config, lib }: { options.demo = {}; config = lib.optionalAttrs ((config: config.enable) { enable = true; }) {}; }",
        r"{ config, lib }: { options.demo = {}; config = lib.optionalAttrs (({ config }: config.enable) { config.enable = true; }) {}; }",
        r"{ config, lib }: { options.demo = {}; config = lib.optionalAttrs ((args@{ config }: config.enable) { config.enable = true; }) {}; }",
        r"{ config, lib }: { options.demo = {}; config = lib.optionalAttrs ((config@{ enable }: config.enable) { enable = true; }) {}; }",
        r"{ config, lib }: { options.demo = {}; config = lib.optionalAttrs (let inherit (other) config; in config.enable) {}; }",
        r"{ lib }: { options.demo = {}; config = lib.optionalAttrs config.demo.enable {}; }",
        r"{ config ? custom, lib }: { options.demo = {}; config = lib.optionalAttrs config.demo.enable {}; }",
        r"config@{ lib }: { options.demo = {}; config = lib.optionalAttrs config.demo.enable {}; }",
        r"{ config, lib }: { options.demo = {}; config = config: lib.optionalAttrs config.demo.enable {}; }",
    ] {
        assert_clean(source);
    }
}

#[test]
fn optional_attrs_requires_an_exact_qualified_two_argument_call() {
    for source in [
        r"{ config, lib }: { options.demo = {}; config = lib.optionalAttrs config.demo.enable; }",
        r"{ config, lib }: { options.demo = {}; config = lib.optionalAttrs config.demo.enable {} extra; }",
        r"{ config, lib }: { options.demo = {}; config = (lib.optionalAttrs config.demo.enable {}) extra; }",
        r"{ config, lib }: { options.demo = {}; config = lib.optionalAttrs config.demo.enable payload; }",
        r"{ config, lib }: { options.demo = {}; config = optionalAttrs config.demo.enable {}; }",
        r"{ config, lib }: { options.demo = {}; config = other.optionalAttrs config.demo.enable {}; }",
        r"{ config, lib }: { options.demo = {}; config = lib.attrsets.optionalAttrs config.demo.enable {}; }",
        r"{ config, lib }: { options.demo = {}; config = (lib.optionalAttrs or fallback) config.demo.enable {}; }",
        r#"{ config, lib }: { options.demo = {}; config = lib.${"optionalAttrs"} config.demo.enable {}; }"#,
    ] {
        assert_clean(source);
    }
}

#[test]
fn mkif_update_advises_on_either_direct_root_operand() {
    for source in [
        r#"{ lib, ... }: {
            options.a = lib.mkEnableOption "a";
            options.b = lib.mkEnableOption "b";
            config = (lib.mkIf false { a = true; }) // { b = true; };
        }"#,
        r"{ lib }: { options.demo = {}; config = { demo = false; } // lib.mkIf true { demo = true; }; }",
        r"{ lib }: { imports = [ ./options.nix ]; config = lib.mkIf false {} // {}; }",
        r"{ lib }: ({ options.demo = {}; config = (((((lib).mkIf) (false)) ({}))) // ({}); })",
        r"{ lib }: let unrelated = true; in { options.demo = {}; config = let other = {}; in (lib.mkIf unrelated {}) // other; }",
        r"let lib = custom; in { lib }: { options.demo = {}; config = lib.mkIf false {} // {}; }",
        r"{ lib }: { options.demo = {}; config = ({} // lib.mkIf false {}) // {}; }",
        r"{ lib }: { options.demo = {}; config = lib.mkIf false {} // lib.mkIf true {}; }",
    ] {
        assert_advisory(source, "module_mkif_update", 28);
    }
}

#[test]
fn mkif_update_preserves_plain_updates_and_inner_values() {
    for source in [
        r"{ lib }: { options.demo = {}; config = { demo = false; } // { demo = true; }; }",
        r"{ lib }: { options.demo = {}; config = lib.optionalAttrs false {} // {}; }",
        r"{ lib }: { options.demo = {}; config = lib.mkIf false {}; }",
        r"{ lib }: { options.demo = {}; config = lib.mkMerge [ (lib.mkIf false {}) {} ]; }",
        r"lib.mkIf false {} // {}",
        r"{ options.demo = {}; config = lib.mkIf false {} // {}; }",
        r"{ lib }: { value = lib.mkIf false {} // {}; }",
        r"{ lib }: { config = lib.mkIf false {} // {}; }",
        r"{ lib }: { options.demo = {}; config = { data = lib.mkIf false {} // {}; }; }",
        r"{ lib }: { options.demo = {}; config = {}; data = lib.mkIf false {} // {}; }",
        r"{ lib }: { options.demo.default = lib.mkIf false {} // {}; config = {}; }",
        r"{ lib }: { options.demo = {}; config.data = lib.mkIf false {} // {}; }",
        r"{ lib }: { options.demo = {}; config = consume (lib.mkIf false {} // {}); }",
        r"{ lib }: consume { options.demo = {}; config = lib.mkIf false {} // {}; }",
        r"{ lib }: { options.demo = {}; config = (consume (lib.mkIf false {})) // {}; }",
        r"{ lib }: { options.demo = {}; config = lib.mkMerge [ (lib.mkIf false {} // {}) ]; }",
    ] {
        assert_clean(source);
    }
}

#[test]
fn mkif_update_requires_an_exact_qualified_two_argument_call() {
    for source in [
        r"{ lib }: { options.demo = {}; config = lib.mkIf false // {}; }",
        r"{ lib }: { options.demo = {}; config = lib.mkIf false {} extra // {}; }",
        r"{ lib }: { options.demo = {}; config = {} // (lib.mkIf false {}) extra; }",
        r"{ lib }: { options.demo = {}; config = lib.mkIf false payload // {}; }",
        r"{ lib }: { options.demo = {}; config = mkIf false {} // {}; }",
        r"{ lib }: { options.demo = {}; config = other.mkIf false {} // {}; }",
        r"{ lib }: { options.demo = {}; config = lib.modules.mkIf false {} // {}; }",
        r"{ lib }: { options.demo = {}; config = (lib.mkIf or fallback) false {} // {}; }",
        r#"{ lib }: { options.demo = {}; config = lib.${"mkIf"} false {} // {}; }"#,
    ] {
        assert_clean(source);
    }
}

#[test]
fn both_advisories_reject_locally_defined_or_differently_supplied_libraries() {
    for source in [
        r"{ config, lib }: let lib = custom; in { options.demo = {}; config = lib.optionalAttrs config.demo.enable {}; }",
        r"{ config, lib }: { options.demo = {}; config = let lib = custom; in lib.optionalAttrs config.demo.enable {}; }",
        r"{ config, lib }: let inherit (custom) lib; in { options.demo = {}; config = lib.optionalAttrs config.demo.enable {}; }",
        r"{ config, lib }: rec { lib = custom; options.demo = {}; config = lib.optionalAttrs config.demo.enable {}; }",
        r"{ config, lib }: { options.demo = {}; config = lib: lib.optionalAttrs config.demo.enable {}; }",
        r"{ config, lib ? custom }: { options.demo = {}; config = lib.optionalAttrs config.demo.enable {}; }",
        r"{ config }: with custom; { options.demo = {}; config = lib.optionalAttrs config.demo.enable {}; }",
        r"{ lib }: let lib = custom; in { options.demo = {}; config = lib.mkIf false {} // {}; }",
        r"{ lib }: { options.demo = {}; config = let lib = custom; in lib.mkIf false {} // {}; }",
        r"{ lib }: let inherit (custom) lib; in { options.demo = {}; config = lib.mkIf false {} // {}; }",
        r"{ lib }: rec { lib = custom; options.demo = {}; config = lib.mkIf false {} // {}; }",
        r"{ lib }: { options.demo = {}; config = lib: lib.mkIf false {} // {}; }",
        r"{ lib ? custom }: { options.demo = {}; config = lib.mkIf false {} // {}; }",
        r"args@{ ... }: { options.demo = {}; config = lib.mkIf false {} // {}; }",
        r"lib@{ ... }: { options.demo = {}; config = lib.mkIf false {} // {}; }",
    ] {
        assert_clean(source);
    }
}
