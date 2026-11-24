use clap::Parser;

#[derive(Parser, Debug, Clone)]
pub struct CnfCommand{
	/// Input file containing the cnf file.
    #[arg(short, long, required = true)]
    pub input: String,
    /// Type of Weighted Model Counting. Can only accept 'Mono' or 'ADDMC' [default: 'ADDMC']
    #[arg(long, default_value = "ADDMC")]
    pub wmc_type: String,
    /// Print debugging metrics.
    #[arg(long, default_value_t = false)]
    pub debug: bool,
    /// Heuristic for diagram var order
    #[arg(long, default_value = "Random")]
    pub diagram_var_order: String,
    /// Heuristic for cluster var order
    #[arg(long, default_value = "Random")]
    pub cluster_var_order: String,
    /// Heuristic for choosing the clause rank
    #[arg(long, default_value = "Mono")]
    pub cluster_rank: String,
    /// Heuristic for choosing the next cluster
    #[arg(long, default_value = "List")]
    pub choose_cluster: String,
}

#[derive(Parser, Debug, Clone)]
pub struct FTCommand{
	/// Input file containing the Fault Tree file.
    #[arg(short, long, required = true)]
    pub input: String,
    /// Compute TEP of the FT a given timepoint.
    #[arg(short, long, default_value_t = 1.0)]
    pub timepoint: f64,
    /// Where the cnf translation would reside.
    #[arg(short, long, default_value = "temp/temp")]
    pub cnf_file: String,
    /// Print debugging metrics.
    #[arg(long, default_value_t = false)]
    pub debug: bool,
    /// Type of Weighted Model Counting. Can only accept 'Mono' or 'ADDMC' [default: 'ADDMC']
    #[arg(long, default_value = "ADDMC")]
    pub wmc_type: String,
    /// Heuristic for diagram var order
    #[arg(long, default_value = "Random")]
    pub diagram_var_order: String,
    /// Heuristic for cluster var order
    #[arg(long, default_value = "Random")]
    pub cluster_var_order: String,
    /// Heuristic for clause rank
    #[arg(long, default_value = "Mono")]
    pub cluster_rank: String,
    /// Heuristic for choosing the next cluster
    #[arg(long, default_value = "List")]
    pub choose_cluster: String,
    /// If provided, the weights will be written to a separate file.
    #[arg(long)]
    pub w_file: Option<String>,
}