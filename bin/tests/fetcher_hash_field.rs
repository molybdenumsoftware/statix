use lib::{LINTS, Report};

const SRI: &str = "sha256-xF9OTFFe8godW4+z9MFaFEkjE9FB42bKWwdl9xRcmEo=";

fn reports(source: &str) -> Vec<Report> {
    let parsed = rnix::Root::parse(source);
    assert!(parsed.errors().is_empty(), "{source}");
    let rule = LINTS
        .iter()
        .find(|rule| rule.name() == "fetcher_hash_field")
        .unwrap();
    parsed
        .syntax()
        .descendants()
        .filter(|node| rule.match_with(&node.kind()))
        .filter_map(|node| rule.validate(&node.into()))
        .collect()
}

#[test]
fn renames_only_the_hash_key_without_changing_payload_or_comments() {
    let source = format!(
        "{{ fetchFromGitHub }}: fetchFromGitHub {{ owner = \"someone\"; sha256 /* retain */ = \"{SRI}\"; }}"
    );
    let expected = format!(
        "{{ fetchFromGitHub }}: fetchFromGitHub {{ owner = \"someone\"; hash /* retain */ = \"{SRI}\"; }}"
    );
    let found = reports(&source);
    assert_eq!(found.len(), 1);
    let mut fixed = source;
    found[0].apply(&mut fixed);
    assert_eq!(fixed, expected);
    assert!(rnix::Root::parse(&fixed).errors().is_empty());
    assert!(reports(&fixed).is_empty());
}

#[test]
fn excludes_other_apis_encodings_and_ambiguous_sets() {
    for source in [
        format!("{{ sha256 = \"{SRI}\"; }}"),
        format!("custom {{ sha256 = \"{SRI}\"; }}"),
        format!("let fetchFromGitHub = custom; in fetchFromGitHub {{ sha256 = \"{SRI}\"; }}"),
        format!("fetchFromGitHub rec {{ sha256 = \"{SRI}\"; other = sha256; }}"),
        format!("fetchFromGitHub {{ sha256 = \"{SRI}\"; hash = \"other\"; }}"),
        format!("fetchFromGitHub {{ sha256 = \"{SRI}\"; sha512 = \"other\"; }}"),
        format!("fetchFromGitHub {{ sha256 = \"{SRI}\"; inherit hash; }}"),
        format!("fetchFromGitHub {{ sha256 = \"{SRI}\"; \"hash\" = \"other\"; }}"),
        "fetchFromGitHub { sha256 = \"legacy-base32\"; }".to_owned(),
        "fetchFromGitHub { sha256 = \"sha256-${value}\"; }".to_owned(),
        "fetchFromGitHub { sha256 = \"sha256-short=\"; }".to_owned(),
    ] {
        assert!(reports(&source).is_empty(), "{source}");
    }
}

#[test]
fn defaulted_fetcher_implementations_are_not_treated_as_nixpkgs() {
    let source =
        format!("{{ fetchFromGitHub ? args: args }}: fetchFromGitHub {{ sha256 = \"{SRI}\"; }}");
    assert!(reports(&source).is_empty(), "{source}");
}
