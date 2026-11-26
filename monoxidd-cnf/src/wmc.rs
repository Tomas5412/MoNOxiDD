use monoxidd::{Circuit, Var};

use crate::addmc::*;
use crate::cnf::*;

use std::str::FromStr;

use oxidd::ManagerRef;
use oxidd::PseudoBooleanFunction;
use oxidd::VarNo;
use oxidd::mtbdd::terminal::F64;
use oxidd::mtbdd::{MTBDDManagerRef};
use oxidd::util::AllocResult;
use oxidd_core::Manager;
use oxidd::Function;


use std::time::Instant;
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::SeedableRng;
use std::collections::HashMap;
use std::collections::HashSet;


/// Monolithic weighted model counting
/// This uses the brute-force oxidd function
pub fn mono_wmc<'id>(
	manager: &<Node as Function>::Manager<'id>,
    circuit: &Circuit,
    one: &Node,
    debug: bool
) -> AllocResult<F64> {

    let mut now = Instant::now();
	
    let f = formula(manager, &circuit, one)?;

    if debug {
        println!("time elapsed calculating the clauses: {:.2?}", now.elapsed());
        println!("gc count after calculating the clauses: {}", manager.gc());
        println!("inner node count after calculating the clauses: {}", manager.num_inner_nodes());
    }
    else {
        manager.gc();
    }
    now = Instant::now();
	
    let weight = total_weight(manager, &circuit, one)?;
    
    if debug {
        println!("time elapsed calculating the weights: {:.2?}", now.elapsed());
        println!("gc count after calculating the weights: {}", manager.gc());
        println!("inner node count after calculating the weights: {}", manager.num_inner_nodes());
    }
    else {
        manager.gc();
    }

    
    now = Instant::now();
    
    let res = f.wmc(&weight);
    if debug {
        println!("time elapsed calculating the wmc: {:.2?}", now.elapsed());
        println!("gc count after calculating the wmc: {}", manager.gc());
        println!("inner node count after calculating the wmc (should be the same as after the weights?): {}", manager.num_inner_nodes());
    } else {
        manager.gc();
    }
	
    Ok(res)
}


pub fn addmc<'id>(
    manager: &<Node as Function>::Manager<'id>,
    circuit: &Circuit,
    one: &Node,
    debug: bool
) -> AllocResult<F64> {
    let seed = 5412;
    let pi = get_diagram_var_order(circuit, seed);
    let rho = get_cluster_var_order(circuit, seed);
    let ranks = get_clause_ranks(circuit, &rho);
    let m = ranks.values().copied().max().unwrap_or(0) as usize;
    let mut all_vars: HashSet<Var> = HashSet::new(); // set of all used variables
    let mut ks = Vec::new(); // vector of clusters of ADDs
    let mut xis = Vec::new(); // vector of used variables per cluster

    let mut now = Instant::now();

    for i in (1..=m).rev() {
        let gamm_i = get_all_keys_with(&ranks, i as i64);
        let k_i = get_adds(manager, one, &gamm_i, circuit, &pi)?;

        let vars_i: HashSet<Var> = gamm_i
            .iter()
            .flat_map(|&cl| circuit.get_vars_of_clause(cl).expect("clause index out of range"))
            .collect();

        let xi = &vars_i - &all_vars;
        all_vars.extend(&vars_i);
        ks.push(k_i);
        xis.push(xi);
    }
    
    if debug {
        println!("time elapsed forming all ADDS: {:.2?}", now.elapsed());
        println!("gc count forming all ADDS: {}", manager.gc());
        println!("inner node count forming all ADDS: {}", manager.num_inner_nodes());
    }
    else {
        manager.gc(); // Still do the gc even without the debug config.
    }

    
    let mut result = None;

    for i in 1..=m {

        let k_i = ks.pop().unwrap();
        let xi = xis.pop().unwrap();

        if k_i.is_empty() {
            continue;
        }

        now = Instant::now();
        
        let mut ai = mult_adds(one, k_i)?; // Bottleneck on one-cluster!
        println!("gc count after multiplying all of cluster {}: {}", i, manager.gc());
        for x in xi {
            ai = mult_and_project(manager, one, x, &pi, circuit, &ai)?;
            if debug { println!("gc count projecting variable {}: {}", x, manager.gc()); } else { manager.gc(); }
            
        }
    
        if i < m {
            let j = choose_cluster(&ai, i);
            ks[m - j as usize].push(ai);
        } else {
            result = Some(ai.eval(std::iter::empty::<(VarNo, bool)>()));
        }

        if debug {
            println!("time elapsed calculating cluster {}: {:.2?}", i, now.elapsed());
            println!("gc count calculating cluster {}: {}", i, manager.gc());
            println!("inner node count calculating cluster {}: {}", i, manager.num_inner_nodes());
        }
        else {
            manager.gc();
        }

    }
    Ok(result.expect("the last cluster should not be empty"))
}


fn setup_manager(num_vars: usize) -> MTBDDManagerRef<F64> {
    let manager_ref: MTBDDManagerRef<F64> = oxidd::mtbdd::new_manager(19000000, 20000000, 20000000, 16);
    manager_ref.with_manager_exclusive(|manager| {
        manager.add_vars(u32::try_from(num_vars).expect("too many variables"));
    });
    manager_ref
}


#[derive(Debug, PartialEq)] //TODO: Remove?
pub enum WmcType {
    MonoWmc,
    AddMc
}


pub fn calculate_wmc(
    circuit: &Circuit,
    debug: bool, 
    wmc_type: &str,
    diagram_var_order: &str,
    cluster_var_order: &str,
    cluster_rank: &str,
    choose_cluster: &str) -> AllocResult<F64> {

    let wmctype = wmc_type.trim().to_lowercase();

	let manager_ref = setup_manager(circuit.num_inputs());

    match wmctype.as_str() {
           "mono" => manager_ref.with_manager_shared(|manager| -> AllocResult<F64> {
                                let one = Node::constant(manager, F64::from(1.0))?;
                                let res = mono_wmc(manager, circuit, &one, debug);
                                res
                            }),
           "addmc" => manager_ref.with_manager_shared(|manager| -> AllocResult<F64> {
                                let one = Node::constant(manager, F64::from(1.0))?;
                                let res = addmc(manager, circuit, &one, debug);
                                res
                            }),
            &_ => unreachable!()
        }

}