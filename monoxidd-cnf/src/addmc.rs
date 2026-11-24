// Auxiliar functions related to the addmc algorithm.

use monoxidd::{Circuit, Var, Literal};

use crate::cnf::*;

use oxidd::PseudoBooleanFunction;
use oxidd::VarNo;
use oxidd::util::AllocResult;
use oxidd::Function;

use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::SeedableRng;
use std::collections::HashMap;


pub type VarOrder = HashMap<Var,i64>;

pub type ClauseOrder = HashMap<usize, i64>; // the key is the index in the clause vector of the circuit

pub enum Heuristic {
    Random,
    Mcs,
    LexP,
    LexM

}

pub fn literal_with_order<'id>(
    manager: &<Node as Function>::Manager<'id>,
    one: &Node,
    lit: Literal,
    pi: &VarOrder,
) -> AllocResult<Node> {
    let x = lit.get_input().expect("clause literal must be an input");
    let var = Node::var(manager, pi[&x] as VarNo)?;
    if lit.is_negative() {
        one.sub(&var)
    } else {
        Ok(var)
    }
}

/// A clause is an OR of its literals. For 0/1-valued functions, OR is `max`.
pub fn clause_with_order<'id>(
    manager: &<Node as Function>::Manager<'id>,
    one: &Node,
    lits: &[Literal],
    pi: &VarOrder
) -> AllocResult<Node> {
    let mut res = one.sub(one)?; // empty clause => zero
    for &lit in lits {
        res = PseudoBooleanFunction::max(&res, &literal_with_order(manager, one, lit, pi)?)?;
    }
    Ok(res)
}


/// A random permutation of 0..num_vars, mapping each variable to its position.
pub fn random_var_order(num_vars: usize, seed: u64) -> VarOrder {
    let mut vars: Vec<Var> = (0..num_vars).collect();
    vars.shuffle(&mut StdRng::seed_from_u64(seed));
    vars.into_iter()
        .enumerate()
        .map(|(pos, v)| (v, pos as i64))
        .collect()
}

pub fn get_diagram_var_order(circuit: &Circuit, seed: u64) -> VarOrder {
    random_var_order(circuit.num_inputs(), seed)
}

pub fn get_cluster_var_order(circuit: &Circuit, seed: u64) -> VarOrder {
    // A different seed, so this order is independent of the diagram order.
    random_var_order(circuit.num_inputs(), seed.wrapping_add(1))
}

/// For now every clause gets rank 1, so there is a single cluster.
pub fn get_clause_ranks(circuit: &Circuit, _rho: &VarOrder) -> ClauseOrder {
    (0..circuit.num_clauses()).map(|i| (i, 1)).collect()
}

/// All clause indices with the given rank, sorted so the result is deterministic.
pub fn get_all_keys_with(ranks: &ClauseOrder, rank: i64) -> Vec<usize> {
    let mut keys: Vec<usize> = ranks
        .iter()
        .filter(|&(_, &r)| r == rank)
        .map(|(&clause, _)| clause)
        .collect();
    keys.sort_unstable();
    keys
}

/// Create the ADDs from a number of clauses.
/// gamm is the slice of indeces of the clauses (that belong to circuit) to make.
pub fn get_adds<'id>(
    manager: &<Node as Function>::Manager<'id>,
    one: &Node,
    gamm: &[usize],
    circuit: &Circuit,
    pi: &VarOrder
) -> AllocResult<Vec<Node>> {
    gamm.iter()
        .map(|&cl| {
            let lits = circuit.get_clause(cl).expect("clause index out of range");
            clause_with_order(manager, one, lits, pi)
        })
        .collect()
}

/// Multiply a vector of ADDs.
pub fn mult_adds<'id>(
    one: &Node,
    adds: Vec<Node>,
) -> AllocResult<Node> {
    let mut it = adds.into_iter();
    let Some(first) = it.next() else {
        return Ok(one.clone()); // empty product is 1
    };
    it.try_fold(first, |acc, a| acc.mul(&a))
}

pub fn mult_and_project<'id>(
    manager: &<Node as Function>::Manager<'id>,
    one: &Node,
    x: Var,
    pi: &VarOrder,
    circuit: &Circuit,
    add: &Node
) -> AllocResult<Node> {
    let w = circuit.get_weight_of(x).expect("weights not found");
    let v = Node::var(manager, pi[&x] as VarNo)?;
    let (neg_w, pos_w) = w.get_weights();
    let w_add = weighted_literal(manager, neg_w.unwrap_or(1.0), pos_w.unwrap_or(1.0), &v, one)?;
    add.mul(&w_add)?.project(&v)
}

pub fn choose_cluster(ai:&Node, i:usize)-> i64 { 1 }
