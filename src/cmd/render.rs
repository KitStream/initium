use crate::logging::Logger;
use crate::safety;
use crate::render as render_lib;
use std::fs;

pub fn run(log: &Logger, template: &str, output: &str, workdir: &str, mode: &str) -> Result<(), String> {
    if template.is_empty() {
        return Err("--template is required".into());
    }
    if output.is_empty() {
        return Err("--output is required".into());
    }
    if mode != "envsubst" && mode != "gotemplate" {
        return Err(format!("--mode must be envsubst or gotemplate, got {:?}", mode));
    }

    let out_path = safety::validate_file_path(workdir, output)?;
    let data = fs::read_to_string(template).map_err(|e| format!("reading template {}: {}", template, e))?;

    log.info("rendering template", &[("template", template), ("output", out_path.to_str().unwrap_or("")), ("mode", mode)]);

    let result = match mode {
        "envsubst" => render_lib::envsubst(&data),
        "gotemplate" => render_lib::template_render(&data)?,
        _ => unreachable!(),
    };

    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("creating output directory: {}", e))?;
    }
    fs::write(&out_path, result).map_err(|e| format!("writing output {:?}: {}", out_path, e))?;
    log.info("render completed", &[("output", out_path.to_str().unwrap_or(""))]);
    Ok(())
}
