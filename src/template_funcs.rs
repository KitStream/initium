use base64::prelude::*;
use minijinja::value::Value;
use sha2::{Digest, Sha256};

/// Register all custom template filters on the given MiniJinja environment.
pub fn register(env: &mut minijinja::Environment<'_>) {
    env.add_filter("sha256", filter_sha256);
    env.add_filter("base64_encode", filter_base64_encode);
    env.add_filter("base64_decode", filter_base64_decode);
}

fn filter_sha256(value: String, mode: Option<String>) -> Result<Value, minijinja::Error> {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    let hash = hasher.finalize();

    match mode.as_deref().unwrap_or("hex") {
        "hex" => Ok(Value::from(hex_encode(&hash))),
        "bytes" => {
            let list: Vec<Value> = hash.iter().map(|b| Value::from(*b as i64)).collect();
            Ok(Value::from(list))
        }
        other => Err(minijinja::Error::new(
            minijinja::ErrorKind::InvalidOperation,
            format!(
                "sha256: unsupported mode '{}' (expected 'hex' or 'bytes')",
                other
            ),
        )),
    }
}

fn filter_base64_encode(value: Value) -> Result<String, minijinja::Error> {
    // String values are encoded directly; byte sequences (from sha256 bytes
    // mode) are collected into a Vec<u8> first.
    if value.is_undefined()
        || value.is_none()
        || value.kind() == minijinja::value::ValueKind::String
    {
        let s = value.to_string();
        return Ok(BASE64_STANDARD.encode(s.as_bytes()));
    }
    if let Ok(items) = value.try_iter() {
        let bytes: Vec<u8> = items
            .map(|v| {
                let n = i64::try_from(v.clone()).map_err(|_| {
                    minijinja::Error::new(
                        minijinja::ErrorKind::InvalidOperation,
                        "base64_encode: byte sequence contains non-integer value",
                    )
                })?;
                u8::try_from(n).map_err(|_| {
                    minijinja::Error::new(
                        minijinja::ErrorKind::InvalidOperation,
                        "base64_encode: byte value out of 0..255 range",
                    )
                })
            })
            .collect::<Result<_, _>>()?;
        Ok(BASE64_STANDARD.encode(&bytes))
    } else {
        let s = value.to_string();
        Ok(BASE64_STANDARD.encode(s.as_bytes()))
    }
}

fn filter_base64_decode(value: String) -> Result<String, minijinja::Error> {
    let bytes = BASE64_STANDARD.decode(value.as_bytes()).map_err(|e| {
        minijinja::Error::new(
            minijinja::ErrorKind::InvalidOperation,
            format!("base64_decode: invalid input: {}", e),
        )
    })?;
    String::from_utf8(bytes).map_err(|e| {
        minijinja::Error::new(
            minijinja::ErrorKind::InvalidOperation,
            format!("base64_decode: result is not valid UTF-8: {}", e),
        )
    })
}

fn hex_encode(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(s, "{:02x}", b);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sha256_hex() {
        let result = filter_sha256("hello".into(), Some("hex".into())).unwrap();
        assert_eq!(
            result.to_string(),
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    #[test]
    fn test_sha256_default_is_hex() {
        let result = filter_sha256("hello".into(), None).unwrap();
        assert_eq!(
            result.to_string(),
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    #[test]
    fn test_sha256_bytes() {
        let result = filter_sha256("hello".into(), Some("bytes".into())).unwrap();
        let items: Vec<Value> = result.try_iter().expect("should be iterable").collect();
        assert_eq!(items.len(), 32);
        // First byte of sha256("hello") is 0x2c = 44
        assert_eq!(i64::try_from(items[0].clone()).unwrap(), 0x2c);
    }

    #[test]
    fn test_sha256_invalid_mode() {
        let result = filter_sha256("hello".into(), Some("raw".into()));
        assert!(result.is_err());
    }

    #[test]
    fn test_sha256_empty_input() {
        let result = filter_sha256(String::new(), None).unwrap();
        assert_eq!(
            result.to_string(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn test_base64_encode() {
        assert_eq!(
            filter_base64_encode(Value::from("hello")).unwrap(),
            "aGVsbG8="
        );
    }

    #[test]
    fn test_base64_encode_empty() {
        assert_eq!(filter_base64_encode(Value::from("")).unwrap(), "");
    }

    #[test]
    fn test_base64_decode() {
        let result = filter_base64_decode("aGVsbG8=".into()).unwrap();
        assert_eq!(result, "hello");
    }

    #[test]
    fn test_base64_decode_empty() {
        let result = filter_base64_decode(String::new()).unwrap();
        assert_eq!(result, "");
    }

    #[test]
    fn test_base64_roundtrip() {
        let original = "initium test data with special chars: é ñ ü";
        let encoded = filter_base64_encode(Value::from(original)).unwrap();
        let decoded = filter_base64_decode(encoded).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn test_base64_decode_invalid() {
        let result = filter_base64_decode("not-valid-base64!!!".into());
        assert!(result.is_err());
    }

    #[test]
    fn test_template_sha256_filter() {
        let mut env = minijinja::Environment::new();
        register(&mut env);
        env.add_template("t", r#"{{ "hello" | sha256 }}"#).unwrap();
        let tmpl = env.get_template("t").unwrap();
        let result = tmpl.render(minijinja::context!()).unwrap();
        assert_eq!(
            result,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    #[test]
    fn test_template_base64_encode_filter() {
        let mut env = minijinja::Environment::new();
        register(&mut env);
        env.add_template("t", r#"{{ "hello" | base64_encode }}"#)
            .unwrap();
        let tmpl = env.get_template("t").unwrap();
        let result = tmpl.render(minijinja::context!()).unwrap();
        assert_eq!(result, "aGVsbG8=");
    }

    #[test]
    fn test_template_base64_decode_filter() {
        let mut env = minijinja::Environment::new();
        register(&mut env);
        env.add_template("t", r#"{{ "aGVsbG8=" | base64_decode }}"#)
            .unwrap();
        let tmpl = env.get_template("t").unwrap();
        let result = tmpl.render(minijinja::context!()).unwrap();
        assert_eq!(result, "hello");
    }

    #[test]
    fn test_template_chained_sha256_then_base64() {
        let mut env = minijinja::Environment::new();
        register(&mut env);
        env.add_template("t", r#"{{ "hello" | sha256 | base64_encode }}"#)
            .unwrap();
        let tmpl = env.get_template("t").unwrap();
        let result = tmpl.render(minijinja::context!()).unwrap();
        // base64 of the hex sha256 of "hello"
        let expected = BASE64_STANDARD
            .encode("2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824");
        assert_eq!(result, expected);
    }

    #[test]
    fn test_sha256_bytes_then_base64_known_vector() {
        let mut env = minijinja::Environment::new();
        register(&mut env);
        env.add_template(
            "t",
            r#"{{ "nbp_TestSecretValue1234567890ABCDE05m4Dm" | sha256("bytes") | base64_encode }}"#,
        )
        .unwrap();
        let tmpl = env.get_template("t").unwrap();
        let result = tmpl.render(minijinja::context!()).unwrap();
        assert_eq!(result, "7X/8tpDCEeSF536pQUogANtV0NHanRgRpN/JS4UJNKg=");
    }

    #[test]
    fn test_template_chained_base64_roundtrip() {
        let mut env = minijinja::Environment::new();
        register(&mut env);
        env.add_template("t", r#"{{ "secret" | base64_encode | base64_decode }}"#)
            .unwrap();
        let tmpl = env.get_template("t").unwrap();
        let result = tmpl.render(minijinja::context!()).unwrap();
        assert_eq!(result, "secret");
    }
}
