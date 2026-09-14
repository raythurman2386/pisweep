//! Launch-contract argument parsing.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliAction {
    Run,
    Version,
    Help,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliError {
    pub argument: String,
}

pub fn parse_cli(args: &[String]) -> Result<CliAction, CliError> {
    match args.get(1).map(String::as_str) {
        None => Ok(CliAction::Run),
        Some("--version") | Some("-V") => Ok(CliAction::Version),
        Some("--help") | Some("-h") => Ok(CliAction::Help),
        Some(other) => Err(CliError {
            argument: other.to_string(),
        }),
    }
}

pub fn usage() -> &'static str {
    "Usage: pisweep [--version]\n\n  --version   Print version and exit"
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(rest: &[&str]) -> Vec<String> {
        std::iter::once("pisweep".into())
            .chain(rest.iter().map(|s| (*s).to_string()))
            .collect()
    }

    #[test]
    fn default_run() {
        assert_eq!(parse_cli(&args(&[])), Ok(CliAction::Run));
        assert_eq!(parse_cli(&args(&["--version"])), Ok(CliAction::Version));
        assert_eq!(parse_cli(&args(&["nope"])).unwrap_err().argument, "nope");
    }
}
