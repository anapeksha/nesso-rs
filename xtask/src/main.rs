mod lint;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let subcommand = args.get(1).map(String::as_str);

    match subcommand {
        Some("lint") => lint::run(),
        Some(unknown) => {
            eprintln!("unknown subcommand: {unknown}");
            eprintln!("available: lint");
            std::process::exit(1);
        }
        None => {
            eprintln!("usage: cargo xtask <subcommand>");
            eprintln!("available: lint");
            std::process::exit(1);
        }
    }
}
