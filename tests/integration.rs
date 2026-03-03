use schoolmarm::{Grammar, GrammarState};

/// Match a full string against a grammar by feeding it character-by-character.
fn match_string(grammar_str: &str, input: &str) -> bool {
    let grammar = match Grammar::new(grammar_str) {
        Ok(g) => g,
        Err(_) => return false,
    };
    let mut state = match GrammarState::new(grammar) {
        Ok(s) => s,
        Err(_) => return false,
    };

    for ch in input.chars() {
        if state.accept_token(&ch.to_string()).is_err() {
            return false;
        }
        if !state.is_valid() {
            return false;
        }
    }

    state.is_accepting()
}

/// Test helper: check passing and failing strings for a grammar.
fn test_grammar(desc: &str, grammar_str: &str, passing: &[&str], failing: &[&str]) {
    for s in passing {
        assert!(
            match_string(grammar_str, s),
            "{}: expected '{}' to PASS but it FAILED",
            desc,
            s
        );
    }
    for s in failing {
        assert!(
            !match_string(grammar_str, s),
            "{}: expected '{}' to FAIL but it PASSED",
            desc,
            s
        );
    }
}

#[test]
fn test_simple_grammar() {
    test_grammar(
        "simple grammar",
        r#"
            root ::= expr
            expr ::= term ("+" term)*
            term ::= number
            number ::= [0-9]+"#,
        &["42", "1+2+3+4+5", "123+456"],
        &["+", "1+2+3+4+5+", "12a45"],
    );
}

#[test]
fn test_medium_complexity() {
    test_grammar(
        "medium complexity grammar",
        r#"
            root ::= expression
            expression ::= term ws (("+"|"-") ws term)*
            term ::= factor ws (("*"|"/") ws factor)*
            factor ::= number | variable | "(" expression ")" | function-call
            number ::= [0-9]+
            variable ::= [a-zA-Z_][a-zA-Z0-9_]*
            function-call ::= variable ws "(" (expression ("," ws expression)*)? ")"
            ws ::= [ \t\n\r]?"#,
        &[
            "42",
            "1*2*3*4*5",
            "x",
            "x+10",
            "x1+y2",
            "(a+b)*(c-d)",
            "func()",
            "func(x,y+2)",
            "a*(b+c)-d/e",
            "f(g(x),h(y,z))",
            "x + 10",
            "x1 + y2",
            "(a + b) * (c - d)",
            "func(x, y + 2)",
            "a * (b + c) - d / e",
            "f(g(x), h(y, z))",
            "123+456",
        ],
        &[
            "+",
            "x + + y",
            "a * / b",
            "func(,)",
            "func(x y)",
            "(a + b",
            "x + y)",
            "42 +",
        ],
    );
}

#[test]
fn test_special_chars_dot() {
    test_grammar(
        "special characters (dot)",
        r#"root ::= ... "abc" ..."#,
        &["abcabcabc", "aaaabcccc"],
        &["aaabcccc", "aaaaabcccc", "aaaabccc", "aaaabccccc"],
    );
}

#[test]
fn test_star_quantifier() {
    test_grammar(
        "* quantifier",
        r#"root ::= "a"*"#,
        &["", "a", "aaaaa", "aaaaaaaaaaaaaaaaaa"],
        &["b", "ab", "aab", "ba"],
    );
}

#[test]
fn test_plus_quantifier() {
    test_grammar(
        "+ quantifier",
        r#"root ::= "a"+"#,
        &["a", "aaaaa", "aaaaaaaaaaaaaaaaaa"],
        &["", "b", "ab", "aab", "ba"],
    );
}

#[test]
fn test_question_quantifier() {
    test_grammar(
        "? quantifier",
        r#"root ::= "a"?"#,
        &["", "a"],
        &["b", "ab", "aa", "ba"],
    );
}

#[test]
fn test_mixed_quantifiers() {
    test_grammar(
        "mixed quantifiers",
        r#"
            root ::= cons+ vowel* cons? (vowel cons)*
            vowel ::= [aeiouy]
            cons ::= [bcdfghjklmnpqrstvwxyz]"#,
        &["yes", "no", "noyes", "crwth", "four", "bryyyy"],
        &["yess", "yesno", "forty", "catyyy"],
    );
}

#[test]
fn test_exact_repetition() {
    test_grammar(
        "exact repetition",
        r#"root ::= [ab]{4}"#,
        &["aaaa", "bbbb", "abab"],
        &["a", "b", "aaaaa"],
    );
}

#[test]
fn test_min_repetition() {
    test_grammar(
        "min repetition",
        r#"root ::= [ab]{4,}"#,
        &["aaaa", "aaaaab", "bbbb", "ababab"],
        &["", "aba"],
    );
}

#[test]
fn test_max_repetition() {
    test_grammar(
        "max repetition",
        r#"root ::= [ab]{0,4}"#,
        &["", "a", "aa", "aaa", "aaab"],
        &["aaaaa"],
    );
}

#[test]
fn test_range_repetition() {
    test_grammar(
        "range repetition",
        r#"root ::= ("0x" [A-F0-9]{2} " "?){3,5}"#,
        &["0xFF 0x12 0xAB", "0xFF 0x12 0xAB 0x00 0x00"],
        &["", "0xFF", "0xFF 0x12", "0xFF 0x12 0xAB 0x00 0x00 0x00"],
    );
}

#[test]
fn test_failure_missing_root() {
    let grammar_str = r#"
        rot ::= expr
        expr ::= term ("+" term)*
        term ::= number
        number ::= [0-9]+"#;
    let result = Grammar::new(grammar_str);
    assert!(result.is_err(), "Should fail with missing root");
}

#[test]
fn test_failure_missing_reference() {
    let grammar_str = r#"root ::= expr
        expr ::= term ("+" term)*
        term ::= numero
        number ::= [0-9]+"#;
    let result = Grammar::new(grammar_str);
    assert!(result.is_err(), "Should fail with undefined rule 'numero'");
}

#[test]
fn test_failure_left_recursion_simple() {
    let result = Grammar::new(r#"root ::= "a" | root "a""#);
    assert!(result.is_err());
}

#[test]
fn test_failure_left_recursion_medium() {
    let result = Grammar::new(
        r#"root ::= asdf
asdf ::= "a" | asdf "a""#,
    );
    assert!(result.is_err());
}

#[test]
fn test_failure_left_recursion_hard() {
    let result = Grammar::new(
        r#"root ::= asdf
asdf ::= "a" | foo "b"
foo ::= "c" | asdf "d" | "e""#,
    );
    assert!(result.is_err());
}

#[test]
fn test_failure_left_recursion_hardest() {
    let result = Grammar::new(
        r#"root ::= asdf
asdf ::= "a" | foo "b"
foo ::= "c" | empty asdf "d" | "e"
empty ::= "blah" | "#,
    );
    assert!(result.is_err());
}

#[test]
fn test_json_grammar_parses() {
    let json_grammar = include_str!("fixtures/json.gbnf");
    let grammar = Grammar::new(json_grammar).unwrap();
    let state = GrammarState::new(grammar).unwrap();
    assert!(state.is_valid());
}

#[test]
fn test_json_grammar_simple_object() {
    let json_grammar = include_str!("fixtures/json.gbnf");
    let grammar = Grammar::new(json_grammar).unwrap();
    let mut state = GrammarState::new(grammar).unwrap();

    // Feed "{}" character by character
    for ch in "{}".chars() {
        state.accept_token(&ch.to_string()).unwrap();
    }
    assert!(state.is_accepting());
}

#[test]
fn test_json_grammar_string_value() {
    let json_grammar = include_str!("fixtures/json.gbnf");
    let grammar = Grammar::new(json_grammar).unwrap();
    let mut state = GrammarState::new(grammar).unwrap();

    let input = r#"{"foo": "bar"}"#;
    for ch in input.chars() {
        state
            .accept_token(&ch.to_string())
            .unwrap_or_else(|e| panic!("Failed at char '{}': {}", ch, e));
    }
    assert!(state.is_accepting());
}

#[test]
fn test_json_grammar_nested() {
    let json_grammar = include_str!("fixtures/json.gbnf");
    let grammar = Grammar::new(json_grammar).unwrap();
    let mut state = GrammarState::new(grammar).unwrap();

    let input = r#"{"a": {"b": "c"}}"#;
    for ch in input.chars() {
        state
            .accept_token(&ch.to_string())
            .unwrap_or_else(|e| panic!("Failed at char '{}': {}", ch, e));
    }
    assert!(state.is_accepting());
}

#[test]
fn test_json_grammar_array() {
    let json_grammar = include_str!("fixtures/json.gbnf");
    let grammar = Grammar::new(json_grammar).unwrap();
    let mut state = GrammarState::new(grammar).unwrap();

    let input = r#"{"items": [1, 2, 3]}"#;
    for ch in input.chars() {
        state
            .accept_token(&ch.to_string())
            .unwrap_or_else(|e| panic!("Failed at char '{}': {}", ch, e));
    }
    assert!(state.is_accepting());
}

#[test]
fn test_json_grammar_rejects_invalid() {
    let json_grammar = include_str!("fixtures/json.gbnf");

    // Not valid JSON object
    assert!(!match_string(json_grammar, ""));
    assert!(!match_string(json_grammar, "[]")); // root requires object
    assert!(!match_string(json_grammar, "null"));
    assert!(!match_string(json_grammar, r#""hello""#));
    assert!(!match_string(json_grammar, "true"));
}

#[test]
fn test_allowed_tokens_multichar() {
    let grammar = Grammar::new(r#"root ::= "hello" " " "world""#).unwrap();
    let state = GrammarState::new(grammar).unwrap();
    let vocab = vec!["h", "he", "hel", "hell", "hello", "world", "w", " "];
    let allowed = state.allowed_tokens(&vocab);
    assert!(allowed[0]); // "h"
    assert!(allowed[1]); // "he"
    assert!(allowed[2]); // "hel"
    assert!(allowed[3]); // "hell"
    assert!(allowed[4]); // "hello"
    assert!(!allowed[5]); // "world" — not yet
    assert!(!allowed[6]); // "w" — not yet
    assert!(!allowed[7]); // " " — not yet
}

#[test]
fn test_allowed_tokens_after_partial() {
    let grammar = Grammar::new(r#"root ::= "hello" " " "world""#).unwrap();
    let mut state = GrammarState::new(grammar).unwrap();
    state.accept_token("hello").unwrap();
    let vocab = vec!["h", "he", "hello", "world", "w", " ", " world", " w"];
    let allowed = state.allowed_tokens(&vocab);
    assert!(!allowed[0]); // "h"
    assert!(!allowed[1]); // "he"
    assert!(!allowed[2]); // "hello"
    assert!(!allowed[3]); // "world" — need space first
    assert!(!allowed[4]); // "w"
    assert!(allowed[5]); // " "
    assert!(allowed[6]); // " world" — full remaining match
    assert!(allowed[7]); // " w" — partial match
}

#[test]
fn test_unicode_dot() {
    // The . in GBNF matches any single Unicode codepoint
    test_grammar(
        "unicode dot",
        r#"root ::= ... "abc" ..."#,
        // Multi-byte chars each count as 1 codepoint for .
        &["🔵🟠✅abc❌🟠🔵"],
        &["🔵🟠✅❌abc❌✅🟠🔵", "🔵🟠abc🟠🔵"],
    );
}
