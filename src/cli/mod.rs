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
        "eval" => {
            if args.len() < 3 {
                eprintln!("Error: eval command requires a payload file");
                eprintln!();
                eprintln!("USAGE:");
                eprintln!("  traxes-demo eval <payload-file>");
                eprintln!();
                eprintln!("EXAMPLES:");
                eprintln!("  traxes-demo eval my_payload.json");
                eprintln!("  traxes-demo eval allow  (use embedded allow example)");
                eprintln!("  traxes-demo eval deny   (use embedded deny example)");
                std::process::exit(1);
            }
            eval::run_async(&args[2..]).await;
        }
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
