use crate::error::GrammarError;
use crate::parse::{self, ParsedGrammar};
use crate::types::*;

const MAX_RECURSION_DEPTH: usize = 512;

/// A compiled grammar ready for constrained decoding.
#[derive(Debug, Clone)]
pub struct Grammar {
    pub(crate) rules: Rules,
    pub(crate) root_index: usize,
    #[allow(dead_code)]
    pub(crate) symbol_names: Vec<String>,
}

/// Runtime grammar state for constrained decoding.
/// Created from a Grammar, advanced token-by-token.
#[derive(Debug, Clone)]
pub struct GrammarState {
    grammar: Grammar,
    stacks: Stacks,
}

impl Grammar {
    /// Parse a GBNF grammar string and compile it.
    pub fn new(grammar_text: &str) -> Result<Self, GrammarError> {
        Self::with_root(grammar_text, "root")
    }

    /// Parse with a custom root rule name.
    pub fn with_root(grammar_text: &str, root_name: &str) -> Result<Self, GrammarError> {
        let parsed = parse::parse(grammar_text)?;
        Self::from_parsed(parsed, root_name)
    }

    fn from_parsed(parsed: ParsedGrammar, root_name: &str) -> Result<Self, GrammarError> {
        let root_index = *parsed
            .symbol_ids
            .get(root_name)
            .ok_or(GrammarError::NoRootRule)? as usize;

        if root_index >= parsed.rules.len() {
            return Err(GrammarError::InvalidStartRule(root_index));
        }

        // Build name lookup (index → name) for diagnostics
        let mut symbol_names = vec![String::new(); parsed.rules.len()];
        for (name, &id) in &parsed.symbol_ids {
            let idx = id as usize;
            if idx < symbol_names.len() {
                symbol_names[idx] = name.clone();
            }
        }

        // Check for left recursion
        detect_left_recursion(&parsed.rules)?;

        Ok(Grammar {
            rules: parsed.rules,
            root_index,
            symbol_names,
        })
    }

    /// Number of rules.
    pub fn num_rules(&self) -> usize {
        self.rules.len()
    }
}

impl GrammarState {
    /// Initialize from a compiled grammar. Expands the root rule into initial stacks.
    pub fn new(grammar: Grammar) -> Result<Self, GrammarError> {
        let stacks = init_stacks(&grammar.rules, grammar.root_index)?;
        Ok(GrammarState { grammar, stacks })
    }

    /// Check if the grammar is in an accepting state (at least one empty stack).
    pub fn is_accepting(&self) -> bool {
        self.stacks.iter().any(|s| s.is_empty())
    }

    /// Check if the grammar has any valid continuation.
    pub fn is_valid(&self) -> bool {
        !self.stacks.is_empty()
    }

    /// Get a bitmask of which tokens are allowed at this position.
    /// `vocab[i]` is the string representation of token ID `i`.
    /// Returns a Vec<bool> of length vocab.len().
    pub fn allowed_tokens(&self, vocab: &[&str]) -> Vec<bool> {
        let mut allowed = vec![false; vocab.len()];

        // If any stack is empty, the grammar can accept EOG
        let allow_eog = self.is_accepting();

        for (i, token_str) in vocab.iter().enumerate() {
            if token_str.is_empty() {
                // Empty tokens are allowed only if we're in accepting state
                allowed[i] = allow_eog;
                continue;
            }

            let codepoints = crate::parse::decode_utf8_string(token_str);
            // codepoints has trailing 0; actual chars are [0..len-1]
            let char_count = codepoints.len() - 1;
            if char_count == 0 {
                allowed[i] = allow_eog;
                continue;
            }

            // Check if this token can be consumed by any current stack
            allowed[i] = self.can_accept_token(&codepoints[..char_count]);
        }

        allowed
    }

    /// Check if a specific token string can be consumed from the current state.
    fn can_accept_token(&self, codepoints: &[u32]) -> bool {
        // Try every current stack
        for stack in &self.stacks {
            if self.can_accept_codepoints_from_stack(stack, codepoints) {
                return true;
            }
        }
        false
    }

    /// Check if a sequence of codepoints can be consumed starting from a single stack.
    fn can_accept_codepoints_from_stack(&self, stack: &Stack, codepoints: &[u32]) -> bool {
        if codepoints.is_empty() {
            return true;
        }

        if stack.is_empty() {
            // Stack is completed but we still have chars to consume
            return false;
        }

        let pos = *stack.last().unwrap();
        let elem = self.grammar.rules[pos.0][pos.1];

        // Must be at a char element
        if !matches!(
            elem.etype,
            ElementType::Char | ElementType::CharNot | ElementType::CharAny
        ) {
            return false;
        }

        let (matched, after_pos) = match_char(&self.grammar.rules, pos, codepoints[0]);
        if !matched {
            return false;
        }

        // Build new stack after consuming this character
        let mut new_stack: Stack = stack[..stack.len() - 1].to_vec();
        if !self.grammar.rules[after_pos.0][after_pos.1].is_end_of_sequence() {
            new_stack.push(after_pos);
        }

        // Expand the new stack (resolve rule refs)
        let mut expanded_stacks = Stacks::new();
        advance_stack(&self.grammar.rules, &new_stack, &mut expanded_stacks, 0);

        // Recurse for remaining codepoints
        let remaining = &codepoints[1..];
        if remaining.is_empty() {
            return true; // consumed all chars
        }
        for es in &expanded_stacks {
            if self.can_accept_codepoints_from_stack(es, remaining) {
                return true;
            }
        }
        false
    }

    /// Accept a token's text and advance the grammar state.
    /// Call after sampling, with the chosen token's string.
    pub fn accept_token(&mut self, token_text: &str) -> Result<(), GrammarError> {
        let codepoints = crate::parse::decode_utf8_string(token_text);
        // chars are [0..len-1], last is terminating 0
        let chars = &codepoints[..codepoints.len() - 1];

        let mut current_stacks = self.stacks.clone();

        for &cp in chars {
            let mut next_stacks = Stacks::new();
            for stack in &current_stacks {
                accept_char(&self.grammar.rules, stack, cp, &mut next_stacks);
            }
            current_stacks = next_stacks;
            if current_stacks.is_empty() {
                return Err(GrammarError::InvalidState(format!(
                    "no valid stacks after accepting character U+{:04X} in '{}'",
                    cp, token_text
                )));
            }
        }

        self.stacks = current_stacks;
        Ok(())
    }

    /// Reset to initial state.
    pub fn reset(&mut self) -> Result<(), GrammarError> {
        self.stacks = init_stacks(&self.grammar.rules, self.grammar.root_index)?;
        Ok(())
    }

    /// Current number of active stacks (for diagnostics).
    pub fn num_stacks(&self) -> usize {
        self.stacks.len()
    }

    // Expose stacks for testing
    #[doc(hidden)]
    pub fn stacks(&self) -> &Stacks {
        &self.stacks
    }
}

// ── Core algorithms ─────────────────────────────────────────────────

/// Initialize stacks from the root rule.
fn init_stacks(rules: &Rules, start_rule_index: usize) -> Result<Stacks, GrammarError> {
    if start_rule_index >= rules.len() {
        return Err(GrammarError::InvalidStartRule(start_rule_index));
    }

    let rule = &rules[start_rule_index];
    let mut stacks = Stacks::new();
    let mut ei = 0;

    loop {
        let mut stack = Stack::new();
        if !rule[ei].is_end_of_sequence() {
            stack.push((start_rule_index, ei));
        }
        advance_stack(rules, &stack, &mut stacks, 0);
        // Scan to end of this alternate
        while !rule[ei].is_end_of_sequence() {
            ei += 1;
        }
        if rule[ei].etype == ElementType::Alt {
            ei += 1; // next alternate
        } else {
            break;
        }
    }

    Ok(stacks)
}

/// Expand a stack until all tops are at terminal elements (char ranges).
/// This is the direct port of llama_grammar_advance_stack.
fn advance_stack(rules: &Rules, stack: &Stack, new_stacks: &mut Stacks, depth: usize) {
    if depth > MAX_RECURSION_DEPTH {
        return; // Fix for C++ bug #18988: prevent stack overflow
    }

    if stack.is_empty() {
        if !new_stacks.contains(stack) {
            new_stacks.push(stack.clone());
        }
        return;
    }

    let pos = *stack.last().unwrap();
    let elem = rules[pos.0][pos.1];

    match elem.etype {
        ElementType::RuleRef => {
            let ref_rule_id = elem.value as usize;
            let ref_rule = &rules[ref_rule_id];
            let mut subpos = 0;

            loop {
                let mut new_stack: Stack = stack[..stack.len() - 1].to_vec();

                // If this rule ref is followed by another element, push continuation
                let next_pos = (pos.0, pos.1 + 1);
                if !rules[next_pos.0][next_pos.1].is_end_of_sequence() {
                    new_stack.push(next_pos);
                }

                // If this alternate of the referenced rule is non-empty, push it
                if !ref_rule[subpos].is_end_of_sequence() {
                    new_stack.push((ref_rule_id, subpos));
                }

                advance_stack(rules, &new_stack, new_stacks, depth + 1);

                // Scan to end of this alternate in the referenced rule
                while !ref_rule[subpos].is_end_of_sequence() {
                    subpos += 1;
                }
                if ref_rule[subpos].etype == ElementType::Alt {
                    subpos += 1;
                } else {
                    break;
                }
            }
        }
        ElementType::Char | ElementType::CharNot | ElementType::CharAny => {
            if !new_stacks.contains(stack) {
                new_stacks.push(stack.clone());
            }
        }
        _ => {
            // End/Alt/CharRngUpper/CharAlt should never be on top of stack
            // (CharRngUpper and CharAlt are always part of a char sequence
            // and the stack should point to the start of the sequence)
        }
    }
}

/// Check if a character matches at a position. Returns (matched, pos_after_char_elements).
fn match_char(rules: &Rules, pos: Pos, chr: u32) -> (bool, Pos) {
    let rule = &rules[pos.0];
    let mut ei = pos.1;
    let elem = rule[ei];

    let is_positive = elem.etype == ElementType::Char || elem.etype == ElementType::CharAny;
    debug_assert!(
        is_positive || elem.etype == ElementType::CharNot,
        "match_char called on non-char element: {:?}",
        elem.etype
    );

    let mut found = false;

    loop {
        if ei + 1 < rule.len() && rule[ei + 1].etype == ElementType::CharRngUpper {
            // Inclusive range
            found = found || (rule[ei].value <= chr && chr <= rule[ei + 1].value);
            ei += 2;
        } else if rule[ei].etype == ElementType::CharAny {
            found = true;
            ei += 1;
        } else {
            // Exact match
            found = found || rule[ei].value == chr;
            ei += 1;
        }

        if ei >= rule.len() || rule[ei].etype != ElementType::CharAlt {
            break;
        }
    }

    (found == is_positive, (pos.0, ei))
}

/// Accept a character on a single stack, producing new expanded stacks.
fn accept_char(rules: &Rules, stack: &Stack, chr: u32, new_stacks: &mut Stacks) {
    if stack.is_empty() {
        return;
    }

    let pos = *stack.last().unwrap();
    let elem = rules[pos.0][pos.1];

    if !matches!(
        elem.etype,
        ElementType::Char | ElementType::CharNot | ElementType::CharAny
    ) {
        return;
    }

    let (matched, after_pos) = match_char(rules, pos, chr);
    if matched {
        let mut new_stack: Stack = stack[..stack.len() - 1].to_vec();
        if !rules[after_pos.0][after_pos.1].is_end_of_sequence() {
            new_stack.push(after_pos);
        }
        advance_stack(rules, &new_stack, new_stacks, 0);
    }
}

// ── Left recursion detection ────────────────────────────────────────

fn detect_left_recursion(rules: &Rules) -> Result<(), GrammarError> {
    let n = rules.len();
    let mut visited = vec![false; n];
    let mut in_progress = vec![false; n];
    let mut may_be_empty = vec![false; n];

    for i in 0..n {
        if visited[i] {
            continue;
        }
        if detect_lr_recursive(rules, i, &mut visited, &mut in_progress, &mut may_be_empty) {
            return Err(GrammarError::LeftRecursion(i));
        }
    }
    Ok(())
}

fn detect_lr_recursive(
    rules: &Rules,
    rule_index: usize,
    visited: &mut [bool],
    in_progress: &mut [bool],
    may_be_empty: &mut [bool],
) -> bool {
    if in_progress[rule_index] {
        return true;
    }
    in_progress[rule_index] = true;

    let rule = &rules[rule_index];

    // Check if rule might produce empty string
    let mut at_rule_start = true;
    for elem in rule {
        if elem.is_end_of_sequence() {
            if at_rule_start {
                may_be_empty[rule_index] = true;
                break;
            }
            at_rule_start = true;
        } else {
            at_rule_start = false;
        }
    }

    // Recurse into leftmost nonterminals
    let mut recurse_into_nonterminal = true;
    for elem in rule {
        if elem.etype == ElementType::RuleRef && recurse_into_nonterminal {
            let ref_idx = elem.value as usize;
            if detect_lr_recursive(rules, ref_idx, visited, in_progress, may_be_empty) {
                return true;
            }
            if !may_be_empty[ref_idx] {
                recurse_into_nonterminal = false;
            }
        } else {
            recurse_into_nonterminal = elem.is_end_of_sequence();
        }
    }

    in_progress[rule_index] = false;
    visited[rule_index] = true;
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_simple() {
        let g = Grammar::new(r#"root ::= "a""#).unwrap();
        let state = GrammarState::new(g).unwrap();
        assert!(state.is_valid());
        assert!(!state.is_accepting()); // haven't consumed 'a' yet
    }

    #[test]
    fn test_accept_simple() {
        let g = Grammar::new(r#"root ::= "a""#).unwrap();
        let mut state = GrammarState::new(g).unwrap();
        state.accept_token("a").unwrap();
        assert!(state.is_accepting());
    }

    #[test]
    fn test_accept_reject() {
        let g = Grammar::new(r#"root ::= "a""#).unwrap();
        let mut state = GrammarState::new(g).unwrap();
        let result = state.accept_token("b");
        assert!(result.is_err());
    }

    #[test]
    fn test_accept_sequence() {
        let g = Grammar::new(r#"root ::= "abc""#).unwrap();
        let mut state = GrammarState::new(g).unwrap();
        // Accept as one token
        state.accept_token("abc").unwrap();
        assert!(state.is_accepting());
    }

    #[test]
    fn test_accept_sequence_char_by_char() {
        let g = Grammar::new(r#"root ::= "abc""#).unwrap();
        let mut state = GrammarState::new(g).unwrap();
        state.accept_token("a").unwrap();
        assert!(!state.is_accepting());
        state.accept_token("b").unwrap();
        assert!(!state.is_accepting());
        state.accept_token("c").unwrap();
        assert!(state.is_accepting());
    }

    #[test]
    fn test_alternates() {
        let g = Grammar::new(r#"root ::= "a" | "b" | "c""#).unwrap();
        let mut state = GrammarState::new(g).unwrap();
        state.accept_token("b").unwrap();
        assert!(state.is_accepting());
    }

    #[test]
    fn test_char_range() {
        let g = Grammar::new(r#"root ::= [a-z]+"#).unwrap();
        let mut state = GrammarState::new(g).unwrap();
        state.accept_token("h").unwrap();
        state.accept_token("e").unwrap();
        state.accept_token("l").unwrap();
        state.accept_token("l").unwrap();
        state.accept_token("o").unwrap();
        assert!(state.is_accepting());
    }

    #[test]
    fn test_star_empty() {
        let g = Grammar::new(r#"root ::= "a"*"#).unwrap();
        let state = GrammarState::new(g).unwrap();
        // * allows empty match
        assert!(state.is_accepting());
    }

    #[test]
    fn test_star_multiple() {
        let g = Grammar::new(r#"root ::= "a"*"#).unwrap();
        let mut state = GrammarState::new(g).unwrap();
        state.accept_token("a").unwrap();
        state.accept_token("a").unwrap();
        state.accept_token("a").unwrap();
        assert!(state.is_accepting());
    }

    #[test]
    fn test_plus_nonempty() {
        let g = Grammar::new(r#"root ::= "a"+"#).unwrap();
        let state = GrammarState::new(g).unwrap();
        assert!(!state.is_accepting()); // + requires at least one
    }

    #[test]
    fn test_allowed_tokens_simple() {
        let g = Grammar::new(r#"root ::= "a" | "b""#).unwrap();
        let state = GrammarState::new(g).unwrap();
        let vocab = vec!["a", "b", "c", "ab"];
        let allowed = state.allowed_tokens(&vocab);
        assert!(allowed[0]); // "a"
        assert!(allowed[1]); // "b"
        assert!(!allowed[2]); // "c"
        assert!(!allowed[3]); // "ab" — would need to match exactly, but grammar only accepts single char
    }

    #[test]
    fn test_allowed_tokens_sequence() {
        let g = Grammar::new(r#"root ::= "ab""#).unwrap();
        let state = GrammarState::new(g).unwrap();
        let vocab = vec!["a", "b", "ab", "abc", "ba"];
        let allowed = state.allowed_tokens(&vocab);
        assert!(allowed[0]); // "a" — partial match is ok
        assert!(!allowed[1]); // "b" — doesn't match start
        assert!(allowed[2]); // "ab" — full match
        assert!(!allowed[3]); // "abc" — too long
        assert!(!allowed[4]); // "ba" — wrong order
    }

    #[test]
    fn test_left_recursion_detected() {
        let result = Grammar::new(r#"root ::= "a" | root "a""#);
        assert!(result.is_err());
    }

    #[test]
    fn test_left_recursion_indirect() {
        let result = Grammar::new("root ::= asdf\nasdf ::= \"a\" | asdf \"a\"");
        assert!(result.is_err());
    }

    #[test]
    fn test_left_recursion_via_empty() {
        let result = Grammar::new(
            "root ::= asdf\nasdf ::= \"a\" | foo \"b\"\nfoo ::= \"c\" | empty asdf \"d\" | \"e\"\nempty ::= \"blah\" | ",
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_dot_any_char() {
        let g = Grammar::new(r#"root ::= ... "abc" ..."#).unwrap();
        let mut state = GrammarState::new(g).unwrap();
        // 3 any + "abc" + 3 any = 9 chars
        state.accept_token("x").unwrap();
        state.accept_token("y").unwrap();
        state.accept_token("z").unwrap();
        state.accept_token("a").unwrap();
        state.accept_token("b").unwrap();
        state.accept_token("c").unwrap();
        state.accept_token("1").unwrap();
        state.accept_token("2").unwrap();
        state.accept_token("3").unwrap();
        assert!(state.is_accepting());
    }

    #[test]
    fn test_negated_range() {
        let g = Grammar::new(r#"root ::= [^0-9]+"#).unwrap();
        let mut state = GrammarState::new(g).unwrap();
        state.accept_token("a").unwrap();
        state.accept_token("b").unwrap();
        assert!(state.is_accepting());
    }

    #[test]
    fn test_negated_range_fails() {
        let g = Grammar::new(r#"root ::= [^0-9]+"#).unwrap();
        let mut state = GrammarState::new(g).unwrap();
        let result = state.accept_token("5");
        assert!(result.is_err());
    }

    #[test]
    fn test_expression_grammar() {
        let g = Grammar::new(
            r#"root ::= expr
expr ::= term ("+" term)*
term ::= number
number ::= [0-9]+"#,
        )
        .unwrap();
        let mut state = GrammarState::new(g).unwrap();
        // "42"
        state.accept_token("4").unwrap();
        state.accept_token("2").unwrap();
        assert!(state.is_accepting());
    }

    #[test]
    fn test_expression_grammar_complex() {
        let g = Grammar::new(
            r#"root ::= expr
expr ::= term ("+" term)*
term ::= number
number ::= [0-9]+"#,
        )
        .unwrap();
        let mut state = GrammarState::new(g).unwrap();
        // "1+2+3"
        for c in "1+2+3".chars() {
            state.accept_token(&c.to_string()).unwrap();
        }
        assert!(state.is_accepting());
    }

    #[test]
    fn test_expression_grammar_trailing_plus_fails() {
        let g = Grammar::new(
            r#"root ::= expr
expr ::= term ("+" term)*
term ::= number
number ::= [0-9]+"#,
        )
        .unwrap();
        let mut state = GrammarState::new(g).unwrap();
        // "42+" — should not accept (trailing operator)
        for c in "42+".chars() {
            let _ = state.accept_token(&c.to_string());
        }
        assert!(!state.is_accepting());
    }

    #[test]
    fn test_quantifier_exact() {
        let g = Grammar::new(r#"root ::= [ab]{4}"#).unwrap();
        let mut state = GrammarState::new(g).unwrap();
        for c in "abab".chars() {
            state.accept_token(&c.to_string()).unwrap();
        }
        assert!(state.is_accepting());
    }

    #[test]
    fn test_quantifier_min() {
        let g = Grammar::new(r#"root ::= [ab]{4,}"#).unwrap();
        let mut state = GrammarState::new(g).unwrap();
        for c in "ababab".chars() {
            state.accept_token(&c.to_string()).unwrap();
        }
        assert!(state.is_accepting());
    }

    #[test]
    fn test_quantifier_range() {
        let g = Grammar::new(r#"root ::= [ab]{0,4}"#).unwrap();
        let state = GrammarState::new(g).unwrap();
        // 0 should be fine
        assert!(state.is_accepting());
    }

    #[test]
    fn test_reset() {
        let g = Grammar::new(r#"root ::= "a""#).unwrap();
        let mut state = GrammarState::new(g).unwrap();
        state.accept_token("a").unwrap();
        assert!(state.is_accepting());
        state.reset().unwrap();
        assert!(!state.is_accepting());
    }
}
