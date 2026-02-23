use crate::logging::Logger;
pub fn run(log: &Logger, args: &[String]) -> Result<(), String> {
    if args.is_empty() {
        return Err("seed command is required after \"--\"".into());
    }
    log.info("starting seed", &[("command", &args[0])]);
    let exit_code = super::run_command(log, args)?;
    if exit_code != 0 {
        return Err(format!("seed exited with code {}", exit_code));
    }
    log.info("seed completed successfully", &[]);
    Ok(())
}
