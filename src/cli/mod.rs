pub mod eval;
pub mod artifacts;
pub mod replay;
pub mod status;
pub mod demo;

use std::env;

pub async fn run() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        print_help();
        return;
    }

    match args[1].as_str() {
        "eval" => eval::run_async(&args[2..]).await,
        "artifacts" => artifacts::run(&args[2..]),
        "replay" => replay::run(&args[2..]),
        "status" => status::run(),
        _ => print_help(),
    }
}

fn print_help() {
    println!(r#"
TRAXES CLI

Commands:
  traxes eval <allow|deny|file>
  traxes artifacts list
  traxes artifacts last
  traxes artifacts show <id>
  traxes replay <id>
  traxes status
"#);
}
