#![windows_subsystem = "windows"]

use std::path::PathBuf;

fn main() {
    let target = std::env::args_os().nth(1).map(PathBuf::from);
    wholocks::gui::run(target);
}
