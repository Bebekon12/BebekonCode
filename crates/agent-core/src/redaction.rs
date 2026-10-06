use regex::Regex;
use std::sync::LazyLock;

static SENSITIVE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
    r#"(?im)(authorization|access_token|refresh_token|api[_-]?key|cookie|password|secret)[\s\"']*[:=][^\r\n]*"#
).expect("static redaction pattern")
});
static BEARER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)bearer\s+[A-Za-z0-9._~+/=-]+").expect("static bearer pattern")
});
static KEY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\bsk-[A-Za-z0-9_-]+").expect("static key pattern"));

pub fn redact(input: &str) -> String {
    let masked = SENSITIVE.replace_all(input, "$1=[REDACTED]");
    let masked = BEARER.replace_all(&masked, "Bearer [REDACTED]");
    KEY.replace_all(&masked, "[REDACTED]").into_owned()
}

/// Buffer an entire formatted record so a token split across writes cannot escape redaction.
#[derive(Default)]
pub struct LogWriter {
    buffer: Vec<u8>,
    overflowed: bool,
}
impl std::io::Write for LogWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if self.buffer.len().saturating_add(bytes.len()) > 64 * 1024 {
            self.buffer.clear();
            self.overflowed = true;
        }
        if !self.overflowed {
            self.buffer.extend_from_slice(bytes);
        }
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
impl Drop for LogWriter {
    fn drop(&mut self) {
        use std::io::Write;
        let output = if self.overflowed {
            "[log record omitted: size limit]\n".into()
        } else {
            redact(&String::from_utf8_lossy(&self.buffer))
        };
        let _ = std::io::stderr().lock().write_all(output.as_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn secrets_do_not_survive_redaction() {
        for text in [
            "Authorization: Bearer private-token",
            "refresh_token=private-token",
            "{\"access_token\":\"private-token\"}",
            "https://example.com/?api_key=private-token",
            "Cookie: private-token",
            "password=private-token",
            "sk-private-token",
            "Cookie: session=private-token; refresh=private-token",
            "password=\"private-token with spaces\"",
        ] {
            assert!(!redact(text).contains("private-token"), "leaked: {text}");
        }
    }
}
