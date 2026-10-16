use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use hostmask_core::platform::{self, PlatformError};
use hostmask_core::{combine, CombinedSource, SetKind, Store, StoreError};

const USAGE: &str = "\
hostmask — manage named hosts-file sets (GasMask-compatible)

USAGE:
  hostmask list                     list all sets
  hostmask show <name>              print a set
  hostmask new <name> [file]        create a set (from file, or empty)
  hostmask save <name> <file>       overwrite a set from a file
  hostmask rm <name>                delete a set
  hostmask rename <old> <new>       rename a set
  hostmask current                  show the active set
  hostmask activate <name>          make a set active and install it to the system hosts file
  hostmask next | prev              activate the next / previous set (wraps around)
  hostmask combine <out> <name>...  build a combined set from several sets
  hostmask system                   print the current system hosts file

ENV:
  HOSTMASK_HOME   data directory (default: platform-specific, see README)
";

fn main() -> ExitCode {
    match run(env::args().skip(1).collect()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(msg) => {
            eprintln!("error: {msg}");
            ExitCode::FAILURE
        }
    }
}

type CmdResult = Result<(), String>;

fn run(args: Vec<String>) -> CmdResult {
    let store = open_store()?;
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match args.as_slice() {
        [] | ["help", ..] | ["-h", ..] | ["--help", ..] => {
            print!("{USAGE}");
            Ok(())
        }
        ["list"] => list(&store),
        ["show", name] => {
            print!("{}", store.read(name).map_err(err)?);
            Ok(())
        }
        ["new", name] => store.create(name, "").map_err(err),
        ["new", name, file] => store.create(name, &read_file(file)?).map_err(err),
        ["save", name, file] => store.save(name, &read_file(file)?).map_err(err),
        ["rm", name] => store.delete(name).map_err(err),
        ["rename", from, to] => store.rename(from, to).map_err(err),
        ["current"] => {
            match store.active().map_err(err)? {
                Some(n) => println!("{n}"),
                None => println!("(none)"),
            }
            Ok(())
        }
        ["activate", name] => activate(&store, name),
        ["next"] => switch(&store, 1),
        ["prev"] => switch(&store, -1),
        ["combine", out, names @ ..] if !names.is_empty() => combine_sets(&store, out, names),
        ["system"] => {
            print!("{}", platform::read_system_hosts().map_err(|e| e.to_string())?);
            Ok(())
        }
        _ => Err(format!("unknown command: {}\n\n{USAGE}", args.join(" "))),
    }
}

fn open_store() -> Result<Store, String> {
    let root = env::var_os("HOSTMASK_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(platform::default_data_dir);
    Store::open(root).map_err(err)
}

fn list(store: &Store) -> CmdResult {
    let active = store.active().map_err(err)?;
    for set in store.list().map_err(err)? {
        let mark = if active.as_deref() == Some(set.name.as_str()) { "*" } else { " " };
        let kind = match set.kind {
            SetKind::Local => "local",
            SetKind::Remote => "remote",
            SetKind::Combined => "combined",
        };
        println!("{mark} {:<24} {kind}", set.name);
    }
    Ok(())
}

fn activate(store: &Store, name: &str) -> CmdResult {
    let contents = store.read(name).map_err(err)?;
    platform::install_system_hosts(&contents).map_err(platform_err)?;
    store.set_active(Some(name)).map_err(err)?;
    println!("activated {name}");
    Ok(())
}

fn switch(store: &Store, step: i32) -> CmdResult {
    match store.neighbour(step).map_err(err)? {
        Some(name) => activate(store, &name),
        None => Err("no sets to switch between".into()),
    }
}

fn combine_sets(store: &Store, out: &str, names: &[&str]) -> CmdResult {
    let contents: Vec<(String, String)> = names
        .iter()
        .map(|n| store.read(n).map(|c| ((*n).to_owned(), c)).map_err(err))
        .collect::<Result<_, _>>()?;
    let sources: Vec<CombinedSource<'_>> = contents
        .iter()
        .map(|(name, c)| CombinedSource { name, contents: c })
        .collect();
    let text = combine(&sources);
    store.create_kind(SetKind::Combined, out, &text).map_err(err)?;
    println!("created combined set {out} from {} sets", names.len());
    Ok(())
}

fn read_file(path: &str) -> Result<String, String> {
    fs::read_to_string(path).map_err(|e| format!("cannot read {path}: {e}"))
}

fn err(e: StoreError) -> String {
    e.to_string()
}

fn platform_err(e: PlatformError) -> String {
    match e {
        PlatformError::Cancelled => "cancelled — the system hosts file was not changed".into(),
        other => other.to_string(),
    }
}
