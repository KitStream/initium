use std::env;
pub fn envsubst(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let bytes = input.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    while i < len {
        if bytes[i] == b'$' && i + 1 < len {
            if bytes[i + 1] == b'{' {
                if let Some((name, end)) = parse_braced_var(input, i + 2) {
                    match env::var(name) {
                        Ok(val) => result.push_str(&val),
                        Err(_) => result.push_str(&input[i..end]),
                    }
                    i = end;
                    continue;
                }
            } else if is_var_start(bytes[i + 1]) {
                let start = i + 1;
                let mut end = start + 1;
                while end < len && is_var_char(bytes[end]) {
                    end += 1;
                }
                let name = &input[start..end];
                match env::var(name) {
                    Ok(val) => result.push_str(&val),
                    Err(_) => result.push_str(&input[i..end]),
                }
                i = end;
                continue;
            }
        }
        result.push(bytes[i] as char);
        i += 1;
    }
    result
}

fn is_var_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_'
}

fn is_var_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn parse_braced_var(input: &str, start: usize) -> Option<(&str, usize)> {
    let bytes = input.as_bytes();
    if start >= bytes.len() || !is_var_start(bytes[start]) {
        return None;
    }
    let mut end = start + 1;
    while end < bytes.len() && is_var_char(bytes[end]) {
        end += 1;
    }
    if end < bytes.len() && bytes[end] == b'}' {
        Some((&input[start..end], end + 1))
    } else {
        None
    }
}
pub fn template_render(input: &str) -> Result<String, String> {
    let env_map: std::collections::HashMap<String, String> = env::vars().collect();
    let mut jinja_env = minijinja::Environment::new();
    jinja_env.set_undefined_behavior(minijinja::UndefinedBehavior::Lenient);
    crate::template_funcs::register(&mut jinja_env);
    jinja_env
        .add_template("t", input)
        .map_err(|e| format!("parsing template: {}", e))?;
    let tmpl = jinja_env
        .get_template("t")
        .map_err(|e| format!("getting template: {}", e))?;
    tmpl.render(minijinja::context!(env => env_map))
        .map_err(|e| format!("executing template: {}", e))
}
#[cfg(test)]
mod tests {
    use super::*;

    use std::ffi::OsString;

    struct EnvGuard {
        name: String,
        previous: Option<OsString>,
    }

    impl EnvGuard {
        fn set(name: &str, value: &str) -> Self {
            let previous = env::var_os(name);
            env::set_var(name, value);
            Self {
                name: name.to_string(),
                previous,
            }
        }

        fn remove(name: &str) -> Self {
            let previous = env::var_os(name);
            env::remove_var(name);
            Self {
                name: name.to_string(),
                previous,
            }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match &self.previous {
                Some(val) => env::set_var(&self.name, val),
                None => env::remove_var(&self.name),
            }
        }
    }

    #[test]
    fn test_envsubst_basic() {
        let _g = EnvGuard::set("TEST_RENDER_VAR", "hello");
        assert_eq!(envsubst("say ${TEST_RENDER_VAR}"), "say hello");
        assert_eq!(envsubst("say $TEST_RENDER_VAR"), "say hello");
    }
    #[test]
    fn test_envsubst_missing() {
        let _g = EnvGuard::remove("MISSING_VAR_XYZ");
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
        let _g = EnvGuard::set("TEST_EMPTY_VAR", "");
        assert_eq!(envsubst("${TEST_EMPTY_VAR}"), "");
    }
    #[test]
    fn test_envsubst_special_chars() {
        let _g = EnvGuard::set("TEST_SPECIAL", "a=b&c");
        assert_eq!(envsubst("${TEST_SPECIAL}"), "a=b&c");
    }
    #[test]
    fn test_envsubst_multiline() {
        let _g = EnvGuard::set("TEST_ML", "val");
        let input = "line1 ${TEST_ML}\nline2 $TEST_ML";
        let output = envsubst(input);
        assert!(output.contains("line1 val"));
        assert!(output.contains("line2 val"));
    }
    #[test]
    fn test_envsubst_adjacent() {
        let _g1 = EnvGuard::set("TEST_A", "X");
        let _g2 = EnvGuard::set("TEST_B", "Y");
        assert_eq!(envsubst("${TEST_A}${TEST_B}"), "XY");
    }
    #[test]
    fn test_template_basic() {
        let _g = EnvGuard::set("TEST_TPL_VAR", "world");
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
    fn test_template_urlencode() {
        let _g = EnvGuard::set("TEST_URLENCODE_VAR", "p@ss%word");
        let result = template_render("{{ env.TEST_URLENCODE_VAR | urlencode }}").unwrap();
        assert_eq!(result, "p%40ss%25word");
    }
    #[test]
    fn test_template_invalid() {
        let result = template_render("{{ invalid %}");
        assert!(result.is_err());
    }
    #[test]
    fn test_template_conditional() {
        let _g = EnvGuard::set("TEST_COND", "yes");
        let result = template_render("{% if env.TEST_COND %}ok{% endif %}").unwrap();
        assert_eq!(result, "ok");
    }
}
