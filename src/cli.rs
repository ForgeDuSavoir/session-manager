//! CLI grammar and process entry orchestration; clients never open persisted state.

use crate::{
    application::Core,
    applications::ApplicationResolver,
    bootstrap::{Startup, client_runtime},
    canonical_json,
    error::{Error, Result},
    interface::{Command, Service, WireError, canonical_uuid},
    transport::{Server, call},
};
use nix::sys::signal::{SigSet, SigmaskHow, Signal, pthread_sigmask};
use std::{
    ffi::OsString,
    io::{self, Write},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

pub const HELP: &str = "Session Manager (protocol 2)\n\nUsage: session-manager [--json] COMMAND\n  daemon\n  project list\n  project create --base-name NAME --template NAME\n  template list\n  session list\n  session start NAME\n  session switch UUID\n  session stop UUID\n  application list\n  --help | --version\n\n--json prints the exact API result or error object. It may also follow an API command.\nStart/switch/stop change logical state only; no applications are executed or closed.\n";
#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    Help,
    Version,
    Daemon,
    Request(Command),
}
#[derive(Debug)]
pub struct Invocation {
    pub action: Action,
    pub json: bool,
}
pub fn parse(args: &[String]) -> Result<Invocation> {
    let bad = Error("cli.invalid_arguments");
    let mut args = args;
    let mut json = false;
    if args.first().is_some_and(|s| s == "--json") {
        json = true;
        args = &args[1..];
    }
    if args.is_empty() {
        return if json {
            Err(bad)
        } else {
            Ok(Invocation {
                action: Action::Help,
                json,
            })
        };
    }
    let action = match args {
        [cmd] if cmd == "--help" || cmd == "-h" => Action::Help,
        [cmd] if cmd == "--version" => Action::Version,
        [cmd] if cmd == "daemon" => Action::Daemon,
        [group, operation, tail @ ..] => {
            let required = usize::from(
                group == "session" && matches!(operation.as_str(), "start" | "switch" | "stop"),
            );
            let mut tail = tail;
            let flag_can_follow =
                !(group == "project" && operation == "create") || tail.len() % 2 == 1;
            if flag_can_follow
                && tail.len() > required
                && tail.last().is_some_and(|s| s == "--json")
            {
                if json {
                    return Err(bad);
                }
                json = true;
                tail = &tail[..tail.len() - 1];
            }
            let command = match (group.as_str(), operation.as_str(), tail) {
                ("project", "list", []) => Command::Projects,
                ("template", "list", []) => Command::Templates,
                ("session", "list", []) => Command::Sessions,
                ("application", "list", []) => Command::Applications,
                ("session", "start", [name]) => Command::Start {
                    project_name: name.clone(),
                },
                ("session", "switch", [id]) => Command::Switch {
                    session_id: canonical_uuid(id).map_err(|_| bad)?,
                },
                ("session", "stop", [id]) => Command::Stop {
                    session_id: canonical_uuid(id).map_err(|_| bad)?,
                },
                ("project", "create", tail) => parse_create(tail)?,
                _ => return Err(bad),
            };
            Action::Request(command)
        }
        _ => return Err(bad),
    };
    if json && !matches!(action, Action::Request(_)) {
        return Err(bad);
    }
    Ok(Invocation { action, json })
}
fn parse_create(args: &[String]) -> Result<Command> {
    let mut base = None;
    let mut template = None;
    let mut i = 0;
    while i < args.len() {
        let value = args
            .get(i + 1)
            .ok_or(Error("cli.invalid_arguments"))?
            .clone();
        match args[i].as_str() {
            "--base-name" if base.is_none() => base = Some(value),
            "--template" if template.is_none() => template = Some(value),
            _ => return Err(Error("cli.invalid_arguments")),
        }
        i += 2;
    }
    Ok(Command::Create {
        base_name: base.ok_or(Error("cli.invalid_arguments"))?,
        template_name: template.ok_or(Error("cli.invalid_arguments"))?,
    })
}
pub fn run(args: impl IntoIterator<Item = OsString>) -> u8 {
    let args: std::result::Result<Vec<_>, _> =
        args.into_iter().map(OsString::into_string).collect();
    let Ok(args) = args else {
        report(&Error("cli.invalid_arguments").into(), false);
        return 2;
    };
    let invocation = match parse(&args) {
        Ok(i) => i,
        Err(e) => {
            report(&e.into(), args.iter().any(|s| s == "--json"));
            return 2;
        }
    };
    match invocation.action {
        Action::Help => {
            print!("{HELP}");
            0
        }
        Action::Version => {
            println!("session-manager {} (protocol 2)", env!("CARGO_PKG_VERSION"));
            0
        }
        Action::Daemon => match daemon() {
            Ok(()) => 0,
            Err(e) => {
                report(&e.into(), false);
                1
            }
        },
        Action::Request(command) => {
            let result = client_runtime()
                .map_err(WireError::from)
                .and_then(|runtime| call(&runtime, &command));
            match result {
                Ok(value) => {
                    let output = if invocation.json {
                        canonical_json::encode_line(&value).map_err(|_| Error("internal.error"))
                    } else {
                        serde_json::to_vec_pretty(&value)
                            .map(|mut v| {
                                v.push(b'\n');
                                v
                            })
                            .map_err(|_| Error("internal.error"))
                    };
                    match output.and_then(|bytes| {
                        io::stdout()
                            .lock()
                            .write_all(&bytes)
                            .map_err(|_| Error("transport.unavailable"))
                    }) {
                        Ok(()) => 0,
                        Err(e) => {
                            report(&e.into(), invocation.json);
                            1
                        }
                    }
                }
                Err(error) => {
                    report(&error, invocation.json);
                    1
                }
            }
        }
    }
}
fn report(error: &WireError, json: bool) {
    if json {
        if let Ok(bytes) = canonical_json::encode_line(error) {
            let _ = io::stdout().lock().write_all(&bytes);
        }
    } else {
        eprintln!("session-manager: {}", error.code);
    }
}
fn daemon() -> Result<()> {
    let startup = Startup::capture()?;
    startup.prepare_state_parent()?;
    // Block only in this daemon process, before creating threads. No async signal handler.
    let mut signals = SigSet::empty();
    signals.add(Signal::SIGINT);
    signals.add(Signal::SIGTERM);
    pthread_sigmask(SigmaskHow::SIG_BLOCK, Some(&signals), None)?;
    let core = Core::open(&startup.locations)?;
    if core.recovery_required()? {
        eprintln!("session-manager: recovery.invalid_runtime");
    }
    if !core.incomplete_creations().is_empty() {
        eprintln!(
            "session-manager: recovery.incomplete_creations ({})",
            core.incomplete_creations().len()
        );
    }
    let server = Server::bind(
        &startup.locations.runtime,
        Service::new(core, ApplicationResolver::new(startup.applications)),
    )?;
    let stop = Arc::new(AtomicBool::new(false));
    let signal_stop = Arc::clone(&stop);
    let waiter = std::thread::Builder::new()
        .name("session-manager-signals".into())
        .spawn(move || {
            let result = signals
                .wait()
                .map(|_| ())
                .map_err(|_| Error("internal.error"));
            signal_stop.store(true, Ordering::Release);
            result
        })
        .map_err(|_| Error("internal.error"))?;
    let result = server.serve_until(&stop);
    if stop.load(Ordering::Acquire) {
        waiter.join().map_err(|_| Error("internal.error"))??;
    }
    result
}
