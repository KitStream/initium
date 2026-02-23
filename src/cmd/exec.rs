use crate::logging::Logger;
pub fn run(log: &Logger, args: &[String], workdir: &str) -> Result<(), String> {
    if args.is_empty() {
        return Err("command is required after \"--\"".into());
    }
    log.info("executing command", &[("command", &args[0])]);
    let dir = if workdir.is_empty() {
        None
    } else {
        Some(workdir)
    };
    let exit_code = super::run_command_in_dir(log, args, dir)?;
    if exit_code != 0 {
        return Err(format!("command exited with code {}", exit_code));
    }
    log.info("command completed successfully", &[]);
    Ok(())
}
