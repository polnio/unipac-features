use std::process::Command;

pub fn is_elevated() -> bool {
    let uid = unsafe { libc::getuid() };
    uid == 0
}

pub fn elevate() {
    if is_elevated() {
        return;
    }
    let status = if cfg!(target_os = "windows") {
        Command::new("runas")
            .arg("/user:Administrator")
            .args(std::env::args())
            .status()
            .expect("Failed to run as elevated")
    } else {
        Command::new("sudo")
            .args(["--preserve-env=HOME,EDITOR", "--"])
            .args(std::env::args())
            .status()
            .expect("Failed to run as elevated")
    };
    std::process::exit(status.code().unwrap_or(1));
}
