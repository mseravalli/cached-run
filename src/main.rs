use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::fs::File;
use std::hash::Hasher;
use std::io;
use std::io::{Read, Write};
use std::process::{Command, Output, Stdio};

const BUF_SIZE: usize = 1 << 13; // 8 KiB

struct Entry {
    stderr_file: File,
    stdout_file: File,
}

impl Entry {
    pub fn new(stderr_file: File, stdout_file: File) -> Self {
        Self {
            stderr_file,
            stdout_file,
        }
    }

    pub fn write_to_stderr_stdout(&mut self) -> io::Result<()> {
        let mut buffer = [0u8; BUF_SIZE];

        loop {
            let read_bytes = self.stderr_file.read(&mut buffer)?;
            if read_bytes == 0 {
                break;
            } else {
                let read_buf = &buffer[..read_bytes];
                io::stderr().write_all(read_buf)?;
            }
        }
        io::stderr().flush()?;

        loop {
            let read_bytes = self.stdout_file.read(&mut buffer)?;
            if read_bytes == 0 {
                break;
            } else {
                let read_buf = &buffer[..read_bytes];
                io::stdout().write_all(read_buf)?;
            }
        }
        io::stdout().flush()?;

        Ok(())
    }
}

struct Cache {
    prefix: String,
}

impl Cache {
    pub fn new() -> Self {
        let prefix = format!("{}/.caru/", dirs::home_dir().unwrap().to_str().unwrap());
        fs::create_dir_all(&prefix).unwrap();
        Self { prefix }
    }

    fn try_get_stderr_stout_files(&self, entry_hash: u64) -> io::Result<(File, File)> {
        let stderr_filename = format!("{}{:x}.stderr", self.prefix, entry_hash);
        let stdout_filename = format!("{}{:x}.stdout", self.prefix, entry_hash);

        let stderr_file = File::open(stderr_filename)?;
        let stdout_file = File::open(stdout_filename)?;

        Ok((stderr_file, stdout_file))
    }

    pub fn get(&self, entry_hash: u64) -> io::Result<Option<Entry>> {
        match self.try_get_stderr_stout_files(entry_hash) {
            Ok((stderr_file, stdout_file)) => Ok(Some(Entry::new(stderr_file, stdout_file))),
            Err(e) => match e.kind() {
                std::io::ErrorKind::NotFound => Ok(None),
                _ => Err(e),
            },
        }
    }

    // TODO: parallelize writing to stdout/stderr and to files
    pub fn insert(&self, process_output: &Output, full_cmd_hash: u64) -> io::Result<()> {
        let stderr_filename = format!("{}{:x}.stderr", self.prefix, full_cmd_hash);
        let mut stderr_chache = File::create(stderr_filename)?;
        stderr_chache.write_all(&process_output.stderr)?;
        stderr_chache.flush()?;

        let stdout_filename = format!("{}{:x}.stdout", self.prefix, full_cmd_hash);
        let mut stdout_chache = File::create(stdout_filename)?;
        stdout_chache.write_all(&process_output.stdout)?;
        stdout_chache.flush()?;

        Ok(())
    }
}

fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().collect();

    let force_running_command = args.get(1).map(|x| *x == "-f").unwrap_or(false);
    let read_from_cache = !force_running_command;

    let args_to_skip = if force_running_command { 2 } else { 1 };

    let cmd = std::env::args()
        .skip(args_to_skip)
        .fold(String::new(), |acc, s| format!("{} {}", acc, s));

    let mut process = Command::new("sh")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .arg("-c")
        .arg(&cmd)
        .spawn()?;

    let mut hasher = DefaultHasher::new();
    hasher.write(cmd.as_bytes());

    if !atty::is(atty::Stream::Stdin) {
        let mut stdin_handle = io::stdin().lock();
        let mut buffer = [0u8; BUF_SIZE];
        let process_stdin = process.stdin.as_mut().unwrap();
        loop {
            let read_bytes = stdin_handle.read(&mut buffer)?;
            if read_bytes == 0 {
                break;
            }
            let read_buf = &buffer[..read_bytes];
            process_stdin.write_all(read_buf)?;
            hasher.write(read_buf);
        }
    }

    let full_cmd_hash = hasher.finish();

    let cache = Cache::new();

    if read_from_cache {
        if let Some(mut entry) = cache.get(full_cmd_hash)? {
            process.kill()?;
            entry.write_to_stderr_stdout()?;
        };
    }

    let process_output = process.wait_with_output()?;
    if process_output.status.success() {
        cache.insert(&process_output, full_cmd_hash)?;

        io::stderr().write_all(&process_output.stderr)?;
        io::stdout().write_all(&process_output.stdout)?;
    }
    Ok(())
}
