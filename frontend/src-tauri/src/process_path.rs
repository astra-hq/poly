use std::process::Command;
use std::sync::OnceLock;

static FIX_PATH_ENV_RESULT: OnceLock<Result<(), String>> = OnceLock::new();

pub fn fix_path_env_once() -> Result<(), String> {
    FIX_PATH_ENV_RESULT
        .get_or_init(|| fix_path_env::fix().map_err(|error| error.to_string()))
        .clone()
}

pub fn command(program: &str) -> Command {
    if let Err(error) = fix_path_env_once() {
        log::warn!(
            "Failed to repair PATH before launching {}: {}",
            program,
            error
        );
    }

    Command::new(program)
}

#[cfg(test)]
mod tests {
    use super::command;

    #[test]
    fn command_preserves_requested_program() {
        let command = command("docker");

        assert_eq!(command.get_program().to_string_lossy(), "docker");
    }
}
