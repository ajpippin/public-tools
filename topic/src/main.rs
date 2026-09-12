mod cli;
mod config;
mod store;
mod version;

fn main() {
    if let Err(error) = cli::run() {
        eprintln!("{error:#}");
        std::process::exit(1);
    }
}
