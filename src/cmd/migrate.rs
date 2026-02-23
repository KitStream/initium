use crate::logging::Logger;
use crate::safety;
use std::fs;
pub fn run(log: &Logger, args: &[String], workdir: &str, lock_file: &str) -> Result<(), String> {
    if args.is_empty() {
        return Err("migration command is required after \"--\"".into());
    }
    if !lock_file.is_empty() {
        let lock_path = safety::validate_file_path(workdir, lock_file)?;
        if lock_path.exists() {
            log.info("lock file exists, skipping migration", &[("lock-file", lock_path.to_str().unwrap_or(""))]);
            return Ok(());
        }
    }
    log.info("starting migration", &[("command", &args[0])]);
    let exit_code = super::run_command(log, args)?;
    if exit_code != 0 {
        return Err(format!("migration exited with code {}", exit_code));
    }
    if !lock_file.is_empty() {
        let lock_path = safety::validate_file_path(workdir, lock_file)?;
        fs::create_dir_all(workdir).map_err(|e| format!("creating workdir {}: {}", workdir, e))?;
        fs::write(&lock_path, "migrated\n").map_err(|e| format!("writing lock file {:?}: {}", lock_path, e))?;
        log.info("lock file created", &[("lock-file", lock_path.to_str().unwrap_or(""))]);
    }
    log.info("migration completed successfully", &[]);
    Ok(())
}
