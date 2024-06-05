/*
use std::{env, io::Write, path::{self, PathBuf}, process::Command};

// #[test]
fn test_file_transfer() {
    let server_bin = cargo_bin_str("gday_contact_exchange_server");
    let mut server_process = Command::new(server_bin)
        .arg("--unencrypted")
        .spawn()
        .expect("Failed to start gday server for test.");

    let mut file_to_send = tempfile::tempfile().expect("Couldn't create test file.");
    file_to_send.write_all(b"Hello, world!").expect("Couldn't create test file.");


    let peer_a_dir = tempfile::tempdir().expect("Couldn't create tmp file");
    let peer_a_paths = vec![PathBuf::from(peer_a_dir.path())];


    let args = crate::Args {
        operation: crate::Command::Send {
            code: Some(String::from("1.188T.W3H.E")),
            paths: peer_a_paths
        },
        server: Some(String::from("localhost")),
        port: None,
        unencrypted: true,
        verbosity: log::LevelFilter::Warn,
    };

    crate::run::run(args);



    server_process.kill().expect("Couldn't stop gday server");
}

// Taken from assert_cmd
fn target_dir() -> path::PathBuf {
    env::current_exe()
        .ok()
        .map(|mut path| {
            path.pop();
            if path.ends_with("deps") {
                path.pop();
            }
            path
        })
        .unwrap()
}

// Taken from assert_cmd
fn cargo_bin_str(name: &str) -> path::PathBuf {
    let env_var = format!("CARGO_BIN_EXE_{}", name);
    std::env::var_os(env_var)
        .map(|p| p.into())
        .unwrap_or_else(|| target_dir().join(format!("{}{}", name, env::consts::EXE_SUFFIX)))
}
*/
