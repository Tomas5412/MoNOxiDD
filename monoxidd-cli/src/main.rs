use monoxidd_cnf::calculate_wmc;
use monoxidd_parser::load_file;
use monoxidd_fta::fault_tree::FaultTree;
use std::process::ExitCode;
use clap::Parser;
use structs::*;
mod structs;

#[derive(Parser, Debug)]
#[command(
    author = "Tomás Romanutti",
    version = "0.1",
    about = "
        MoNOxiDD is a tool to analize Fault Trees and formula in Conjunctive Normal Forms.
    "
)]
struct Arguments {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Parser, Clone, Debug)]
enum Command {
    #[clap(
        about = "Calculates the Weighted Model Count of the CNF file."
    )]
    Cnf(CnfCommand),
    #[clap(
        about = "Translates the Fault Tree to a CNF, then it calculates the WMC of it."
    )]
    Ft(FTCommand),
}


fn cnf_wc(
    filepath:&str,
    debug: bool, 
    wmc_type: &str,
    diagram_var_order: &str,
    cluster_var_order: &str,
    cluster_rank: &str,
    choose_cluster: &str) -> ExitCode {
    let Some(circuit) = load_file(filepath) else {
        return ExitCode::FAILURE; // the parser displayed the relevant errors
    };

    let model_count = match calculate_wmc(&circuit, debug, &wmc_type, &diagram_var_order, &cluster_var_order, &cluster_rank, &choose_cluster) {
    Ok(v) => v,
    Err(_) => {
        eprintln!("error: out of memory while building the diagrams");
        return ExitCode::FAILURE;
    }
    };
    let ans = f64::from(model_count);
    println!("{ans:e}");
    ExitCode::SUCCESS
}

fn cnf_command(command: CnfCommand) -> ExitCode {
    let filepath = command.input;
    let debug = command.debug;
    let wmc_type = command.wmc_type;
    let diagram_var_order = command.diagram_var_order;
    let cluster_var_order = command.cluster_var_order;
    let cluster_rank = command.cluster_rank;
    let choose_cluster = command.choose_cluster;
    cnf_wc(&filepath, debug, &wmc_type, &diagram_var_order, &cluster_var_order, &cluster_rank, &choose_cluster)
}

fn ft_wmc(command: FTCommand) -> ExitCode {
    let dft_filename = command.input;
    let cnf_filename = command.cnf_file;
    let w_file       = command.w_file;
    let ft = FaultTree::new_from_file(&dft_filename);
    let filepath = format!("{}.cnf", cnf_filename);
    ft.dump_cnf_to_file(
        filepath.clone(),
        command.timepoint,
        w_file,
    );

    let debug = command.debug;
    let wmc_type = command.wmc_type;
    let diagram_var_order = command.diagram_var_order;
    let cluster_var_order = command.cluster_var_order;
    let cluster_rank = command.cluster_rank;
    let choose_cluster = command.choose_cluster;

    cnf_wc(&filepath, debug, &wmc_type, &diagram_var_order, &cluster_var_order, &cluster_rank, &choose_cluster)
}


fn main() -> ExitCode {
    let arguments = Arguments::parse();
    match arguments.command {
        Command::Cnf(command) => cnf_command(command),
        Command::Ft(command) => ft_wmc(command),
    }
}