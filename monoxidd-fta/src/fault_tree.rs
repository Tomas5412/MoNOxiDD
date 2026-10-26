use index_vec::IndexVec;
use indicatif::ParallelProgressIterator;
use itertools::Itertools;
use nodes::{Node, NodeId};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::sync::atomic::AtomicUsize;

use crate::fault_tree_normalizer::FaultTreeNormalizer;
use crate::formula::Formula;
use crate::nodes::{self, BasicEvent};

type ImpMeasures = (f64, f64, f64);

impl<T> From<FaultTreeNormalizer<T>> for FaultTree<T> {
    fn from(ft_norm: FaultTreeNormalizer<T>) -> Self {
        FaultTree::<T> {
            nodes: ft_norm.nodes,
            root_id: ft_norm.root_id,
            node_counter: ft_norm.node_counter,
            negate_or: false,
        }
    }
}

impl<T> Clone for FaultTree<T>
where
    T: Clone,
{
    fn clone(&self) -> Self {
        FaultTree {
            nodes: self.nodes.clone(),
            root_id: self.root_id,
            node_counter: AtomicUsize::new(
                self.node_counter.load(std::sync::atomic::Ordering::Relaxed),
            ),
            negate_or: self.negate_or,
        }
    }
}

/// A Fault Tree representation in Coyan.
/// Can be created empty or read from a file using FT normalizer.
/// They have some extra details:
/// - Handle the logic of the Tseitin Encoding
/// - Do not have information about names of nodes
/// - Do not have VOT gates
/// - Do not have negations in arguments, but in separated gates.
pub struct FaultTree<T> {
    pub nodes: IndexVec<NodeId, Node<T>>,
    pub root_id: NodeId,
    node_counter: AtomicUsize,
    negate_or: bool,
}

impl FaultTree<String> {
    pub fn empty() -> Self {
        FaultTree {
            nodes: IndexVec::new(),
            root_id: NodeId::new(0),
            node_counter: AtomicUsize::new(0),
            negate_or: false,
        }
    }

    /// Generate a FT from a dft file.
    pub fn new_from_file(filename: &str, simplify: bool, negate_or: bool) -> Self {
        let mut ft_norm = FaultTreeNormalizer::default();
        ft_norm.read_from_file(filename, simplify);
        let mut ft = FaultTree::from(ft_norm);
        ft.negate_or = negate_or;
        ft
    }

    /// Internal method, changes the id of the root node.
    fn _set_root(&mut self, new_root_id: NodeId) {
        self.root_id = new_root_id;
    }

    /// Add the node to the fields of the struct.
    pub fn add_node(&mut self, node: Node<String>, nid: NodeId) {
        self.nodes.insert(nid, node)
    }

    fn new_id(&self) -> NodeId {
        NodeId::new(
            self.node_counter
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        )
    }

    /// Update the roots of a Node in the nodes fields.
    pub fn update_roots(&mut self, new_node: Node<String>, nid: NodeId) {
        self.nodes.remove(nid);
        self.nodes.insert(nid, new_node);
    }

    /// Creates a new Fault Tree that is a submodule from the orignal one.
    /// It only contains the nodes that are used in the subtree
    pub fn subtree_with_root(&self, new_root_id: NodeId) -> FaultTree<String> {
        // Create New Fault Tree
        let mut sub_ft = FaultTree::empty();

        // Create a mapper to relate the old NodeIds to the new ones.
        let mut new_id_mapper = HashMap::new();

        // Create new ids for each children and add them to the subtree.
        let mut to_process = vec![new_root_id];
        while let Some(nid) = to_process.pop() {
            let node = self.nodes[nid].clone();
            let children = node.children();
            let new_nid = sub_ft.new_id();

            new_id_mapper.insert(nid, new_nid);
            sub_ft.add_node(node, new_nid);
            to_process.append(
                &mut children
                    .into_iter()
                    .filter(|c_id| !new_id_mapper.contains_key(c_id))
                    .collect(),
            );
        }

        // For each node, use the mapper to replace the node arguments.
        let _ = sub_ft
            .nodes
            .iter_mut()
            .map(|node| node.map_to_args(&new_id_mapper))
            .collect_vec();

        sub_ft
    }

    /// Return number of nodes in the tree.
    pub fn get_count(&self) -> usize {
        self.node_counter.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Get useful information from the Tree.
    pub fn get_info(&self, _preprocess: Option<String>) -> (usize, usize, usize) {
        fn visit_dfs(
            nodes_visits: &mut IndexVec<NodeId, bool>,
            nodes: &IndexVec<NodeId, Node<String>>,
            curr_idx: NodeId,
        ) {
            // Take current node using the idx
            let curr_node_visits = &mut nodes_visits[curr_idx];
            let curr_child: &Node<String> = &nodes[curr_idx];
            // First time? Mark and propagate.
            if !*curr_node_visits {
                *curr_node_visits = true;
                let children = curr_child.children();
                if !children.is_empty() {
                    for child_nid in children
                        .into_iter()
                        .filter(|c_id| !nodes_visits[*c_id])
                        .collect::<Vec<NodeId>>()
                    {
                        visit_dfs(nodes_visits, nodes, child_nid);
                    }
                }
            }
        }
        let mut vis_nodes: IndexVec<NodeId, bool> =
            IndexVec::from_vec(vec![false; self.nodes.len()]);
        visit_dfs(&mut vis_nodes, &self.nodes, self.root_id);

        let zipped = vis_nodes.iter().zip(&self.nodes).collect_vec();

        let num_be = zipped
            .iter()
            .filter(|(vis, n)| **vis && (matches!(n, Node::BasicEvent(_, _))))
            .count();

        let num_gates = zipped
            .iter()
            .filter(|(vis, n)| {
                **vis
                    && (matches!(
                        n,
                        Node::And(_) | Node::Or(_) | Node::Vot(_, _) | Node::Not(_) | Node::Xor(_)
                    ))
            })
            .count();

        let f = self.apply_tseitin();

        (
            num_be,
            num_gates,
            f.num_clauses()
                .expect("Top gate must be an AND to translate to CNF."),
        )
    }

    /// Apply the tseitin transformation to all the nodes in the tree.
    pub fn apply_tseitin(&self) -> Formula<NodeId> {
        let mut args = if self.nodes[self.root_id].is_or() && self.negate_or {
            vec![Formula::Not(Box::new(Formula::Atom(self.root_id)))]
        } else {
            vec![Formula::Atom(self.root_id)]
        };

        for (nid, node) in self.nodes.iter_enumerated() {
            match node.tseitin_transformation(nid) {
                Formula::And(or_args) => args.extend(or_args),
                Formula::Or(literals) => args.push(Formula::Or(literals)),
                Formula::True => {}
                _ => panic!("Something went wrong translating the Tseitin transformation."),
            }
        }

        Formula::And(args)
    }

    pub fn apply_tseitin_used_only(&self) -> Formula<NodeId> {
        let mut args = if self.nodes[self.root_id].is_or() && self.negate_or {
            vec![Formula::Not(Box::new(Formula::Atom(self.root_id)))]
        } else {
            vec![Formula::Atom(self.root_id)]
        };

        let mut to_process = vec![self.root_id];
        let mut seen = vec![self.root_id];
        while let Some(nid) = to_process.pop() {
            let node = self.nodes[nid].clone();
            let mut children = node.children();

            seen.append(&mut children);

            let mut unseen_children = children
                .into_iter()
                .filter(|cid| seen.contains(cid))
                .collect_vec();
            to_process.append(&mut unseen_children);

            match node.tseitin_transformation(nid) {
                Formula::And(or_args) => args.extend(or_args),
                Formula::Or(literals) => args.push(Formula::Or(literals)),
                Formula::True => {}
                _ => panic!("Something went wrong while translating the Tseitin transformation."),
            }
        }

        Formula::And(args)
    }

    /// Save the fault tree CNF formula into a .wcnf o .cnf file depending on the format.
    pub fn dump_cnf_to_file(
        &self,
        filename: String,
        timepoint: f64,
        w_file: Option<String>,
        unav: bool,
    ) {
        let (formula_cnf, weights) = self.implicit_formula(timepoint, unav);

        let mut f = File::create(filename).expect("unable to create file");
        f.write_all(formula_cnf.as_bytes())
            .expect("Error writing the formula to file");
        match w_file {
            None => {
                f.write_all(weights.as_bytes())
                    .expect("Error writing weights to file");
            }
            Some(w_filename) => {
                let mut w_f =
                    File::create(format!("{}.w", w_filename)).expect("unable to create file");
                w_f.write_all(weights.as_bytes())
                    .expect("Error writing the BE weights to file");
            }
        }
    }

    /// Internal method for code reading.
    /// Produces both the CNF of the implicit boolean formula and the weights in the specified format.
    fn implicit_formula(
        &self,
        timepoint: f64,
        unav: bool,
    ) -> (String, String) {
        let cnf_formula = self.apply_tseitin();
        let text_formula = cnf_formula.to_text();
        let n_vars = self.get_count();
        let n_clauses = cnf_formula
            .num_clauses()
            .expect("Top gate must be an AND to translate to CNF.");

        let (problem_line, weight_start) = (
                format!("p cnf {} {}\n", n_vars, n_clauses),
                String::from("c p weight"),
            );

        let (gate_weights, be_weights) = self.get_weights(weight_start, timepoint, unav);
        let be_weights = be_weights.join("\n");
        let gate_weights = gate_weights.join("\n");

        let mut formula_str = text_formula
            .replace(" ∧ ", " 0 \n")
            .replace(" V ", " ")
            .replace("(", "")
            .replace(")", "");
        formula_str.push_str(" 0 \n");

        let formula_cnf = format!("{}\n{}\n", problem_line, formula_str);

        let weights = format!("{}\n{}", be_weights, gate_weights);

        (formula_cnf, weights)
    }

    /// Dump the implicit formula in CNF format to a String.
    pub fn dump_cnf(
        &self,
        timepoint: f64,
        unav: bool,
    ) -> String {
        let (mut formula_cnf, weights) = self.implicit_formula(timepoint, unav);
        formula_cnf.push_str(&weights);

        formula_cnf
    }

    /// Gives the weights in DIMACS format for the Gates and of the BE respectively.
    fn get_weights(
        &self,
        weight_start: String,
        timepoint: f64,
        unav: bool,
    ) -> (Vec<String>, Vec<String>) {
        let n_vars = self.get_count();
        // We see which NodeId do not have the weights and set them to 1.
        // Only the BasicEvents Nodes have a fixed weight.
        let used_ids = self
            .nodes
            .iter()
            .enumerate()
            .filter_map(|(i, n)| match n {
                Node::BasicEvent(_, _) => Some(i),
                _ => None,
            })
            .collect_vec();

        // Obtain weights of the gates
        let gate_weights: Vec<String> = (0..n_vars)
            .filter_map(|i| {
                if !used_ids.contains(&i) {
                    Some(format!(
                        "{} {} 1 0\n{} -{} 1 0",
                        weight_start,
                        i + 1,
                        weight_start,
                        i + 1
                    ))
                } else {
                    None
                }
            })
            .collect_vec();

        // Obtain weights of the basic Events.
        let be_weights = self
            .nodes
            .iter()
            .enumerate()
            .filter_map(|(i, n)| match &n {
                Node::BasicEvent(_, be) => {
                    let weight: f64 = if !unav {
                        be.unreliability(timepoint)
                    } else {
                        be.unavailability(timepoint)
                    };
                    Some(format!(
                        "{} {} {} 0\n{} -{} {} 0",
                        weight_start,
                        i + 1,
                        weight,
                        weight_start,
                        i + 1,
                        1.0 - weight,
                    ))
                }
                _ => None,
            })
            .collect_vec();
        (gate_weights, be_weights)
    }

    /// Update a Node by replacing it with another one.
    pub fn update_root(&mut self, new_node: Node<String>, nid: NodeId) {
        self.nodes.remove(nid);
        self.nodes.insert(nid, new_node);
    }

}
