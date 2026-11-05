//! Foreground daemon and version 2 command-line client.

fn main() -> std::process::ExitCode {
    std::process::ExitCode::from(session_manager::cli::run(std::env::args_os().skip(1)))
}
