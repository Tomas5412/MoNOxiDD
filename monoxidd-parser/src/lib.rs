use std::collections::HashMap;
use std::fmt::{self, Write};

mod util;
pub mod parser;

#[cfg(feature = "load-file")]
mod load_file;
#[cfg(feature = "load-file")]
pub use load_file::*;

/// Variable type
///
/// The two most significant bits are never set.
pub type Var = usize;

/// A possibly negated variable
///
/// There are three kinds of variables: constants ([`Literal::FALSE`] and
/// [`Literal::TRUE`]), circuit inputs, and gates.
///
/// Concerning ordering, the following properties hold: Given two [`Literal`]s
/// `a, b`, `a <= b` is equivalent to the lexicographic comparison
/// `(a.positive(), a.is_negative()) <= (b.positive(), b.is_negative())`.
/// Furthermore, `Literal::FALSE <= a` holds (and thus `Literal::TRUE <= a` if
/// `a != Literal::FALSE`).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Literal(usize);

impl Literal {
    // We rely on POLARITY_BIT being the least significant bit for the ordering
    const POLARITY_BIT: u32 = 0;
    const GATE_BIT: u32 = 1;
    const VAR_LSB: u32 = 2;

    /// Maximum input number representable as literal
    pub const MAX_INPUT: usize = (usize::MAX >> Self::VAR_LSB) - 2;
    /// Maximum gate number representable as literal
    pub const MAX_GATE: usize = usize::MAX >> Self::VAR_LSB;

    /// Literal representing the constant `⊥`
    ///
    /// This is literal is considered to be positive (i.e., not negated):
    ///
    /// ```
    /// # use monoxidd_parser::Literal;
    /// assert!(Literal::FALSE.is_positive());
    /// assert!(Literal::FALSE < Literal::TRUE);
    /// assert_eq!(Literal::FALSE.to_string(), "⊥");
    /// ```
    pub const FALSE: Self = Self(0);
    /// Literal representing the constant `⊤`
    ///
    /// This is literal is considered to be negated:
    ///
    /// ```
    /// # use monoxidd_parser::Literal;
    /// assert!(Literal::TRUE.is_negative());
    /// assert!(Literal::TRUE < Literal::from_input(false, 0));
    /// assert!(Literal::TRUE < Literal::from_gate(false, 0));
    /// assert_eq!(Literal::TRUE.to_string(), "⊤");
    /// ```
    pub const TRUE: Self = Self(1 << Self::POLARITY_BIT);

    /// Undefined literal
    ///
    /// This is considered to be a positive (i.e., not negated) input, but its
    /// variable number is larger than `Self::MAX_INPUT`. Attempting to create
    /// this literal using [`Self::from_input()`] will trigger a debug
    /// assertion.
    ///
    /// ```
    /// # use monoxidd_parser::Literal;
    /// assert!(Literal::UNDEF.is_positive());
    /// assert!(Literal::UNDEF.is_input());
    /// assert_eq!(Literal::UNDEF.get_input(), Some(Literal::MAX_INPUT + 1));
    /// ```
    pub const UNDEF: Self = Self((Self::MAX_INPUT + 2) << Self::VAR_LSB);

    /// Create a new literal from an input variable
    #[track_caller]
    #[inline]
    pub const fn from_input(negative: bool, input: Var) -> Self {
        debug_assert!(input <= Self::MAX_INPUT, "input too large");
        Self(((input + 1) << Self::VAR_LSB) | ((negative as usize) << Self::POLARITY_BIT))
    }

    /// Create a new literal from an input variable (value range `1..`) or false
    /// (`0`)
    #[track_caller]
    #[inline]
    const fn from_input_or_false(negative: bool, input: Var) -> Self {
        debug_assert!(input <= Self::MAX_INPUT + 1, "input too large");
        Self((input << Self::VAR_LSB) | ((negative as usize) << Self::POLARITY_BIT))
    }

    /// Create a new literal from a gate number
    #[track_caller]
    #[inline]
    pub const fn from_gate(negative: bool, gate: Var) -> Self {
        debug_assert!(gate <= Self::MAX_GATE, "gate number too large");
        Self(
            (gate << Self::VAR_LSB)
                | (1 << Self::GATE_BIT)
                | ((negative as usize) << Self::POLARITY_BIT),
        )
    }

    /// Is the literal positive?
    ///
    /// Same as [`!self.is_negative()`][Self::is_negative]
    ///
    /// ```
    /// # use monoxidd_parser::Literal;
    /// assert!(Literal::from_input(false, 42).is_positive());
    /// assert!(!Literal::from_input(true, 1337).is_positive());
    /// ```
    #[inline(always)]
    pub const fn is_positive(self) -> bool {
        self.0 & (1 << Self::POLARITY_BIT) == 0
    }
    /// Is the literal negative?
    ///
    /// Same as [`!self.is_positive()`][Self::is_positive]
    ///
    /// ```
    /// # use monoxidd_parser::Literal;
    /// assert!(Literal::from_gate(true, 42).is_negative());
    /// assert!(!Literal::from_gate(false, 1337).is_negative());
    /// ```
    #[inline(always)]
    pub const fn is_negative(self) -> bool {
        !self.is_positive()
    }

    /// Get the positive variant of this literal
    ///
    /// ```
    /// # use monoxidd_parser::Literal;
    /// assert!(Literal::from_input(true, 42).positive().is_positive());
    /// ```
    #[inline(always)]
    pub const fn positive(self) -> Self {
        Self(self.0 & !(1 << Self::POLARITY_BIT))
    }

    /// Get the negative variant of this literal
    ///
    /// ```
    /// # use monoxidd_parser::Literal;
    /// assert_eq!(Literal::from_input(true, 42), Literal::from_input(false, 42).negative());
    /// assert!(Literal::from_input(false, 42).negative().is_negative());
    /// ```
    #[inline(always)]
    pub const fn negative(self) -> Self {
        Self(self.0 | (1 << Self::POLARITY_BIT))
    }

    /// Does this literal refer to an input?
    ///
    /// ```
    /// # use monoxidd_parser::Literal;
    /// assert!(Literal::from_input(false, 42).is_input());
    /// assert!(!Literal::FALSE.is_input());
    /// assert!(!Literal::TRUE.is_input());
    /// assert!(!Literal::from_gate(false, 1337).is_input());
    /// ```
    #[inline(always)]
    pub const fn is_input(self) -> bool {
        self.get_input().is_some()
    }

    /// Check if this literal refers to a gate
    ///
    /// ```
    /// # use monoxidd_parser::Literal;
    /// assert!(Literal::from_gate(false, 42).is_gate());
    /// assert!(!Literal::FALSE.is_gate());
    /// assert!(!Literal::TRUE.is_gate());
    /// assert!(!Literal::from_input(false, 1337).is_gate());
    /// ```
    #[inline(always)]
    pub const fn is_gate(self) -> bool {
        self.0 & (1 << Self::GATE_BIT) != 0
    }

    /// Get the input number (if this literal refers to a gate)
    ///
    /// ```
    /// # use monoxidd_parser::Literal;
    /// assert_eq!(Literal::from_input(false, 42).get_input(), Some(42));
    /// assert_eq!(Literal::FALSE.get_input(), None);
    /// assert_eq!(Literal::TRUE.get_input(), None);
    /// assert_eq!(Literal::from_gate(false, 42).get_input(), None);
    /// ```
    #[inline]
    pub const fn get_input(self) -> Option<Var> {
        if self.is_gate() {
            return None;
        }
        (self.0 >> Self::VAR_LSB).checked_sub(1)
    }

}

impl fmt::Display for Literal {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let i = self.0 >> Literal::VAR_LSB;
        let (kind, i) = if self.0 & (1 << Self::GATE_BIT) != 0 {
            ('g', i)
        } else {
            if i == 0 {
                return f.write_char(if self.is_positive() { '⊥' } else { '⊤' });
            }
            if i == Literal::MAX_INPUT + 2 {
                return f.write_str(if self.is_positive() { "+U" } else { "-U" });
            }
            ('i', i - 1)
        };

        let sign = if self.is_positive() { '+' } else { '-' };
        write!(f, "{sign}{kind}{i}")
    }
}

#[derive(PartialEq, Clone)]
pub struct Weight {
    pos_weight: Option<f64>,
    neg_weight: Option<f64>,
}

impl Weight {
    /// Assign both weights
    fn new() -> Self {
        Self {
            pos_weight: None,
            neg_weight: None,
        }
    }

    fn add_pos_weight(&mut self, w: f64) {
        if self.has_pos_weight() {
            unimplemented!();
        }
        self.pos_weight = Some(w);
    }

    fn add_neg_weight(&mut self, w: f64) {
        if self.has_neg_weight() {
            unimplemented!();
        }
        self.neg_weight = Some(w);
    }

    fn get_weights(&self) -> (Option<f64>,Option<f64>){
        (self.neg_weight, self.pos_weight)
    }

    fn has_pos_weight(&self) -> bool {
        !self.pos_weight.is_none()
    }

    fn has_neg_weight(&self) -> bool {
        !self.neg_weight.is_none()
    }

    fn is_completed(&self) -> bool {
        self.has_neg_weight() && self.has_pos_weight()
    }

}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct VarSet {
    /// Number of variables
    len: usize,

    /// Permutation of the variables.
    order: Vec<Var>,

}

impl VarSet {
    /// Create a variable set without a variable order and names
    #[inline(always)]
    pub const fn new(n: usize) -> Self {
        Self {
            len: n,
            order: Vec::new(),
        }
    }

    /// Number of variables
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns true iff the number of variables is 0
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Get the linear variable order, if present
    ///
    /// The order is given as a mapping from positions to variables.
    ///
    /// If [`self.order_tree()`][Self::order_tree] is not `None`, then it is the
    /// flattened tree.
    #[inline]
    pub fn order(&self) -> Option<&[Var]> {
        if self.len != self.order.len() {
            None
        } else {
            Some(&self.order)
        }
    }
}

type Clause = Vec<Literal>;

#[derive(Eq, PartialEq, Clone, Debug)]
struct Clauses2d{
    clauses: Vec<Clause>,
}


impl Clauses2d {
    fn new() -> Self {
        Self{
            clauses: Vec::new(),
        }
    }

    fn len(&self) -> usize{
        self.clauses.len()
    }

    fn new_clause(&mut self) {
        self.clauses.push(Vec::new());
    }

    fn last_clause(&self) -> Option<&Clause> {
        self.clauses.last()
    }

    fn last_clause_mut(&mut self) -> Option<&mut Clause> {
        self.clauses.last_mut()
    }

    fn push_element(&mut self, element:Literal) {
        if let Some(c) = self.last_clause_mut() {
            c.push(element);
        }
    }

    fn pop_clause(&mut self) -> bool {
        !self.clauses.pop().is_none()
    }

}

type WeightMap = HashMap<Var,Weight>;

/// Circuit of the cunjunctive normal form, where the clauses are in the form of a 2d vector.
#[derive(Clone, PartialEq)]
pub struct Circuit {
    inputs: VarSet,
    clauses: Clauses2d,
    weights: WeightMap,
}

impl Circuit {
    /// Create an empty circuit with the given circuit inputs
    #[inline(always)]
    pub fn new(inputs: VarSet) -> Self {

        let mut w_map = WeightMap::new();
        for i in 0..inputs.len {
            w_map.insert(i, Weight::new());
        }
        
        Self {
            inputs,
            clauses: Clauses2d::new(),
            weights: w_map,
        }
    }

    /// Get the circuit inputs
    #[inline(always)]
    pub fn inputs(&self) -> &VarSet {
        &self.inputs
    }
    /// Get a mutable reference to the circuit inputs
    #[inline(always)]
    pub fn inputs_mut(&mut self) -> &mut VarSet {
        &mut self.inputs
    }

    /// Get the number of gates in this circuit
    #[inline(always)]
    pub fn num_clauses(&self) -> usize {
        self.clauses.len()
    }

    #[inline(always)]
    pub fn num_weights(&self) -> usize {
        self.weights.len()
    }

    pub fn add_clause(&mut self) {
        self.clauses.new_clause();
    }

    pub fn add_literal(&mut self, input:Literal) {
        self.clauses.push_element(input);
    }

    pub fn add_weight(&mut self, lit:Literal, weight:f64){
        if let Some(val) = lit.get_input() {
             if lit.is_positive() {
                if let Some(val) = self.weights.get_mut(&val) { val.add_pos_weight(weight); };
             }
             else {
                if let Some(val) = self.weights.get_mut(&val) { val.add_neg_weight(weight); };
             }
        };
    }

    pub fn last_clause(&self) -> Option<&Clause> {
        self.clauses.last_clause()
    }

    pub fn pop_clause(&mut self) -> bool {
        self.clauses.pop_clause()
    }

    ///// Iterate over all gates in the circuit
    /////
    ///// The iteration order is from first to last in the gate sequence.
    //#[inline(always)]
    //pub fn iter_gates(&self) -> CircuitGateIter<'_> {
    //    CircuitGateIter(self.gates.iter())
    //}
}