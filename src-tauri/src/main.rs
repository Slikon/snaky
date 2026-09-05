fn main() {
    let mut args = std::env::args().skip(1);
    let mode = args.next();
    if mode.as_deref() == Some("--install-integrations") {
        match agent_snake_lib::install_integrations() {
            Ok(report) => println!("{}", serde_json::to_string(&report).unwrap()),
            Err(error) => { eprintln!("{error}"); std::process::exit(1); }
        }
        return;
    }
    if mode.as_deref() == Some("hook") {
        let source = args.next().unwrap_or_else(|| "unknown".into());
        let action = args.next().unwrap_or_else(|| "unknown".into());
        std::process::exit(agent_snake_lib::run_hook(&source, &action));
    }
    agent_snake_lib::run();
}
