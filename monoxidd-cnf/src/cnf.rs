use monoxidd::{Circuit, Literal, Weight};

use oxidd::ManagerRef;
use oxidd::PseudoBooleanFunction;
use oxidd::VarNo;
use oxidd::mtbdd::terminal::F64;
use oxidd::mtbdd::{MTBDDFunction, MTBDDManagerRef};
use oxidd::util::AllocResult;
use oxidd_core::Manager;
use oxidd::Function;

use std::time::Instant;

pub type Node = MTBDDFunction<F64>;

fn weighted_literal<'id>(
		manager: &<Node as Function>::Manager<'id>,
		neg_w: f64,
        pos_w: f64, 
		var:&Node, 
		one:&Node
	) -> AllocResult<Node> {
	let c_true = MTBDDFunction::constant(manager, F64::from(pos_w))?;
    let c_false = MTBDDFunction::constant(manager, F64::from(neg_w))?;
    let true_branch = var.mul(&c_true)?;
    let false_branch = one.sub(var)?.mul(&c_false)?;
    true_branch.add(&false_branch)
}

/// The 0/1 indicator of a literal: `x` for a positive literal, `1 - x` for a negative one.
fn literal<'id>(
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
fn clause<'id>(
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
fn translate_weights<'id>(
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

fn total_weight<'id>(manager: &<Node as Function>::Manager<'id>, circuit: &Circuit, one: &Node) -> AllocResult<Node> {
    let mut res = one.clone();
    for (&var, weight) in circuit.iter_weights() {
        let (neg_w, pos_w) = weight.get_weights();
        let v = Node::var(manager, var as VarNo)?;
        let literal = weighted_literal(manager, neg_w.unwrap_or(1.0), pos_w.unwrap_or(1.0), &v, one)?;
        res = res.mul(&literal)?;
    }
    Ok(res)
}

fn formula<'id>(
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


/// Monolithic weighted model counting
/// This uses the brute-force oxidd function
pub fn mono_wmc<'id>(
	manager: &<Node as Function>::Manager<'id>,
    circuit: &Circuit,
    one: &Node
) -> AllocResult<F64> {

    let mut now = Instant::now();
	let f = formula(manager, &circuit, one)?;
    println!("time elapsed calculating the clauses: {:.2?}", now.elapsed());
    println!("gc count after calculating the clauses: {}", manager.gc());
    println!("inner node count after calculating the clauses: {}", manager.num_inner_nodes());
	now = Instant::now();
	let weight = total_weight(manager, &circuit, one)?;
    println!("time elapsed calculating the weights: {:.2?}", now.elapsed());
    println!("gc count after calculating the weights: {}", manager.gc());
    println!("inner node count after calculating the clauses: {}", manager.num_inner_nodes());
    now = Instant::now();
    let res = f.wmc(&weight);
    println!("time elapsed calculating the wmc: {:.2?}", now.elapsed());
    println!("gc count after calculating the wmc: {}", manager.gc());
    println!("inner node count after calculating the clauses: {}", manager.num_inner_nodes());
	Ok(res)
}

fn setup_manager(num_vars: usize) -> MTBDDManagerRef<F64> {
    let manager_ref: MTBDDManagerRef<F64> = oxidd::mtbdd::new_manager(73741824, 8000000, 73741824, 16);
    manager_ref.with_manager_exclusive(|manager| {
        manager.add_vars(u32::try_from(num_vars).expect("too many variables"));
    });
    manager_ref
}


#[derive(Debug)]
#[warn(unused)]
enum WmcType {
    MonoWmc,
}

pub fn calculate_wmc(circuit: &Circuit) -> AllocResult<F64>{
	let manager_ref = setup_manager(circuit.num_inputs());

	manager_ref.with_manager_shared(|manager| -> AllocResult<F64> {
    	let one = Node::constant(manager, F64::from(1.0))?;
    	let res = mono_wmc(manager, circuit, &one);
        res
	})
}