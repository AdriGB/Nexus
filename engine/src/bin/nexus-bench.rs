use nexus_engine::benchmarking::{run_scenario_json, scenario_names};

fn main() {
    let mut arguments = std::env::args().skip(1);
    match (arguments.next().as_deref(), arguments.next()) {
        (Some("--list"), None) => {
            for name in scenario_names() {
                println!("{name}");
            }
        }
        (Some(name), None) => match run_scenario_json(name) {
            Ok(json) => println!("{json}"),
            Err(error) => exit_with_error(&error),
        },
        _ => exit_with_error("usage: nexus-bench --list | <scenario-name>"),
    }
}

fn exit_with_error(message: &str) -> ! {
    eprintln!("nexus-bench: {message}");
    std::process::exit(2);
}
