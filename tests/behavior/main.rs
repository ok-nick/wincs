use std::process::ExitCode;

use libtest_mimic::{run, Arguments, Trial};

mod async_filter;
mod sync_filter;

fn main() -> ExitCode {
    let args = Arguments::from_args();
    let tests = vec![Trial::test("sync_filter", sync_filter::test)];

    let conclusion = run(&args, tests);
    if conclusion.has_failed() {
        return conclusion.exit_code();
    }

    let tests = vec![Trial::test("async_filter", async_filter::test)];
    let conclusion = run(&args, tests);

    conclusion.exit_code()
}

fn test_list_folders(root: &str) {
    let output = powershell_script::run(&format!("Get-ChildItem {root} -Recurse -Name"))
        .expect("run script");
    assert_eq!(
        "dir1\r\n\
        test1.txt\r\n\
        dir1\\test2.txt\r\n",
        output.stdout().expect("stdout"),
    );
}

fn test_read_file(root: &str) {
    for relative in ["test1.txt", "dir1\\test2.txt"] {
        let path = format!("{root}\\{relative}");
        let output =
            powershell_script::run(&format!("Get-Content {path} -Raw")).expect("run script");
        assert_eq!(output.stdout().expect("stdout"), format!("{relative}\r\n"));
    }
}
