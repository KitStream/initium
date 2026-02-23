pub mod exec;
pub mod fetch;
pub mod migrate;
pub mod render;
pub mod seed;
pub mod wait_for;
use crate::logging::Logger;
use std::io::{BufRead, BufReader, Read};
use std::process::Command;
pub fn run_command(log: &Logger, args: &[String]) -> Result<i32, String> {
    run_command_in_dir(log, args, None)
}
pub fn run_command_in_dir(log: &Logger, args: &[String], dir: Option<&str>) -> Result<i32, String> {
    let mut cmd = Command::new(&args[0]);
    cmd.args(&args[1..]);
    if let Some(d) = dir {
        cmd.current_dir(d);
    }
    cmd.stdin(std::process::Stdio::null());
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("starting command {:?}: {}", args[0], e))?;
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    std::thread::scope(|s| {
        let h1 = s.spawn(|| {
            if let Some(r) = stdout {
                stream_lines(log, r, "stdout");
            }
        });
        let h2 = s.spawn(|| {
            if let Some(r) = stderr {
                stream_lines(log, r, "stderr");
            }
        });
        h1.join().ok();
        h2.join().ok();
    });
    let status = child
        .wait()
        .map_err(|e| format!("waiting for command: {}", e))?;
    Ok(status.code().unwrap_or(-1))
}
fn stream_lines<R: Read>(log: &Logger, reader: R, stream: &str) {
    let buf = BufReader::new(reader);
    for l in buf.lines().map_while(Result::ok) {
        log.info(&l, &[("stream", stream)]);
    }
}
