use clap::Parser;

#[derive(Parser, Debug, Clone)]
pub struct CnfCommand{
	/// Input file containing the cnf file.
    #[arg(short, long, required = true)]
    pub input: String,
    /// Type of Weighted Model Counting. Can only accept 'Mono' or 'ADDMC' [default: 'Mono']
    #[arg(long, default_value = "Mono")]
    pub format: String,
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
    /// Type of Weighted Model Counting. Can only accept 'Mono' or 'ADDMC' [default: 'Mono']
    #[arg(long, default_value = "Mono")]
    pub format: String,
    /// If provided, the weights will be written to a separate file.
    #[arg(long)]
    pub w_file: Option<String>,
}