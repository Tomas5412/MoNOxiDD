// Auxiliar functions related to the building of cnfs into MTBDD.

use monoxidd::{Circuit, Literal};

use oxidd::PseudoBooleanFunction;
use oxidd::VarNo;
use oxidd::mtbdd::terminal::F64;
use oxidd::mtbdd::{MTBDDFunction};
use oxidd::util::AllocResult;
use oxidd::Function;

pub type Node = MTBDDFunction<F64>;

pub fn weighted_literal<'id>(
		manager: &<Node as Function>::Manager<'id>,
		neg_w: f64,
        pos_w: f64, 
		var:&Node, 
		one:&Node
	) -> AllocResult<Node> {
	let c_true = MTBDDFunction::constant(manager, F64::from(pos_w))?;
    let c_false = MTBDDFunction::constant(manager, F64::from(neg_w))?;
    var.ite(&c_true,&c_false)
}

/// The 0/1 indicator of a literal: `x` for a positive literal, `1 - x` for a negative one.
pub fn literal<'id>(
    manager: &<Node as Function>::Manager<'id>,
    one: &Node,
    lit: Literal,
) -> AllocResult<Node> {
    let var = Node::var(manager, lit.get_input().expect("clause literal must be an input") as VarNo)?;
    if lit.is_negative() {
        one.sub(&var)
    } else {
        Ok(var)
    }
}

/// A clause is an OR of its literals. For 0/1-valued functions, OR is `max`.
pub fn clause<'id>(
    manager: &<Node as Function>::Manager<'id>,
    one: &Node,
    lits: &[Literal],
) -> AllocResult<Node> {
    let mut res = one.sub(one)?; // empty clause => zero
    for &lit in lits {
        res = PseudoBooleanFunction::max(&res, &literal(manager, one, lit)?)?;
    }
    Ok(res)
}

/// One weighted-literal function per entry of the weight map.
pub fn translate_weights<'id>(
    manager: &<Node as Function>::Manager<'id>,
    circuit: &Circuit,
    one: &Node,
) -> AllocResult<Vec<Node>> {
    circuit
        .iter_weights()
        .map(|(&var, weight)| {
            let v = Node::var(manager, var as VarNo)?;
            let (neg_w, pos_w) = weight.get_weights();
            weighted_literal(manager, neg_w.unwrap_or(1.0), pos_w.unwrap_or(1.0), &v, one)
        })
        .collect()
}

pub fn total_weight<'id>(manager: &<Node as Function>::Manager<'id>, circuit: &Circuit, one: &Node) -> AllocResult<Node> {
    let mut res = one.clone();
    for (&var, weight) in circuit.iter_weights() {
        let (neg_w, pos_w) = weight.get_weights();
        let v = Node::var(manager, var as VarNo)?;
        let literal = weighted_literal(manager, neg_w.unwrap_or(1.0), pos_w.unwrap_or(1.0), &v, one)?;
        res = res.mul(&literal)?;
    }
    Ok(res)
}

pub fn formula<'id>(
    manager: &<Node as Function>::Manager<'id>,
    circuit: &Circuit,
    one: &Node,
) -> AllocResult<Node> {
    let mut acc = one.clone();
    for lits in circuit.iter_clauses() {
        let c = clause(manager, one, lits)?;
        acc = PseudoBooleanFunction::mul(&acc, &c)?;
    }
    Ok(acc)
}