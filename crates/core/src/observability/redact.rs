// MIT License
// Copyright (c) 2025 fi-code contributors
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

// 凭据脱敏与超大 payload 截断
// 工作流：先按 UTF-8 字符边界截断到 MAX_ATTR_BYTES，再依次套用所有正则替换规则

use once_cell::sync::Lazy;
use regex::Regex;

/// OTel 属性单字段的最大字节数。超过则截断，避免 trace payload 撑爆后端
pub const MAX_ATTR_BYTES: usize = 50 * 1024;

// 凭据正则规则集：先匹配更具体的前缀（sk-ant- / sk-lf- / pk-lf-），再匹配通用 sk- 前缀
// 顺序很关键：通用规则放后面，避免提前吞掉特定前缀的 key
static PATTERNS: Lazy<Vec<(Regex, &'static str)>> = Lazy::new(|| {
    vec![
        (
            Regex::new(r"sk-ant-[A-Za-z0-9_\-]{20,}").unwrap(),
            "sk-ant-***REDACTED***",
        ),
        (
            Regex::new(r"sk-lf-[A-Za-z0-9_\-]{20,}").unwrap(),
            "sk-lf-***REDACTED***",
        ),
        (
            Regex::new(r"pk-lf-[A-Za-z0-9_\-]{20,}").unwrap(),
            "pk-lf-***REDACTED***",
        ),
        (
            Regex::new(r"sk-[A-Za-z0-9_\-]{20,}").unwrap(),
            "sk-***REDACTED***",
        ),
        (
            Regex::new(r"(?i)ANTHROPIC_API_KEY\s*[:=]\s*\S+").unwrap(),
            "ANTHROPIC_API_KEY=***REDACTED***",
        ),
        (
            Regex::new(r"(?i)OPENAI_API_KEY\s*[:=]\s*\S+").unwrap(),
            "OPENAI_API_KEY=***REDACTED***",
        ),
        (
            Regex::new(r"Bearer\s+[A-Za-z0-9._\-]{20,}").unwrap(),
            "Bearer ***REDACTED***",
        ),
        (
            Regex::new(r"(?i)Authorization\s*:\s*Basic\s+[A-Za-z0-9+/=]{20,}").unwrap(),
            "Authorization: Basic ***REDACTED***",
        ),
        (
            Regex::new(r#"(?i)password["']?\s*[:=]\s*["']?[^\s"',}]+"#).unwrap(),
            "password=***REDACTED***",
        ),
    ]
});

/// 对输入字符串先截断再脱敏，返回安全可上报的 String
pub fn redact_and_truncate(input: &str) -> String {
    let truncated = truncate_utf8(input, MAX_ATTR_BYTES);
    let mut result = truncated.to_string();
    for (re, replacement) in PATTERNS.iter() {
        result = re.replace_all(&result, *replacement).into_owned();
    }
    result
}

/// 按 UTF-8 字符边界安全地截断字符串到不超过 max_bytes 字节
/// 注意：直接对 &str 做字节切片会 panic（若切到 char 中间），所以必须扫描字符边界
fn truncate_utf8(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    // 找到不大于 max_bytes 的最大字符边界
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redact_openai_key() {
        let input = "my key is sk-abcdefghijklmnopqrstuvwxyz123";
        let output = redact_and_truncate(input);
        assert!(output.contains("sk-***REDACTED***"));
        assert!(!output.contains("abcdefghijklmnopqrstuvwxyz123"));
    }

    #[test]
    fn test_redact_anthropic_key() {
        let input = "claude key sk-ant-api03-aaaaaaaaaaaaaaaaaaaaaaaa";
        let output = redact_and_truncate(input);
        assert!(output.contains("sk-ant-***REDACTED***"));
        assert!(!output.contains("api03-aaaa"));
    }

    #[test]
    fn test_redact_langfuse_keys() {
        let input = "pub pk-lf-12345678901234567890abcd and sec sk-lf-09876543210987654321xyzw";
        let output = redact_and_truncate(input);
        assert!(output.contains("pk-lf-***REDACTED***"));
        assert!(output.contains("sk-lf-***REDACTED***"));
    }

    #[test]
    fn test_redact_bearer() {
        let input = "Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.xxx";
        let output = redact_and_truncate(input);
        assert!(output.contains("Bearer ***REDACTED***"));
        assert!(!output.contains("eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9"));
    }

    #[test]
    fn test_redact_basic_auth() {
        let input = "header Authorization: Basic dXNlcjpwYXNzd29yZDEyMzQ1Ng==";
        let output = redact_and_truncate(input);
        assert!(output.contains("Authorization: Basic ***REDACTED***"));
        assert!(!output.contains("dXNlcjpwYXNz"));
    }

    #[test]
    fn test_redact_password() {
        let input = r#"{"password": "supersecret123"}"#;
        let output = redact_and_truncate(input);
        assert!(output.contains("password=***REDACTED***"));
        assert!(!output.contains("supersecret123"));
    }

    #[test]
    fn test_redact_env_assignment() {
        let input = "OPENAI_API_KEY=sk-realkey1234567890abcdefghij and ANTHROPIC_API_KEY=sk-ant-realkey1234567890abcd";
        let output = redact_and_truncate(input);
        assert!(output.contains("OPENAI_API_KEY=***REDACTED***"));
        assert!(output.contains("ANTHROPIC_API_KEY=***REDACTED***"));
    }

    #[test]
    fn test_no_false_positive_on_plain_text() {
        let input = "the quick brown fox jumps over the lazy dog";
        let output = redact_and_truncate(input);
        assert_eq!(output, input);
    }

    #[test]
    fn test_truncate_within_50kb_then_redact() {
        // 构造一个 60KB 的字符串，末尾插入一个 key
        let big = "a".repeat(60 * 1024);
        let input = format!("{}sk-shouldbecutoff1234567890abcdef", big);
        let output = redact_and_truncate(&input);
        // 必须 <= 50KB
        assert!(output.len() <= MAX_ATTR_BYTES);
        // key 在 60KB 之后，已被截掉
        assert!(!output.contains("shouldbecutoff"));
    }

    #[test]
    fn test_truncate_char_boundary_safe() {
        // 构造大量多字节字符（中文），刚好处于 50KB 边界附近
        // 每个汉字占 3 字节，约 17000 字符
        let s = "中".repeat(20000);
        let output = redact_and_truncate(&s);
        // 不应 panic（字符边界）；长度受限
        assert!(output.len() <= MAX_ATTR_BYTES);
        // 输出必须是合法 UTF-8（通过 String 已保证）；末尾不会出现破损序列
        assert!(output.chars().all(|c| c == '中'));
    }
}
