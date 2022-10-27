use atty::Stream;
use dirs;
use std::collections::hash_map::DefaultHasher;
use std::fs::File;
use std::hash::Hasher;
use std::io;
use std::io::Read;
use std::io::Write;
use std::process::Command;
use std::process::Output;
use std::process::Stdio;

fn prefix() -> String {
    format!("{}/.caru/", dirs::home_dir().unwrap().to_str().unwrap())
}

fn read_from_cache(full_cmd_hash: u64) -> io::Result<()> {
    let stderr_filename = format!("{}{:x}.stderr", prefix(), full_cmd_hash);
    let stdout_filename = format!("{}{:x}.stdout", prefix(), full_cmd_hash);

    let mut stderr_file = File::open(stderr_filename)?;
    let mut stdout_file = File::open(stdout_filename)?;

    const BUF_SIZE: usize = 2 << 12; // 4 KiB
    let mut buffer = [0u8; BUF_SIZE];

    loop {
        match stderr_file.read(&mut buffer) {
            Ok(0) => break,
            Ok(read_bytes) => {
                let read_buf = &buffer[..read_bytes];
                io::stdout().write_all(read_buf)?;
            }
            Err(e) => print!("{}", e),
        }
    }
    // io::stdout().flush()?;

    loop {
        match stdout_file.read(&mut buffer) {
            Ok(0) => break,
            Ok(read_bytes) => {
                let read_buf = &buffer[..read_bytes];
                io::stdout().write_all(read_buf)?;
            }
            Err(e) => print!("{}", e),
        }
    }
    // io::stdout().flush()?;

    Ok(())
}

// TODO improve get_cache_result api to return an optional
fn get_cache_result(full_cmd_hash: u64) -> io::Result<()> {
    let stderr_filename = format!("{}{:x}.stderr", prefix(), full_cmd_hash);
    let stdout_filename = format!("{}{:x}.stdout", prefix(), full_cmd_hash);

    let _stderr_file = File::open(stderr_filename)?;
    let _stdout_file = File::open(stdout_filename)?;

    Ok(())
}

// TODO: parallelize writing to stdout/stderr and to files
fn store_output(process_output: &Output, full_cmd_hash: u64) -> io::Result<()> {
    // TODO: mkdir -p

    let stderr_filename = format!("{}{:x}.stderr", prefix(), full_cmd_hash);
    let mut stderr_chache = File::create(stderr_filename)?;
    stderr_chache.write_all(&process_output.stderr)?;
    // stderr_chache.flush()?;

    let stdout_filename = format!("{}{:x}.stdout", prefix(), full_cmd_hash);
    let mut stdout_chache = File::create(stdout_filename)?;
    stdout_chache.write_all(&process_output.stdout)?;
    // stdout_chache.flush()?;

    Ok(())
}

fn main() -> io::Result<()> {
    let handle = if atty::is(Stream::Stdin) {
        None
    } else {
        Some(io::stdin().lock())
    };

    let cmd = std::env::args()
        .skip(1)
        .fold(String::new(), |acc, s| format!("{} {}", acc, s));

    let mut args_hasher = DefaultHasher::new();
    args_hasher.write(cmd.as_bytes());
    let args_hash = args_hasher.finish();

    let mut process = Command::new("sh")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .arg("-c")
        .arg(cmd)
        .spawn()
        .unwrap();

    const BUF_SIZE: usize = 2 << 12; // 4 KiB
    let mut buffer = [0u8; BUF_SIZE];

    let mut stdin_hasher = DefaultHasher::new();

    let stdin_hash = match handle {
        Some(mut h) => {
            loop {
                match h.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(read_bytes) => {
                        let read_buf = &buffer[..read_bytes];
                        process.stdin.as_mut().unwrap().write(read_buf).unwrap();
                        stdin_hasher.write(read_buf);
                    }
                    Err(e) => print!("{}", e),
                };
            }
            stdin_hasher.finish()
        }
        None => 0,
    };

    // TODO: investigate dropping stdin: according to
    // https://stackoverflow.com/questions/49218599/write-to-child-process-stdin-in-rust we might
    // nedd to drop stdin

    let mut full_cmd_hasher = DefaultHasher::new();
    full_cmd_hasher.write_u64(stdin_hash);
    full_cmd_hasher.write_u64(args_hash);

    let full_cmd_hash = full_cmd_hasher.finish();

    match get_cache_result(full_cmd_hash) {
        Ok(()) => {
            // println!("Command was already executed!");
            process.kill().unwrap();
            read_from_cache(full_cmd_hash).unwrap();
        }
        Err(e) => match e.kind() {
            std::io::ErrorKind::NotFound => {
                // println!("New execution")
            }
            e => println!("{}", e),
        },
    };

    let process_output = process.wait_with_output().unwrap();
    if process_output.status.success() {
        store_output(&process_output, full_cmd_hash).unwrap();

        io::stdout().write_all(&process_output.stderr).unwrap();
        io::stdout().write_all(&process_output.stdout).unwrap();
    }
    Ok(())
}
