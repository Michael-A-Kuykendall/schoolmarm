//! # schoolmarm
//!
//! GBNF grammar-constrained decoding for LLM inference, ported from llama.cpp.
//!
//! This crate provides grammar-constrained sampling for autoregressive language model
//! inference. Given a GBNF grammar string and a vocabulary of token strings, it produces
//! bitmasks of allowed tokens at each generation step.
//!
//! ## Usage
//!
//! ```rust
//! use schoolmarm::{Grammar, GrammarState};
//!
//! // Parse a grammar
//! let grammar = Grammar::new(r#"root ::= "hello" | "world""#).unwrap();
//!
//! // Create runtime state
//! let mut state = GrammarState::new(grammar).unwrap();
//!
//! // Get allowed tokens for your vocabulary
//! let vocab = vec!["hello", "world", "foo", "hel"];
//! let allowed = state.allowed_tokens(&vocab);
//! // allowed = [true, true, false, true]
//!
//! // Accept a token and advance state
//! state.accept_token("hello").unwrap();
//! assert!(state.is_accepting());
//! ```
//!
//! ## GBNF Format
//!
//! GBNF (GGML BNF) is an extended BNF notation for defining formal grammars.
//! See the [llama.cpp GBNF documentation](https://github.com/ggml-org/llama.cpp/blob/master/grammars/README.md)
//! for the full specification.
//!
//! Supported features:
//! - Literals: `"hello"`
//! - Character ranges: `[a-z]`, `[^0-9]`, `[abc]`
//! - Any character: `.`
//! - Rule references: `rulename`
//! - Alternation: `|`
//! - Grouping: `( ... )`
//! - Repetition: `*`, `+`, `?`, `{n}`, `{n,m}`, `{n,}`
//! - Comments: `# ...`
//! - Unicode escapes: `\xNN`, `\uNNNN`, `\UNNNNNNNN`

pub mod error;
pub mod parse;
pub mod state;
pub mod types;

pub use error::GrammarError;
pub use state::{Grammar, GrammarState};
pub use types::{Element, ElementType};
