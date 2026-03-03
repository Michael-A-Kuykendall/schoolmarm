# SchoolMarm Roadmap

SchoolMarm is currently a production-ready, zero-dependency port of the `llama.cpp` GBNF engine. It is fast, safe, and robust.

To make it the absolute "perfect" tool in the grammar-constrained decoding lane, here is the path forward.

## 🚀 Phase 1: Optimization & Efficiency (The "Fastest" Lane)

- [ ] **Bitmask Optimization**: Switch `allowed_tokens` from returning `Vec<bool>` to a compressed bitset (e.g., `BitVec` or `u64` chunks).
  - *Impact*: Reduces memory bandwidth by 8x during the hot masking loop. Critical for large vocabularies (128k+ tokens).
- [ ] **State Caching**: Implement an optional LRU cache for `(GrammarState, Token) -> NextGrammarState` transitions to bypass the NFA stack operations for frequently repeated patterns (like whitespace or JSON punctuation).
- [ ] **Trie-based Masking**: If the vocabulary is provided as a Trie, we can optimize rejection by pruning entire branches of the vocabulary early, rather than iterating every single token.

## 🛠 Phase 2: Developer Experience (The "Easiest" Lane)

- [ ] **JSON Schema to GBNF Converter**:
  - Currently, users must write GBNF manually.
  - *Goal*: `Grammar::from_json_schema(serde_json::Value)` to automatically generate robust GBNF from standard schemas.
- [ ] **Standard Grammar Library**:
  - Ship with pre-optimized grammars for common formats:
    - `grammars::json()`
    - `grammars::c_code()`
    - `grammars::markdown_table()`
- [ ] **Debug Visualizer**: A utility to print the current stack state as a human-readable tree. "Why was this token rejected?" diagnostics.

## 🛡 Phase 3: Robustness & Ecosystem

- [ ] **Fuzzing Infrastructure**: Add `cargo fuzz` targets to hammer the parser with malformed GBNF and the state machine with weird token sequences.
- [ ] **Streaming Unicode Handling**: rigorous tests for tokenizers that might split multi-byte characters across tokens (edge case handling).
- [ ] **C / Python Bindings**: seamless integration for non-Rust inference engines.

## ✅ Completed

- [x] Full GBNF Spec compliance (matching `llama.cpp`)
- [x] Zero-dependency parsing and state management
- [x] Recursion depth limits (security)
- [x] Left-recursion detection
