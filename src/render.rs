use regex::Regex;
use std::env;
pub fn envsubst(input: &str) -> String {
    let re = Regex::new(r"\$\{([a-zA-Z_][a-zA-Z0-9_]*)\}|\$([a-zA-Z_][a-zA-Z0-9_]*)").unwrap();
    re.replace_all(input, |caps: &regex::Captures| {
        let name = caps.get(1).or_else(|| caps.get(2)).unwrap().as_str();
        match env::var(name) {
            Ok(val) => val,
            Err(_) => caps[0].to_string(),
        }
    }).into_owned()
}
pub fn template_render(input: &str) -> Result<String, String> {
    let env_map: std::collections::HashMap<String, String> = env::vars().collect();
    let mut jinja_env = minijinja::Environment::new();
    jinja_env.set_undefined_behavior(minijinja::UndefinedBehavior::Lenient);
    jinja_env.add_template("t", input).map_err(|e| format!("parsing template: {}", e))?;
    let tmpl = jinja_env.get_template("t").map_err(|e| format!("getting template: {}", e))?;
    tmpl.render(minijinja::context!(env => env_map))
        .map_err(|e| format!("executing template: {}", e))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_envsubst_basic() {
        env::set_var("TEST_RENDER_VAR", "hello");
        assert_eq!(envsubst("say ${TEST_RENDER_VAR}"), "say hello");
        assert_eq!(envsubst("say $TEST_RENDER_VAR"), "say hello");
    }
    #[test]
    fn test_envsubst_missing() {
        env::remove_var("MISSING_VAR_XYZ");
        assert_eq!(envsubst("${MISSING_VAR_XYZ}"), "${MISSING_VAR_XYZ}");
    }
    #[test]
    fn test_envsubst_empty() {
        assert_eq!(envsubst(""), "");
    }
    #[test]
    fn test_envsubst_no_vars() {
        assert_eq!(envsubst("no vars here"), "no vars here");
    }
    #[test]
    fn test_envsubst_empty_value() {
        env::set_var("TEST_EMPTY_VAR", "");
        assert_eq!(envsubst("${TEST_EMPTY_VAR}"), "");
    }
    #[test]
    fn test_envsubst_special_chars() {
        env::set_var("TEST_SPECIAL", "a=b&c");
        assert_eq!(envsubst("${TEST_SPECIAL}"), "a=b&c");
    }
    #[test]
    fn test_envsubst_multiline() {
        env::set_var("TEST_ML", "val");
        let input = "line1 ${TEST_ML}\nline2 $TEST_ML";
        let output = envsubst(input);
        assert!(output.contains("line1 val"));
        assert!(output.contains("line2 val"));
    }
    #[test]
    fn test_envsubst_adjacent() {
        env::set_var("TEST_A", "X");
        env::set_var("TEST_B", "Y");
        assert_eq!(envsubst("${TEST_A}${TEST_B}"), "XY");
    }
    #[test]
    fn test_template_basic() {
        env::set_var("TEST_TPL_VAR", "world");
        let result = template_render("hello {{ env.TEST_TPL_VAR }}").unwrap();
        assert_eq!(result, "hello world");
    }
    #[test]
    fn test_template_missing() {
        let result = template_render("{{ env.NONEXISTENT_TPL_VAR_XYZ }}").unwrap();
        assert_eq!(result.trim(), "");
    }
    #[test]
    fn test_template_empty() {
        assert_eq!(template_render("").unwrap(), "");
    }
    #[test]
    fn test_template_invalid() {
        let result = template_render("{{ invalid %}");
        assert!(result.is_err());
    }
    #[test]
    fn test_template_conditional() {
        env::set_var("TEST_COND", "yes");
        let result = template_render("{% if env.TEST_COND %}ok{% endif %}").unwrap();
        assert_eq!(result, "ok");
    }
}
