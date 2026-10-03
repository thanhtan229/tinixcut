// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::env;

#[cfg(windows)]
fn attach_console() {
    #[link(name = "kernel32")]
    extern "system" {
        fn AttachConsole(dwProcessId: u32) -> i32;
    }
    const ATTACH_PARENT_PROCESS: u32 = 0xFFFFFFFF;
    unsafe {
        AttachConsole(ATTACH_PARENT_PROCESS);
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() > 1 {
        let subcmd = &args[1];
        if subcmd == "inspect"
            || subcmd == "diff"
            || subcmd == "apply"
            || subcmd == "render"
            || subcmd == "help"
            || subcmd == "--help"
            || subcmd == "-h"
        {
            #[cfg(windows)]
            attach_console();

            match tinixcut_lib::modules::automation::AutomationAdapter::handle_cli(&args[1..]) {
                Ok(output) => {
                    println!("{}", output);
                    std::process::exit(0);
                }
                Err(err) => {
                    eprintln!("Error: {}", err);
                    std::process::exit(1);
                }
            }
        }
    }

    tinixcut_lib::run()
}
