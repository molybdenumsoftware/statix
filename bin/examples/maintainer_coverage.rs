use std::{borrow::Cow, collections::BTreeSet};

use anyhow::{Context, Result, ensure};
use rnix::{Root, SyntaxKind};
use serde::Deserialize;
use statix::{LintMap, config::ConfFile, fix::FixResult, lint::lint_with};
use vfs::ReadOnlyVfs;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    id: String,
    source: String,
    rationale: String,
    rules: Vec<String>,
    before: String,
    after: String,
    autofix: bool,
    #[serde(default)]
    control: bool,
    #[serde(default)]
    negatives: Vec<String>,
}

fn tokens(source: &str) -> Result<Vec<(SyntaxKind, String)>> {
    let parsed = Root::parse(source);
    ensure!(
        parsed.errors().is_empty(),
        "invalid Nix: {:?}",
        parsed.errors()
    );
    Ok(parsed
        .syntax()
        .descendants_with_tokens()
        .filter_map(rnix::NodeOrToken::into_token)
        .filter(|token| token.kind() != SyntaxKind::TOKEN_WHITESPACE)
        .map(|token| (token.kind(), token.text().to_owned()))
        .collect())
}

fn reports(source: &str, lints: &LintMap) -> Vec<lib::Report> {
    let vfs = ReadOnlyVfs::singleton("case.nix", source.as_bytes());
    lint_with(&vfs.iter().next().unwrap(), lints).reports
}

fn fixed(source: &str, lints: &LintMap) -> Result<String> {
    let mut result = FixResult {
        src: Cow::Borrowed(source),
        fixed: Vec::new(),
        lints,
    };
    // Exercise the same fix iterator as `statix fix`, with a convergence guard.
    for _ in 0..64 {
        match result.next() {
            Some(next) => {
                tokens(&next.src).context("fix produced invalid Nix")?;
                result = next;
            }
            None => return Ok(result.src.into_owned()),
        }
    }
    anyhow::bail!("fix did not converge within 64 passes")
}

fn percent(numerator: u32, denominator: u32) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        100.0 * f64::from(numerator) / f64::from(denominator)
    }
}

fn main() -> Result<()> {
    let cases: Vec<Case> =
        serde_json::from_str(include_str!("../benchmarks/maintainer_cases.json"))?;
    ensure!(!cases.is_empty(), "empty benchmark corpus");
    let lints = ConfFile::default().lints();
    let mut ids = BTreeSet::new();
    let mut positives = 0;
    let mut detected = 0;
    let mut false_positives = 0;
    let mut negative_count = 0;
    let mut fixable = 0;
    let mut correct_fixes = 0;
    let mut controls = 0;
    let mut controls_detected = 0;
    let mut control_fixable = 0;
    let mut control_fixes = 0;
    let mut incorrect_fixes = 0;

    for case in cases {
        ensure!(ids.insert(case.id.clone()), "duplicate case: {}", case.id);
        ensure!(
            !case.source.is_empty() && !case.rationale.is_empty() && !case.rules.is_empty(),
            "incomplete evidence: {}",
            case.id
        );
        let before_tokens = tokens(&case.before).with_context(|| case.id.clone())?;
        let after_tokens = tokens(&case.after).with_context(|| case.id.clone())?;
        ensure!(before_tokens != after_tokens, "identical pair: {}", case.id);
        let before_reports = reports(&case.before, &lints);
        // An unrelated style warning does not count as detecting the issue.
        let hit = before_reports
            .iter()
            .any(|r| case.rules.iter().any(|n| n == r.name));
        let output = fixed(&case.before, &lints).with_context(|| case.id.clone())?;
        let output_tokens = tokens(&output)?;
        let correct_fix = output_tokens == after_tokens;
        let changed = output_tokens != before_tokens;
        if case.control {
            controls += 1;
            controls_detected += u32::from(hit);
            if case.autofix {
                control_fixable += 1;
                control_fixes += u32::from(correct_fix);
            }
        } else {
            positives += 1;
            detected += u32::from(hit);
            if case.autofix {
                fixable += 1;
                correct_fixes += u32::from(correct_fix);
            }
        }
        if changed && !correct_fix {
            incorrect_fixes += 1;
        }
        println!(
            "CASE {} detected={} fixed={} reports={:?} source={}",
            case.id,
            hit,
            correct_fix,
            before_reports.iter().map(|r| r.name).collect::<Vec<_>>(),
            case.source
        );
        for negative in std::iter::once(&case.after).chain(case.negatives.iter()) {
            let expected = tokens(negative).with_context(|| case.id.clone())?;
            let warnings = reports(negative, &lints);
            negative_count += 1;
            false_positives += u32::from(!warnings.is_empty());
            let output = fixed(negative, &lints).with_context(|| case.id.clone())?;
            if tokens(&output)? != expected {
                incorrect_fixes += 1;
            }
            if !warnings.is_empty() {
                println!(
                    "NEGATIVE {} reports={:?}",
                    case.id,
                    warnings.iter().map(|r| r.name).collect::<Vec<_>>()
                );
            }
        }
    }
    ensure!(positives > 0, "no researched cases");
    // F1 over researched positives and all accepted/near-miss negative examples.
    println!(
        "METRIC maintainer_f1={:.6}",
        percent(2 * detected, positives + detected + false_positives)
    );
    println!("METRIC detection_pct={:.6}", percent(detected, positives));
    println!("METRIC fix_pct={:.6}", percent(correct_fixes, fixable));
    println!("METRIC false_positives={false_positives}");
    println!("METRIC incorrect_fixes={incorrect_fixes}");
    println!(
        "METRIC control_detection_pct={:.6}",
        percent(controls_detected, controls)
    );
    println!(
        "METRIC control_fix_pct={:.6}",
        percent(control_fixes, control_fixable)
    );
    println!("METRIC researched_cases={positives}");
    println!("METRIC negative_cases={negative_count}");
    Ok(())
}
