mod cli;
mod config;
mod transcript;

fn main() {
    if let Err(error) = cli::run() {
        eprintln!("{error:#}");
        std::process::exit(1);
    }
}
