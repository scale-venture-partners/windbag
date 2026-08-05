use crate::rules::{Severity, Violation};

pub fn print_human(violations: &[Violation]) {
    for v in violations {
        let sev = match v.severity {
            Severity::Error => "error",
            Severity::Warn => "warn ",
        };
        println!("{}:{}  {} {}  {}", v.file, v.line, sev, v.rule, v.message);
    }
    let errors = violations
        .iter()
        .filter(|v| v.severity == Severity::Error)
        .count();
    let warnings = violations.len() - errors;
    println!();
    println!(
        "{} error(s), {} warning(s) across {} violation(s)",
        errors,
        warnings,
        violations.len()
    );
}

pub fn print_json(violations: &[Violation]) -> anyhow::Result<()> {
    println!("{}", serde_json::to_string_pretty(violations)?);
    Ok(())
}
