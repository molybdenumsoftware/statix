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
                    lint.name() == "suspect_native_dependency" && lint.match_with(&kind)
                })
                .filter_map(move |lint| lint.validate(&node.clone().into()))
        })
        .collect()
}

fn assert_advisory(source: &str) {
    let reports = reports(source);
    assert_eq!(reports.len(), 1, "{source}");
    let report = &reports[0];
    assert_eq!(report.name, "suspect_native_dependency", "{source}");
    assert_eq!(report.code, 37, "{source}");
    assert!(matches!(report.severity, Severity::Hint), "{source}");
    assert_eq!(report.diagnostics.len(), 1, "{source}");
    assert!(report.diagnostics[0].suggestion.is_none(), "{source}");
    let mut unchanged = source.to_owned();
    report.apply(&mut unchanged);
    assert_eq!(
        unchanged, source,
        "host inputs must never be moved automatically"
    );
}

fn assert_clean(source: &str) {
    assert!(reports(source).is_empty(), "{source}");
}

#[test]
fn advises_on_python_application_hook_and_host_introspection_context() {
    for source in [
        // Dependency shape from the hushboard change in nixpkgs PR #239191.
        r"{ buildPythonApplication, wrapGAppsHook, gobject-introspection, gtk3, libappindicator, libpulseaudio, ... }:
          buildPythonApplication {
            nativeBuildInputs = [ wrapGAppsHook ];
            buildInputs = [ gobject-introspection gtk3 libappindicator libpulseaudio ];
          }",
        r"buildPythonApplication { nativeBuildInputs = [ wrapGAppsHook ]; buildInputs = [ gobject-introspection ]; }",
        r"{ buildPythonApplication, wrapGAppsHook, gobject-introspection }: ((buildPythonApplication) ({ nativeBuildInputs = ([ (wrapGAppsHook) ]); buildInputs = ([ (gobject-introspection) ]); }))",
        r"{ buildPythonApplication, wrapGAppsHook, gobject-introspection, gtk3, pkg-config }: buildPythonApplication { nativeBuildInputs = [ pkg-config wrapGAppsHook ]; buildInputs = [ gtk3 gobject-introspection ]; }",
        r"let buildPythonApplication = custom; in { buildPythonApplication, wrapGAppsHook, gobject-introspection }: buildPythonApplication { nativeBuildInputs = [ wrapGAppsHook ]; buildInputs = [ gobject-introspection ]; }",
        r"{ buildPythonApplication, wrapGAppsHook, gobject-introspection, src }: buildPythonApplication { inherit src; nativeBuildInputs = [ wrapGAppsHook ]; buildInputs = [ gobject-introspection ]; }",
    ] {
        assert_advisory(source);
    }
}

#[test]
fn preserves_host_library_only_shapes_and_packages_without_the_hook() {
    for source in [
        r"buildPythonApplication { buildInputs = [ gobject-introspection ]; }",
        r"buildPythonApplication { nativeBuildInputs = []; buildInputs = [ gobject-introspection ]; }",
        r"buildPythonApplication { nativeBuildInputs = [ pkg-config ]; buildInputs = [ gobject-introspection ]; }",
        r"buildPythonApplication { nativeBuildInputs = []; buildInputs = [ wrapGAppsHook gobject-introspection ]; }",
        r"stdenv.mkDerivation { nativeBuildInputs = [ pkg-config ]; buildInputs = [ gobject-introspection ]; }",
        r"stdenv.mkDerivation { nativeBuildInputs = [ wrapGAppsHook ]; buildInputs = [ gobject-introspection ]; }",
    ] {
        assert_clean(source);
    }
}

#[test]
fn preserves_correct_native_placement_including_retained_host_libraries() {
    for source in [
        r"buildPythonApplication { nativeBuildInputs = [ wrapGAppsHook gobject-introspection ]; buildInputs = [ gtk3 ]; }",
        r"buildPythonApplication { nativeBuildInputs = [ gobject-introspection wrapGAppsHook ]; buildInputs = [ gobject-introspection gtk3 ]; }",
        r"buildPythonApplication { nativeBuildInputs = [ wrapGAppsHook ]; buildInputs = [ gtk3 ]; }",
        r"buildPythonApplication { nativeBuildInputs = [ wrapGAppsHook ]; buildInputs = []; }",
    ] {
        assert_clean(source);
    }
}

#[test]
fn excludes_data_custom_builders_and_indirect_or_recursive_arguments() {
    for source in [
        r"{ nativeBuildInputs = [ wrapGAppsHook ]; buildInputs = [ gobject-introspection ]; }",
        r"consume { nativeBuildInputs = [ wrapGAppsHook ]; buildInputs = [ gobject-introspection ]; }",
        r"custom.buildPythonApplication { nativeBuildInputs = [ wrapGAppsHook ]; buildInputs = [ gobject-introspection ]; }",
        r"buildPythonApplication rec { nativeBuildInputs = [ wrapGAppsHook ]; buildInputs = [ gobject-introspection ]; }",
        r"buildPythonApplication (args: { nativeBuildInputs = [ wrapGAppsHook ]; buildInputs = [ gobject-introspection ]; })",
        r"let args = { nativeBuildInputs = [ wrapGAppsHook ]; buildInputs = [ gobject-introspection ]; }; in buildPythonApplication args",
        r"buildPythonApplication ({ nativeBuildInputs = [ wrapGAppsHook ]; } // { buildInputs = [ gobject-introspection ]; })",
        r"buildPythonApplication (let unrelated = true; in { nativeBuildInputs = [ wrapGAppsHook ]; buildInputs = [ gobject-introspection ]; })",
        r"buildPythonApplication { nativeBuildInputs = [ wrapGAppsHook ]; buildInputs = [ gobject-introspection ]; } extra",
        r"(buildPythonApplication { nativeBuildInputs = [ wrapGAppsHook ]; buildInputs = [ gobject-introspection ]; }) extra",
    ] {
        assert_clean(source);
    }
}

#[test]
fn rejects_locally_defined_builders_and_package_values() {
    for source in [
        r"let buildPythonApplication = args: args; in buildPythonApplication { nativeBuildInputs = [ wrapGAppsHook ]; buildInputs = [ gobject-introspection ]; }",
        r"let inherit (custom) buildPythonApplication; in buildPythonApplication { nativeBuildInputs = [ wrapGAppsHook ]; buildInputs = [ gobject-introspection ]; }",
        r"rec { buildPythonApplication = custom; package = buildPythonApplication { nativeBuildInputs = [ wrapGAppsHook ]; buildInputs = [ gobject-introspection ]; }; }",
        r"let gobject-introspection = custom; in buildPythonApplication { nativeBuildInputs = [ wrapGAppsHook ]; buildInputs = [ gobject-introspection ]; }",
        r"let wrapGAppsHook = custom; in buildPythonApplication { nativeBuildInputs = [ wrapGAppsHook ]; buildInputs = [ gobject-introspection ]; }",
        r"let gtk3 = custom; in buildPythonApplication { nativeBuildInputs = [ wrapGAppsHook ]; buildInputs = [ gobject-introspection gtk3 ]; }",
        r"let inherit (custom) gobject-introspection; in buildPythonApplication { nativeBuildInputs = [ wrapGAppsHook ]; buildInputs = [ gobject-introspection ]; }",
        r"with custom; buildPythonApplication { nativeBuildInputs = [ wrapGAppsHook ]; buildInputs = [ gobject-introspection ]; }",
    ] {
        assert_clean(source);
    }
}

#[test]
fn excludes_dynamic_lists_and_unknown_dependency_expressions() {
    for source in [
        r"let deps = [ gobject-introspection ]; in buildPythonApplication { nativeBuildInputs = [ wrapGAppsHook ]; buildInputs = deps; }",
        r"let hooks = [ wrapGAppsHook ]; in buildPythonApplication { nativeBuildInputs = hooks; buildInputs = [ gobject-introspection ]; }",
        r"buildPythonApplication { nativeBuildInputs = [ wrapGAppsHook ] ++ extra; buildInputs = [ gobject-introspection ]; }",
        r"buildPythonApplication { nativeBuildInputs = [ wrapGAppsHook ]; buildInputs = [ gobject-introspection ] ++ extra; }",
        r"buildPythonApplication { nativeBuildInputs = if enabled then [ wrapGAppsHook ] else []; buildInputs = [ gobject-introspection ]; }",
        r"buildPythonApplication { nativeBuildInputs = [ wrapGAppsHook ]; buildInputs = if enabled then [ gobject-introspection ] else []; }",
        r"buildPythonApplication { nativeBuildInputs = [ wrapGAppsHook ]; buildInputs = [ gobject-introspection (if enabled then gtk3 else gtk4) ]; }",
        r"buildPythonApplication { nativeBuildInputs = [ wrapGAppsHook (chooseHook enabled) ]; buildInputs = [ gobject-introspection ]; }",
        r"buildPythonApplication { nativeBuildInputs = with pkgs; [ wrapGAppsHook ]; buildInputs = [ gobject-introspection ]; }",
        r"buildPythonApplication { nativeBuildInputs = [ wrapGAppsHook ]; buildInputs = with pkgs; [ gobject-introspection ]; }",
        r"buildPythonApplication { inherit nativeBuildInputs; buildInputs = [ gobject-introspection ]; }",
        r"buildPythonApplication { nativeBuildInputs = [ wrapGAppsHook ]; inherit buildInputs; }",
        r#"buildPythonApplication { nativeBuildInputs = [ wrapGAppsHook ]; buildInputs = [ gobject-introspection ]; ${"buildInputs"} = extra; }"#,
    ] {
        assert_clean(source);
    }
}

#[test]
fn excludes_selected_library_outputs_and_qualified_package_guesses() {
    for source in [
        r"buildPythonApplication { nativeBuildInputs = [ wrapGAppsHook ]; buildInputs = [ gobject-introspection.out ]; }",
        r"buildPythonApplication { nativeBuildInputs = [ wrapGAppsHook ]; buildInputs = [ gobject-introspection.dev ]; }",
        r"buildPythonApplication { nativeBuildInputs = [ wrapGAppsHook ]; buildInputs = [ pkgs.gobject-introspection ]; }",
        r"buildPythonApplication { nativeBuildInputs = [ pkgs.wrapGAppsHook ]; buildInputs = [ gobject-introspection ]; }",
        r"buildPythonApplication { nativeBuildInputs = [ wrapGAppsHook gobject-introspection.out ]; buildInputs = [ gobject-introspection ]; }",
        r"buildPythonApplication { nativeBuildInputs = [ wrapGAppsHook ]; buildInputs = [ gobject-introspection gtk3.dev ]; }",
    ] {
        assert_clean(source);
    }
}
