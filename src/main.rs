use std::{collections::HashSet, env, error::Error, path::PathBuf};

use wholocks::{find_locks, install_context_menu, terminate_process, uninstall_context_menu};

fn print_usage() {
    eprintln!("Usage:");
    eprintln!(r#"  wholocks "C:\path\to\file-or-folder""#);
    eprintln!(r#"  wholocks --kill "C:\path\to\file-or-folder""#);
    eprintln!("  wholocks --install-context-menu");
    eprintln!("  wholocks --uninstall-context-menu");
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut arguments = env::args_os().skip(1);
    let Some(first_argument) = arguments.next() else {
        print_usage();
        return Ok(());
    };

    if first_argument == "--install-context-menu" {
        if arguments.next().is_some() {
            print_usage();
            return Ok(());
        }
        let gui_path = install_context_menu()?;
        println!("Installed the Explorer context menu for:");
        println!("{}", gui_path.display());
        println!("On Windows 11, use Show more options > WhoLocks");
        return Ok(());
    }

    if first_argument == "--uninstall-context-menu" {
        if arguments.next().is_some() {
            print_usage();
            return Ok(());
        }
        uninstall_context_menu()?;
        println!("Removed the WhoLocks Explorer context menu.");
        return Ok(());
    }

    let kill = first_argument == "--kill";
    let path = if kill {
        let Some(path) = arguments.next() else {
            eprintln!("Missing path after --kill.");
            return Ok(());
        };
        path
    } else {
        first_argument
    };

    if arguments.next().is_some() {
        eprintln!("Expected exactly one path.");
        return Ok(());
    }

    let path = PathBuf::from(path);
    if !path.exists() {
        eprintln!("Path does not exist:");
        eprintln!("{}", path.display());
        return Ok(());
    }

    println!("Scanning Windows file and folder handles...");
    let report = find_locks(&path)?;

    if report.processes.is_empty() {
        println!("No matching handles found.");
        if report.inaccessible_process_count > 0 {
            println!("Some protected processes could not be inspected.");
        }
        return Ok(());
    }

    for process in &report.processes {
        println!();
        println!("Process : {}", process.executable.display());
        println!("PID     : {}", process.pid);
        for handle in &process.matched_handles {
            println!("Handle  : {handle}");
        }
    }

    if kill {
        let pids: HashSet<_> = report.processes.iter().map(|process| process.pid).collect();
        let mut failures = Vec::new();

        for pid in pids {
            match terminate_process(pid) {
                Ok(()) => println!("Terminated PID: {pid}"),
                Err(error) => {
                    eprintln!("Could not terminate PID {pid}: {error}");
                    failures.push(pid);
                }
            }
        }

        if !failures.is_empty() {
            return Err(format!("could not terminate {} process(es)", failures.len()).into());
        }
    }

    Ok(())
}
