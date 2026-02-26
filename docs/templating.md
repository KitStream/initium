# Template Functions

Initium extends the MiniJinja template engine with utility filters for hashing and encoding. These filters are available in all templates — both `render` templates and `seed` spec files.

## Available Filters

### `sha256`

Compute the SHA-256 hash of a string.

```jinja
{{ "hello" | sha256 }}
{# → 2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824 #}
```

**Parameters:**

| Parameter | Type   | Default | Description                         |
| --------- | ------ | ------- | ----------------------------------- |
| `mode`    | string | `"hex"` | Output format: `"hex"` or `"bytes"` |

**Modes:**

- `"hex"` (default) — returns a lowercase hex string (64 characters).
- `"bytes"` — returns an array of 32 byte values (integers 0–255).

```jinja
{{ "hello" | sha256("hex") }}
{# → 2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824 #}

{{ "hello" | sha256("bytes") }}
{# → [44, 242, 77, ...] (32 integers) #}
```

### `base64_encode`

Encode a string to Base64 (standard alphabet with padding).

```jinja
{{ "hello world" | base64_encode }}
{# → aGVsbG8gd29ybGQ= #}
```

### `base64_decode`

Decode a Base64 string back to its original value. Returns an error if the input is not valid Base64 or the decoded bytes are not valid UTF-8.

```jinja
{{ "aGVsbG8gd29ybGQ=" | base64_decode }}
{# → hello world #}
```

## Chaining Filters

Filters can be chained to compose operations:

```jinja
{# SHA-256 hash then Base64-encode the hex digest #}
{{ "secret" | sha256 | base64_encode }}

{# Base64 encode then decode (roundtrip) #}
{{ "data" | base64_encode | base64_decode }}

{# Hash an environment variable value #}
{{ env.API_KEY | sha256 }}
```

## Use Cases

### Content Fingerprinting

Generate a checksum for a config value to detect changes:

```jinja
checksum: {{ env.CONFIG_DATA | sha256 }}
```

### Encoding Secrets

Base64-encode a value for Kubernetes secret manifests:

```jinja
data:
  password: {{ env.DB_PASSWORD | base64_encode }}
```

### Verifying Integrity

Decode and verify Base64-encoded content:

```jinja
decoded_cert: {{ env.B64_CERT | base64_decode }}
```

## Error Handling

| Error                            | Cause                                      |
| -------------------------------- | ------------------------------------------ |
| `sha256: unsupported mode '…'`   | Mode parameter is not `"hex"` or `"bytes"` |
| `base64_decode: invalid input`   | Input string is not valid Base64           |
| `base64_decode: not valid UTF-8` | Decoded bytes are not a valid UTF-8 string |
