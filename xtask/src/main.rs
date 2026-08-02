#![forbid(unsafe_code)]

mod recipes;
mod simdoc;

fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    let program = args.first().map(String::as_str).unwrap_or("xtask");
    let result = match args.get(1).map(String::as_str) {
        Some("simdoc" | "check-file-sizes") => simdoc::run(args),
        Some("check-recipes") if args.len() == 2 => recipes::run(),
        _ => Err(format!(
            "usage: {program} simdoc [--check] | check-recipes | check-file-sizes"
        )),
    };
    if let Err(err) = result {
        eprintln!("{err}");
        std::process::exit(1);
    }
}
