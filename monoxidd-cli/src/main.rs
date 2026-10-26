use monoxidd_cnf::calculate_wmc;
use monoxidd_parser::load_file;
use std::env;
use std::process::ExitCode;

fn main() -> ExitCode {
    let Some(filepath) = env::args().nth(1) else {
        eprintln!("usage: monoxidd-cli <file.cnf>");
        return ExitCode::FAILURE;
    };

    let Some(circuit) = load_file(&filepath) else {
        return ExitCode::FAILURE; // load_file presumably already reported the parse error
    };

    let model_count = match calculate_wmc(&circuit) {
    Ok(v) => v,
    Err(_) => {
        eprintln!("error: out of memory while building the diagrams");
        return ExitCode::FAILURE;
    }
    };
    println!("{model_count}");
    ExitCode::SUCCESS
}