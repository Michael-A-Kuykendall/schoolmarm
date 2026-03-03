# schoolmarm

GBNF grammar-constrained decoding for LLM inference, ported from [llama.cpp](https://github.com/ggml-org/llama.cpp).

**Zero dependencies. No unsafe. Pure Rust.**

## What It Does

Given a GBNF grammar string and a vocabulary of token strings, this crate produces
bitmasks of allowed tokens at each autoregressive generation step. This constrains
a language model to only generate output that matches the grammar — valid JSON, valid
code, structured data, or any other formally-defined format.

## Usage

```rust
use schoolmarm::{Grammar, GrammarState};

// Parse a grammar
let grammar = Grammar::new(r#"root ::= "hello" | "world""#).unwrap();

// Create runtime state
let mut state = GrammarState::new(grammar).unwrap();

// Get allowed tokens for your vocabulary
let vocab = vec!["hello", "world", "foo", "hel"];
let allowed = state.allowed_tokens(&vocab);
// allowed = [true, true, false, true]

// Accept a token and advance state
state.accept_token("hello").unwrap();
assert!(state.is_accepting());
```

## Integration With Inference Engines

The typical integration loop:

```rust
use gbnf::{Grammar, GrammarState};

fn generate(grammar_str: &str, vocab: &[&str]) -> Vec<usize> {
    let grammar = Grammar::new(grammar_str).unwrap();
    let mut state = GrammarState::new(grammar).unwrap();
    let mut tokens = Vec::new();

    loop {
        // 1. Get allowed token mask
        let allowed = state.allowed_tokens(vocab);

        // 2. Apply mask to logits (set disallowed to -inf)
        // ... your inference engine's logit masking here ...

        // 3. Sample from masked logits
        let token_id = 0; // placeholder: your sampling logic
        if !allowed[token_id] { break; }

        // 4. Accept the token and advance grammar state
        state.accept_token(vocab[token_id]).unwrap();
        tokens.push(token_id);

        // 5. Check if grammar is complete
        if state.is_accepting() {
            break;
        }
    }
    tokens
}
```

## GBNF Format

GBNF (GGML BNF) supports:

- **Literals**: `"hello"`
- **Character ranges**: `[a-z]`, `[^0-9]`, `[abc]`
- **Any character**: `.`
- **Rule references**: `rulename`
- **Alternation**: `|`
- **Grouping**: `( ... )`
- **Repetition**: `*`, `+`, `?`, `{n}`, `{n,m}`, `{n,}`
- **Unicode escapes**: `\xNN`, `\uNNNN`, `\UNNNNNNNN`
- **Comments**: `# ...`

Example JSON grammar:
```
root   ::= object
value  ::= object | array | string | number | ("true" | "false" | "null") ws
object ::= "{" ws (string ":" ws value ("," ws string ":" ws value)*)? "}" ws
array  ::= "[" ws (value ("," ws value)*)? "]" ws
string ::= "\"" ([^"\\\x7F\x00-\x1F] | "\\" (["\\bfnrt] | "u" [0-9a-fA-F]{4}))* "\"" ws
number ::= ("-"? ([0-9] | [1-9] [0-9]*)) ("." [0-9]+)? ([eE] [-+]? [0-9]+)? ws
ws     ::= | " " | "\n" [ \t]*
```

## Origin

Ported from llama.cpp's `src/llama-grammar.cpp` and `src/llama-grammar.h` (MIT License).
This is a clean-room Rust implementation following the same algorithms: recursive descent
GBNF parser, nondeterministic pushdown stack state machine, character-level token matching.

Fixes applied over the C++ original:
- Recursion depth limit in `advance_stack` (prevents stack overflow on pathological grammars)
- Bounds-checked rule index validation (prevents buffer overflows)
- All errors returned as `Result<T, GrammarError>` (no panics, no asserts)

## License

MIT
