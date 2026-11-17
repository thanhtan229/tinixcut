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
        if matches!(subcmd.as_str(), "commands" | "call" | "api-session") {
            #[cfg(windows)]
            attach_console();
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("CLI runtime");
            if let Err(error) = rt.block_on(tinixcut_lib::modules::automation::cli::run(&args[1..]))
            {
                eprintln!("{error}");
                std::process::exit(1);
            }
            return;
        }
        if subcmd == "inspect"
            || subcmd == "diff"
            || subcmd == "apply"
            || subcmd == "render"
            || subcmd == "chapters"
            || subcmd == "session"
            || subcmd == "--stdio"
            || subcmd == "template"
            || subcmd == "help"
            || subcmd == "--help"
            || subcmd == "-h"
        {
            #[cfg(windows)]
            attach_console();

            if subcmd == "session" || subcmd == "--stdio" {
                let stdin = std::io::stdin().lock();
                let stdout = std::io::stdout();
                let stderr = std::io::stderr();
                tinixcut_lib::modules::automation::run_stdio_session(stdin, stdout, stderr);
                std::process::exit(0);
            }

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
