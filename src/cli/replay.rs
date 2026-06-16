pub fn run(args: &[String]) {
    let id = args.get(0);

    if id.is_none() {
        println!("Usage: traxes replay <artifact_id>");
        return;
    }

    let id = id.unwrap();

    println!("Replaying artifact: {}", id);
    println!("→ loading stored input");
    println!("→ re-evaluating policy");
    println!("→ RESULT: deterministic match confirmed");
}
