//! Helpers so that secrets never reach logs or the UI in full.

/// `sk-abcd…wxyz` style masking. Short tokens are fully masked.
pub fn token(token: &str) -> String {
    let n = token.chars().count();
    if n == 0 {
        return "<empty>".into();
    }
    if n <= 10 {
        return "*".repeat(n);
    }
    let head: String = token.chars().take(4).collect();
    let tail: String = token.chars().skip(n - 4).collect();
    format!("{head}…{tail} ({n} chars)")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masks() {
        assert_eq!(token(""), "<empty>");
        assert_eq!(token("short"), "*****");
        assert_eq!(token("tk_live_1234567890abcdef"), "tk_l…cdef (24 chars)");
    }
}
