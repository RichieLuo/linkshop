use linkshop::postman;

pub fn run(args: &[std::ffi::OsString]) -> bool {
    if args.first().is_some_and(|arg| arg == "import") {
        match postman::import_profile(
            &args[1..]
                .iter()
                .map(std::path::PathBuf::from)
                .collect::<Vec<_>>(),
        ) {
            Ok(report) => println!("{}", serde_json::to_string(&report).unwrap()),
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(1);
            }
        }
        return true;
    }
    false
}
