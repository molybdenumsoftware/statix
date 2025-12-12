use crate::{config::WarningCode, err::ExplainErr, utils};

pub fn explain(code: WarningCode) -> Result<&'static str, ExplainErr> {
    let lints = utils::lint_map();
    match code.0 {
        0 => Ok("syntax error"),
        _ => lints
            .values()
            .flatten()
            .find(|l| l.code() == code.0)
            .map(|l| l.explanation())
            .ok_or(ExplainErr::LintNotFound(code.0)),
    }
}

pub mod main {

    use crate::{config::Explain as ExplainConfig, err::StatixErr};

    pub fn main(explain_config: &ExplainConfig) -> Result<(), StatixErr> {
        let explanation = super::explain(explain_config.target)?;
        println!("{explanation}");
        Ok(())
    }
}
