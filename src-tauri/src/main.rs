fn main() {
    let mut args = std::env::args().skip(1);
    if args.next().as_deref() == Some("hook") {
        let source = args.next().unwrap_or_else(|| "unknown".into());
        let action = args.next().unwrap_or_else(|| "unknown".into());
        std::process::exit(agent_snake_lib::run_hook(&source, &action));
    }
    agent_snake_lib::run();
}
