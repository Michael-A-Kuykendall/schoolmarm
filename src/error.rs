use std::fmt;

/// All possible errors from grammar parsing and evaluation.
#[derive(Debug, Clone)]
pub enum GrammarError {
    /// Syntax error during GBNF parsing
    ParseError(String),
    /// A rule was referenced but never defined
    UndefinedRule(String),
    /// Left recursion detected in grammar
    LeftRecursion(usize),
    /// Grammar has no root rule
    NoRootRule,
    /// Grammar is empty (no rules)
    EmptyGrammar,
    /// Invalid state during grammar evaluation
    InvalidState(String),
    /// Start rule index out of bounds
    InvalidStartRule(usize),
}

impl fmt::Display for GrammarError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GrammarError::ParseError(msg) => write!(f, "parse error: {}", msg),
            GrammarError::UndefinedRule(name) => write!(f, "undefined rule: '{}'", name),
            GrammarError::LeftRecursion(idx) => {
                write!(f, "left recursion detected at rule index {}", idx)
            }
            GrammarError::NoRootRule => write!(f, "grammar does not contain a 'root' rule"),
            GrammarError::EmptyGrammar => write!(f, "grammar is empty"),
            GrammarError::InvalidState(msg) => write!(f, "invalid state: {}", msg),
            GrammarError::InvalidStartRule(idx) => {
                write!(f, "start rule index {} out of bounds", idx)
            }
        }
    }
}

impl std::error::Error for GrammarError {}
