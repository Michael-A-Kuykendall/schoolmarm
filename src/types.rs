/// Type tag for a grammar element, matching llama.cpp's llama_gretype.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ElementType {
    /// End of rule definition
    End = 0,
    /// Start of alternate definition for rule
    Alt = 1,
    /// Non-terminal: reference to another rule (value = rule index)
    RuleRef = 2,
    /// Terminal: character/codepoint (value = unicode codepoint)
    Char = 3,
    /// Inverse char(s): [^a], [^a-b], [^abc]
    CharNot = 4,
    /// Modifies preceding Char/CharAlt to be inclusive range upper bound
    CharRngUpper = 5,
    /// Adds alternate char to match: [ab], [a-zA]
    CharAlt = 6,
    /// Any character (.)
    CharAny = 7,
}

/// A single element in a grammar rule's production.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Element {
    pub etype: ElementType,
    pub value: u32,
}

impl Element {
    pub fn new(etype: ElementType, value: u32) -> Self {
        Self { etype, value }
    }

    pub fn end() -> Self {
        Self::new(ElementType::End, 0)
    }

    pub fn alt() -> Self {
        Self::new(ElementType::Alt, 0)
    }

    pub fn rule_ref(rule_id: u32) -> Self {
        Self::new(ElementType::RuleRef, rule_id)
    }

    pub fn char_(cp: u32) -> Self {
        Self::new(ElementType::Char, cp)
    }

    pub fn char_not(cp: u32) -> Self {
        Self::new(ElementType::CharNot, cp)
    }

    pub fn char_rng_upper(cp: u32) -> Self {
        Self::new(ElementType::CharRngUpper, cp)
    }

    pub fn char_alt(cp: u32) -> Self {
        Self::new(ElementType::CharAlt, cp)
    }

    pub fn char_any() -> Self {
        Self::new(ElementType::CharAny, 0)
    }

    /// Is this a character-class element?
    pub fn is_char_element(&self) -> bool {
        matches!(
            self.etype,
            ElementType::Char
                | ElementType::CharNot
                | ElementType::CharAlt
                | ElementType::CharRngUpper
                | ElementType::CharAny
        )
    }

    /// Is this an end-of-sequence marker (End or Alt)?
    pub fn is_end_of_sequence(&self) -> bool {
        matches!(self.etype, ElementType::End | ElementType::Alt)
    }
}

/// A rule is a sequence of elements, terminated by End, with Alt separating alternatives.
pub type Rule = Vec<Element>;

/// All rules in a grammar.
pub type Rules = Vec<Rule>;

/// A position within the grammar: (rule_index, element_index).
pub type Pos = (usize, usize);

/// A parse stack: a list of positions we need to match, back = top.
pub type Stack = Vec<Pos>;

/// Multiple possible parse stacks (nondeterministic).
pub type Stacks = Vec<Stack>;
