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
        if subcmd == "mcp" {
            // MCP runs strictly over stdin/stdout; redirect panic hook to stderr
            std::panic::set_hook(Box::new(|panic_info| {
                eprintln!("TinixCut MCP Panic: {panic_info}");
            }));

            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("Failed to initialize tokio runtime for MCP server");
            rt.block_on(async {
                if let Err(e) = tinixcut_lib::modules::automation::mcp::run_mcp_server().await {
                    eprintln!("TinixCut MCP Server error: {e}");
                    std::process::exit(1);
                }
            });
            std::process::exit(0);
        }

        if subcmd == "inspect"
            || subcmd == "diff"
            || subcmd == "apply"
            || subcmd == "render"
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
