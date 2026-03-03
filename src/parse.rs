use std::collections::BTreeMap;

use crate::error::GrammarError;
use crate::types::*;

const MAX_REPETITION_THRESHOLD: u64 = 2000;

/// Parsed grammar: rules + symbol table.
#[derive(Debug, Clone)]
pub struct ParsedGrammar {
    pub rules: Rules,
    pub symbol_ids: BTreeMap<String, u32>,
}

impl ParsedGrammar {
    /// Get the rule index for the "root" rule.
    pub fn root_index(&self) -> Option<usize> {
        self.symbol_ids.get("root").map(|&id| id as usize)
    }
}

/// GBNF grammar parser.
struct Parser<'a> {
    src: &'a [u8],
    symbol_ids: BTreeMap<String, u32>,
    rules: Rules,
}

impl<'a> Parser<'a> {
    fn new(src: &'a [u8]) -> Self {
        Self {
            src,
            symbol_ids: BTreeMap::new(),
            rules: Vec::new(),
        }
    }

    fn get_symbol_id(&mut self, name: &str) -> u32 {
        let next_id = self.symbol_ids.len() as u32;
        *self.symbol_ids.entry(name.to_string()).or_insert(next_id)
    }

    fn generate_symbol_id(&mut self, base_name: &str) -> u32 {
        let next_id = self.symbol_ids.len() as u32;
        let name = format!("{}_{}", base_name, next_id);
        self.symbol_ids.insert(name, next_id);
        next_id
    }

    fn add_rule(&mut self, rule_id: u32, rule: Rule) {
        let id = rule_id as usize;
        if self.rules.len() <= id {
            self.rules.resize(id + 1, Vec::new());
        }
        self.rules[id] = rule;
    }

    // ── Lexing helpers ──────────────────────────────────────────────

    /// Skip whitespace, comments; if newline_ok, skip newlines too.
    fn parse_space(&self, mut pos: usize, newline_ok: bool) -> usize {
        while pos < self.src.len() {
            let c = self.src[pos];
            if c == b' ' || c == b'\t' || c == b'#' || (newline_ok && (c == b'\r' || c == b'\n')) {
                if c == b'#' {
                    while pos < self.src.len() && self.src[pos] != b'\r' && self.src[pos] != b'\n' {
                        pos += 1;
                    }
                } else {
                    pos += 1;
                }
            } else {
                break;
            }
        }
        pos
    }

    /// Parse a name: [a-zA-Z0-9_-]+
    fn parse_name(&self, pos: usize) -> Result<(String, usize), GrammarError> {
        let start = pos;
        let mut p = pos;
        while p < self.src.len() && is_word_char(self.src[p]) {
            p += 1;
        }
        if p == start {
            return Err(GrammarError::ParseError(format!(
                "expecting name at position {}",
                pos
            )));
        }
        let name = std::str::from_utf8(&self.src[start..p])
            .map_err(|_| GrammarError::ParseError("invalid UTF-8 in rule name".into()))?;
        Ok((name.to_string(), p))
    }

    /// Parse an integer: [0-9]+
    fn parse_int(&self, pos: usize) -> Result<(u64, usize), GrammarError> {
        let start = pos;
        let mut p = pos;
        while p < self.src.len() && self.src[p].is_ascii_digit() {
            p += 1;
        }
        if p == start {
            return Err(GrammarError::ParseError(format!(
                "expecting integer at position {}",
                pos
            )));
        }
        let s = std::str::from_utf8(&self.src[start..p])
            .map_err(|_| GrammarError::ParseError("invalid UTF-8 in integer".into()))?;
        let val: u64 = s
            .parse()
            .map_err(|_| GrammarError::ParseError(format!("invalid integer: {}", s)))?;
        Ok((val, p))
    }

    /// Parse a hex escape: exactly `size` hex digits.
    fn parse_hex(&self, pos: usize, size: usize) -> Result<(u32, usize), GrammarError> {
        let mut p = pos;
        let end = pos + size;
        let mut value: u32 = 0;
        while p < end && p < self.src.len() && self.src[p] != 0 {
            value <<= 4;
            let c = self.src[p];
            if c.is_ascii_digit() {
                value += (c - b'0') as u32;
            } else if (b'a'..=b'f').contains(&c) {
                value += (c - b'a' + 10) as u32;
            } else if (b'A'..=b'F').contains(&c) {
                value += (c - b'A' + 10) as u32;
            } else {
                break;
            }
            p += 1;
        }
        if p != end {
            return Err(GrammarError::ParseError(format!(
                "expecting {} hex chars at position {}",
                size, pos
            )));
        }
        Ok((value, p))
    }

    /// Parse a single character (with escape handling), return (codepoint, new_pos).
    fn parse_char(&self, pos: usize) -> Result<(u32, usize), GrammarError> {
        if pos >= self.src.len() {
            return Err(GrammarError::ParseError("unexpected end of input".into()));
        }
        if self.src[pos] == b'\\' {
            if pos + 1 >= self.src.len() {
                return Err(GrammarError::ParseError(
                    "unexpected end of input after backslash".into(),
                ));
            }
            match self.src[pos + 1] {
                b'x' => self.parse_hex(pos + 2, 2),
                b'u' => self.parse_hex(pos + 2, 4),
                b'U' => self.parse_hex(pos + 2, 8),
                b't' => Ok((b'\t' as u32, pos + 2)),
                b'r' => Ok((b'\r' as u32, pos + 2)),
                b'n' => Ok((b'\n' as u32, pos + 2)),
                b'\\' | b'"' | b'[' | b']' => Ok((self.src[pos + 1] as u32, pos + 2)),
                _ => Err(GrammarError::ParseError(format!(
                    "unknown escape '\\{}' at position {}",
                    self.src[pos + 1] as char,
                    pos
                ))),
            }
        } else {
            // Decode UTF-8
            decode_utf8_at(self.src, pos)
        }
    }

    // ── Rule parsing ────────────────────────────────────────────────

    fn parse_alternates(
        &mut self,
        pos: usize,
        rule_name: &str,
        rule_id: u32,
        is_nested: bool,
    ) -> Result<usize, GrammarError> {
        let mut rule = Rule::new();
        let mut p = self.parse_sequence(pos, rule_name, &mut rule, is_nested)?;
        while p < self.src.len() && self.src[p] == b'|' {
            rule.push(Element::alt());
            p = self.parse_space(p + 1, true);
            p = self.parse_sequence(p, rule_name, &mut rule, is_nested)?;
        }
        rule.push(Element::end());
        self.add_rule(rule_id, rule);
        Ok(p)
    }

    fn parse_sequence(
        &mut self,
        pos: usize,
        rule_name: &str,
        rule: &mut Rule,
        is_nested: bool,
    ) -> Result<usize, GrammarError> {
        let mut last_sym_start = rule.len();
        let mut p = pos;

        while p < self.src.len() {
            let c = self.src[p];

            if c == b'"' {
                // Literal string
                p += 1;
                last_sym_start = rule.len();
                while p < self.src.len() && self.src[p] != b'"' {
                    let (cp, np) = self.parse_char(p)?;
                    p = np;
                    rule.push(Element::char_(cp));
                }
                if p >= self.src.len() {
                    return Err(GrammarError::ParseError(
                        "unexpected end of input in string literal".into(),
                    ));
                }
                p = self.parse_space(p + 1, is_nested);
            } else if c == b'[' {
                // Character range
                p += 1;
                let start_type = if p < self.src.len() && self.src[p] == b'^' {
                    p += 1;
                    ElementType::CharNot
                } else {
                    ElementType::Char
                };
                last_sym_start = rule.len();
                while p < self.src.len() && self.src[p] != b']' {
                    let (cp, np) = self.parse_char(p)?;
                    p = np;
                    let etype = if rule.len() > last_sym_start {
                        ElementType::CharAlt
                    } else {
                        start_type
                    };
                    rule.push(Element::new(etype, cp));
                    if p < self.src.len()
                        && self.src[p] == b'-'
                        && p + 1 < self.src.len()
                        && self.src[p + 1] != b']'
                    {
                        let (end_cp, np2) = self.parse_char(p + 1)?;
                        p = np2;
                        rule.push(Element::char_rng_upper(end_cp));
                    }
                }
                if p >= self.src.len() {
                    return Err(GrammarError::ParseError(
                        "unexpected end of input in character range".into(),
                    ));
                }
                p = self.parse_space(p + 1, is_nested);
            } else if is_word_char(c) {
                // Rule reference
                let (name, name_end) = self.parse_name(p)?;
                let ref_rule_id = self.get_symbol_id(&name);
                p = self.parse_space(name_end, is_nested);
                last_sym_start = rule.len();
                rule.push(Element::rule_ref(ref_rule_id));
            } else if c == b'(' {
                // Grouping
                p = self.parse_space(p + 1, true);
                let sub_rule_id = self.generate_symbol_id(rule_name);
                p = self.parse_alternates(p, rule_name, sub_rule_id, true)?;
                last_sym_start = rule.len();
                rule.push(Element::rule_ref(sub_rule_id));
                if p >= self.src.len() || self.src[p] != b')' {
                    return Err(GrammarError::ParseError(format!(
                        "expecting ')' at position {}",
                        p
                    )));
                }
                p = self.parse_space(p + 1, is_nested);
            } else if c == b'.' {
                // Any char
                last_sym_start = rule.len();
                rule.push(Element::char_any());
                p = self.parse_space(p + 1, is_nested);
            } else if c == b'*' {
                p = self.parse_space(p + 1, is_nested);
                self.handle_repetitions(rule_name, rule, last_sym_start, 0, u64::MAX)?;
            } else if c == b'+' {
                p = self.parse_space(p + 1, is_nested);
                self.handle_repetitions(rule_name, rule, last_sym_start, 1, u64::MAX)?;
            } else if c == b'?' {
                p = self.parse_space(p + 1, is_nested);
                self.handle_repetitions(rule_name, rule, last_sym_start, 0, 1)?;
            } else if c == b'{' {
                p = self.parse_space(p + 1, is_nested);
                if p >= self.src.len() || !self.src[p].is_ascii_digit() {
                    return Err(GrammarError::ParseError(format!(
                        "expecting integer at position {}",
                        p
                    )));
                }
                let (min_times, np) = self.parse_int(p)?;
                p = self.parse_space(np, is_nested);

                let max_times;
                if p < self.src.len() && self.src[p] == b'}' {
                    max_times = min_times;
                    p = self.parse_space(p + 1, is_nested);
                } else if p < self.src.len() && self.src[p] == b',' {
                    p = self.parse_space(p + 1, is_nested);
                    if p < self.src.len() && self.src[p].is_ascii_digit() {
                        let (val, np2) = self.parse_int(p)?;
                        max_times = val;
                        p = self.parse_space(np2, is_nested);
                    } else {
                        max_times = u64::MAX;
                    }
                    if p >= self.src.len() || self.src[p] != b'}' {
                        return Err(GrammarError::ParseError(format!(
                            "expecting '}}' at position {}",
                            p
                        )));
                    }
                    p = self.parse_space(p + 1, is_nested);
                } else {
                    return Err(GrammarError::ParseError(format!(
                        "expecting ',' or '}}' at position {}",
                        p
                    )));
                }

                let has_max = max_times != u64::MAX;
                if min_times > MAX_REPETITION_THRESHOLD
                    || (has_max && max_times > MAX_REPETITION_THRESHOLD)
                {
                    return Err(GrammarError::ParseError(
                        "repetition count exceeds maximum threshold".into(),
                    ));
                }
                self.handle_repetitions(rule_name, rule, last_sym_start, min_times, max_times)?;
            } else {
                break;
            }
        }
        Ok(p)
    }

    /// Handle repetition modifiers (*, +, ?, {n,m}).
    /// Implements the same rewrite rules as llama.cpp's handle_repetitions lambda.
    fn handle_repetitions(
        &mut self,
        rule_name: &str,
        rule: &mut Rule,
        last_sym_start: usize,
        min_times: u64,
        max_times: u64,
    ) -> Result<(), GrammarError> {
        let no_max = max_times == u64::MAX;

        if last_sym_start == rule.len() {
            return Err(GrammarError::ParseError(
                "expecting preceding item for repetition operator".into(),
            ));
        }

        let prev_rule: Vec<Element> = rule[last_sym_start..].to_vec();

        if min_times == 0 {
            rule.truncate(last_sym_start);
        } else {
            // Repeat (min_times - 1) additional times
            for _ in 1..min_times {
                rule.extend_from_slice(&prev_rule);
            }
        }

        let mut last_rec_rule_id: u32 = 0;
        let n_opt = if no_max { 1 } else { max_times - min_times };

        for i in 0..n_opt {
            let mut rec_rule: Vec<Element> = prev_rule.clone();
            let rec_rule_id = self.generate_symbol_id(rule_name);
            if i > 0 || no_max {
                let ref_id = if no_max {
                    rec_rule_id
                } else {
                    last_rec_rule_id
                };
                rec_rule.push(Element::rule_ref(ref_id));
            }
            rec_rule.push(Element::alt());
            rec_rule.push(Element::end());
            self.add_rule(rec_rule_id, rec_rule);
            last_rec_rule_id = rec_rule_id;
        }

        if n_opt > 0 {
            rule.push(Element::rule_ref(last_rec_rule_id));
        }

        Ok(())
    }

    fn parse_rule(&mut self, pos: usize) -> Result<usize, GrammarError> {
        let (name, name_end) = self.parse_name(pos)?;
        let p = self.parse_space(name_end, false);
        let rule_id = self.get_symbol_id(&name);

        // Check for ::=
        if p + 2 >= self.src.len()
            || self.src[p] != b':'
            || self.src[p + 1] != b':'
            || self.src[p + 2] != b'='
        {
            return Err(GrammarError::ParseError(format!(
                "expecting '::=' at position {}",
                p
            )));
        }
        let p = self.parse_space(p + 3, true);
        let p = self.parse_alternates(p, &name, rule_id, false)?;

        // Skip newline
        let p = if p < self.src.len() && self.src[p] == b'\r' {
            if p + 1 < self.src.len() && self.src[p + 1] == b'\n' {
                p + 2
            } else {
                p + 1
            }
        } else if p < self.src.len() && self.src[p] == b'\n' {
            p + 1
        } else if p < self.src.len() {
            return Err(GrammarError::ParseError(format!(
                "expecting newline or end at position {}",
                p
            )));
        } else {
            p
        };

        Ok(self.parse_space(p, true))
    }

    fn parse_all(&mut self) -> Result<(), GrammarError> {
        let mut p = self.parse_space(0, true);
        while p < self.src.len() {
            p = self.parse_rule(p)?;
        }
        self.validate()
    }

    fn validate(&self) -> Result<(), GrammarError> {
        if self.rules.is_empty() {
            return Err(GrammarError::EmptyGrammar);
        }
        for (idx, rule) in self.rules.iter().enumerate() {
            if rule.is_empty() {
                // Find the name for this rule index
                let name = self
                    .symbol_ids
                    .iter()
                    .find(|(_, &v)| v == idx as u32)
                    .map(|(k, _)| k.as_str())
                    .unwrap_or("unknown");
                return Err(GrammarError::UndefinedRule(name.to_string()));
            }
            for elem in rule {
                if elem.etype == ElementType::RuleRef {
                    let ref_idx = elem.value as usize;
                    if ref_idx >= self.rules.len() || self.rules[ref_idx].is_empty() {
                        let name = self
                            .symbol_ids
                            .iter()
                            .find(|(_, &v)| v == elem.value)
                            .map(|(k, _)| k.as_str())
                            .unwrap_or("unknown");
                        return Err(GrammarError::UndefinedRule(name.to_string()));
                    }
                }
            }
        }
        Ok(())
    }
}

/// Parse a GBNF grammar string into rules and symbol table.
pub fn parse(grammar_text: &str) -> Result<ParsedGrammar, GrammarError> {
    let mut parser = Parser::new(grammar_text.as_bytes());
    parser.parse_all()?;
    Ok(ParsedGrammar {
        rules: parser.rules,
        symbol_ids: parser.symbol_ids,
    })
}

// ── UTF-8 helpers ───────────────────────────────────────────────────

fn is_word_char(c: u8) -> bool {
    c.is_ascii_lowercase() || c.is_ascii_uppercase() || c == b'-' || c.is_ascii_digit()
}

/// Decode a single UTF-8 codepoint starting at `pos` in `src`.
fn decode_utf8_at(src: &[u8], pos: usize) -> Result<(u32, usize), GrammarError> {
    if pos >= src.len() {
        return Err(GrammarError::ParseError("unexpected end of input".into()));
    }
    let first = src[pos];
    let (len, mask): (usize, u8) = if first < 0x80 {
        (1, 0x7F)
    } else if first < 0xE0 {
        (2, 0x1F)
    } else if first < 0xF0 {
        (3, 0x0F)
    } else {
        (4, 0x07)
    };
    let mut value = (first & mask) as u32;
    for i in 1..len {
        if pos + i >= src.len() {
            return Err(GrammarError::ParseError("truncated UTF-8 sequence".into()));
        }
        value = (value << 6) | (src[pos + i] & 0x3F) as u32;
    }
    Ok((value, pos + len))
}

/// Decode a full string to codepoints, with partial UTF-8 state handling.
/// Returns (codepoints_with_terminating_zero, partial_state).
pub fn decode_utf8_string(s: &str) -> Vec<u32> {
    let mut cps: Vec<u32> = s.chars().map(|c| c as u32).collect();
    cps.push(0); // terminating zero, matches C++ convention
    cps
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple() {
        let g = parse(r#"root ::= "a""#).unwrap();
        assert_eq!(g.rules.len(), 1);
        assert_eq!(g.rules[0].len(), 2); // CHAR('a'), END
        assert_eq!(g.rules[0][0], Element::char_(b'a' as u32));
        assert_eq!(g.rules[0][1], Element::end());
    }

    #[test]
    fn test_parse_alternates() {
        let g = parse(r#"root ::= "a" | [bdx-z] | [^1-3]"#).unwrap();
        assert_eq!(g.rules.len(), 1);
        let r = &g.rules[0];
        // "a" | [bdx-z] | [^1-3]
        assert_eq!(r[0], Element::char_(b'a' as u32));
        assert_eq!(r[1], Element::alt());
        assert_eq!(r[2], Element::new(ElementType::Char, b'b' as u32));
        assert_eq!(r[3], Element::char_alt(b'd' as u32));
        assert_eq!(r[4], Element::char_alt(b'x' as u32));
        assert_eq!(r[5], Element::char_rng_upper(b'z' as u32));
        assert_eq!(r[6], Element::alt());
        assert_eq!(r[7], Element::char_not(b'1' as u32));
        assert_eq!(r[8], Element::char_rng_upper(b'3' as u32));
        assert_eq!(r[9], Element::end());
    }

    #[test]
    fn test_parse_plus_rule_ref() {
        // root ::= a+
        // a    ::= "a"
        let g = parse("root ::= a+\na ::= \"a\"").unwrap();
        assert_eq!(g.symbol_ids.len(), 3); // a, root, root_2
        assert!(g.symbol_ids.contains_key("a"));
        assert!(g.symbol_ids.contains_key("root"));
    }

    #[test]
    fn test_parse_plus_literal() {
        let g = parse("root ::= \"a\"+").unwrap();
        assert_eq!(g.symbol_ids.len(), 2); // root, root_1
        let root = &g.rules[*g.symbol_ids.get("root").unwrap() as usize];
        // CHAR('a'), RULE_REF(root_1), END
        assert_eq!(root[0], Element::char_(b'a' as u32));
        assert_eq!(root[1].etype, ElementType::RuleRef);
        assert_eq!(root[2], Element::end());
    }

    #[test]
    fn test_parse_optional() {
        let g = parse("root ::= \"a\"?").unwrap();
        assert_eq!(g.symbol_ids.len(), 2); // root, root_1
        let root = &g.rules[*g.symbol_ids.get("root").unwrap() as usize];
        // RULE_REF(root_1), END
        assert_eq!(root[0].etype, ElementType::RuleRef);
        assert_eq!(root[1], Element::end());
        // root_1: CHAR('a'), ALT, END
        let r1 = &g.rules[root[0].value as usize];
        assert_eq!(r1[0], Element::char_(b'a' as u32));
        assert_eq!(r1[1], Element::alt());
        assert_eq!(r1[2], Element::end());
    }

    #[test]
    fn test_parse_star() {
        let g = parse("root ::= \"a\"*").unwrap();
        let root = &g.rules[*g.symbol_ids.get("root").unwrap() as usize];
        // RULE_REF(root_1), END
        assert_eq!(root[0].etype, ElementType::RuleRef);
        assert_eq!(root[1], Element::end());
        // root_1: CHAR('a'), RULE_REF(self), ALT, END
        let r1 = &g.rules[root[0].value as usize];
        assert_eq!(r1[0], Element::char_(b'a' as u32));
        assert_eq!(r1[1].etype, ElementType::RuleRef);
        assert_eq!(r1[1].value, root[0].value); // self-referential
        assert_eq!(r1[2], Element::alt());
        assert_eq!(r1[3], Element::end());
    }

    #[test]
    fn test_parse_exact_repetition() {
        let g = parse("root ::= \"a\"{2}").unwrap();
        let root = &g.rules[*g.symbol_ids.get("root").unwrap() as usize];
        assert_eq!(root[0], Element::char_(b'a' as u32));
        assert_eq!(root[1], Element::char_(b'a' as u32));
        assert_eq!(root[2], Element::end());
    }

    #[test]
    fn test_parse_min_repetition() {
        let g = parse("root ::= \"a\"{2,}").unwrap();
        let root = &g.rules[*g.symbol_ids.get("root").unwrap() as usize];
        // CHAR('a'), CHAR('a'), RULE_REF(root_1), END
        assert_eq!(root[0], Element::char_(b'a' as u32));
        assert_eq!(root[1], Element::char_(b'a' as u32));
        assert_eq!(root[2].etype, ElementType::RuleRef);
        assert_eq!(root[3], Element::end());
    }

    #[test]
    fn test_parse_range_repetition() {
        let g = parse("root ::= \"a\"{2,4}").unwrap();
        let root = &g.rules[*g.symbol_ids.get("root").unwrap() as usize];
        // CHAR('a'), CHAR('a'), RULE_REF(root_2), END
        assert_eq!(root[0], Element::char_(b'a' as u32));
        assert_eq!(root[1], Element::char_(b'a' as u32));
        assert_eq!(root[2].etype, ElementType::RuleRef);
        assert_eq!(root[3], Element::end());
    }

    #[test]
    fn test_undefined_rule_error() {
        let result = parse("root ::= foo\nbar ::= \"b\"");
        assert!(result.is_err());
    }

    #[test]
    fn test_failure_missing_comma() {
        assert!(parse("root ::= \"a\"{,}").is_err());
    }

    #[test]
    fn test_failure_missing_comma_with_max() {
        assert!(parse("root ::= \"a\"{,10}").is_err());
    }

    #[test]
    fn test_json_grammar() {
        let json_grammar = r#"root   ::= object
value  ::= object | array | string | number | ("true" | "false" | "null") ws
object ::=
  "{" ws (
            string ":" ws value
    ("," ws string ":" ws value)*
  )? "}" ws
array  ::=
  "[" ws (
            value
    ("," ws value)*
  )? "]" ws
string ::=
  "\"" (
    [^"\\\x7F\x00-\x1F] |
    "\\" (["\\bfnrt] | "u" [0-9a-fA-F]{4})
  )* "\"" ws
number ::= ("-"? ([0-9] | [1-9] [0-9]{0,15})) ("." [0-9]+)? ([eE] [-+]? [0-9] [1-9]{0,15})? ws
ws ::= | " " | "\n" [ \t]{0,20}"#;
        let g = parse(json_grammar).unwrap();
        assert!(g.symbol_ids.contains_key("root"));
        assert!(g.symbol_ids.contains_key("value"));
        assert!(g.symbol_ids.contains_key("object"));
        assert!(g.symbol_ids.contains_key("array"));
        assert!(g.symbol_ids.contains_key("string"));
        assert!(g.symbol_ids.contains_key("number"));
        assert!(g.symbol_ids.contains_key("ws"));
    }

    #[test]
    fn test_expression_grammar() {
        let g = parse(
            r#"root  ::= (expr "=" term "\n")+
expr  ::= term ([-+*/] term)*
term  ::= [0-9]+"#,
        )
        .unwrap();
        assert!(g.symbol_ids.contains_key("root"));
        assert!(g.symbol_ids.contains_key("expr"));
        assert!(g.symbol_ids.contains_key("term"));
    }
}
