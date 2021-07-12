use anyhow::{Context, Result};
#[cfg(features = "unicorn")]
use timeout_readwrite::TimeoutReader;

use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Command, Stdio};
use std::time::Duration;
use log::*;

use crate::program_io::ProgramIO;

pub struct LocalIO {
    process_handle: std::process::Child,
    #[cfg(features = "unicorn")]
    stdout_reader: BufReader<TimeoutReader<std::process::ChildStdout>>,
    #[cfg(not(features = "unicorn"))]
    stdout_reader: BufReader<std::process::ChildStdout>,
    cmd: String,
}

impl LocalIO {
    pub fn new(file: &str, args: &[&str]) -> Result<Self> {
        let mut process_handle = Command::new(&file)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .context("Couldn't spawn process")?;

        #[cfg(features = "unicorn")]
        let stdout_reader = BufReader::new(TimeoutReader::new(
            process_handle.stdout.take().unwrap(),
            Duration::new(5, 0),
        ));

        #[cfg(not(features = "unicorn"))]
        let stdout_reader = BufReader::new(process_handle.stdout.take().unwrap());

        Ok(LocalIO {
            process_handle,
            stdout_reader,
            cmd: file.to_owned(),
        })
    }
}

impl ProgramIO for LocalIO {
    fn restart(&mut self) -> Result<()> {
        let args: &[&str] = &[];
        let mut process_handle = Command::new(&self.cmd)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .context("Couldn't spawn process")?;

        #[cfg(features = "unicorn")]
        let stdout_reader = BufReader::new(TimeoutReader::new(
            process_handle.stdout.take().unwrap(),
            Duration::new(5, 0),
        ));

        #[cfg(not(features = "unicorn"))]
        let stdout_reader = BufReader::new(process_handle.stdout.take().unwrap());

        self.process_handle = process_handle;
        self.stdout_reader = stdout_reader;
        Ok(())
    }

    fn send(&mut self, data: &[u8]) -> Result<()> {
        self.process_handle
            .stdin
            .as_mut()
            .unwrap()
            .write_all(data.as_ref())
            .context("Failed to send to process")?;

        Ok(())
    }

    fn send_line(&mut self, data: &[u8]) -> Result<()> {
        let data = data.as_ref();
        self.process_handle
            .stdin
            .as_mut()
            .unwrap()
            .write_all(data)
            .context("Failed to send to process")?;
        self.process_handle
            .stdin
            .as_mut()
            .unwrap()
            .write_all(b"\n")
            .context("Failed to send newline")?;

        Ok(())
    }

    fn recv(&mut self, num_bytes: usize) -> Result<Vec<u8>> {
        // create a new vector that holds up to num_bytes
        let mut temp = Vec::new();
        temp.resize(num_bytes, 0);

        let read_size = self
            .stdout_reader
            .read(&mut temp)
            .context("Failed to read from process")?;

        temp.resize(read_size, 0);
        Ok(temp)
    }

    fn recv_until(&mut self, terminator: &[u8]) -> Result<Vec<u8>> {
        // temporary buffer
        let mut temp_data: Vec<u8> = Vec::new();

        loop {
            // access the internal bufreader buffer and append that to our temporary buffer
            let internal_buf = self.stdout_reader.fill_buf()?;
            let internal_buf_len = internal_buf.len();
            let prev_internal_buf_len = temp_data.len();
            temp_data.extend(internal_buf);

            // if our temporary buffer already contains the iterator, consume the bytes we are about to return
            // exclude the bytes we already consumed in previous iterations
            if let Some(pos) = temp_data
                .windows(terminator.len())
                .position(|x| x == terminator)
            {
                temp_data.resize(pos + terminator.len(), 0);
                self.stdout_reader
                    .consume(pos + terminator.len() - prev_internal_buf_len);
                return Ok(temp_data);
            } else {
                // terminator not found yet, consume everything we read
                self.stdout_reader.consume(internal_buf_len);
            }
        }
    }

    fn attach_debugger(&self) -> Result<()> {
        Command::new("gnome-terminal")
            .args(&["--", "gdb", "-p", &self.process_handle.id().to_string()])
            .spawn()
            .context("Couldn't spawn debugger")?;
        std::thread::sleep(std::time::Duration::from_millis(2000));
        Ok(())
    }
}

impl Drop for LocalIO {
    fn drop(&mut self) {
        self.process_handle.kill().expect("Failed killing process");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_send_recv() {
        let mut local_io = LocalIO::new("cat", &[]).expect("Failed to create LocalIO object");

        // send test data
        local_io.send(b"Test_Str_123?").expect("send() failed");

        // receive single byte and check
        assert_eq!(local_io.recv(1).expect("recv() failed"), b"T");

        // receive multiple bytes and check
        assert_eq!(local_io.recv(3).expect("recv() failed"), b"est");

        // receive remaining bytes and check
        assert_eq!(local_io.recv(100).expect("recv() failed"), b"_Str_123?");
    }

    #[test]
    fn test_sendline_recv() {
        let mut local_io = LocalIO::new("cat", &[]).expect("Failed to create LocalIO object");

        // send test data
        local_io
            .send_line(b"Test_Str_123?")
            .expect("send_line() failed");
        local_io
            .send_line(b"Test_Str_456?")
            .expect("send_line() failed");

        // receive data and check
        assert_eq!(
            local_io.recv(100).expect("recv() failed"),
            b"Test_Str_123?\nTest_Str_456?\n"
        );
    }

    #[test]
    fn test_send_recvline() {
        let mut local_io = LocalIO::new("cat", &[]).expect("Failed to create LocalIO object");

        // send test data
        local_io
            .send(b"Test_Str_123?\nTest_Str_456?\n")
            .expect("send() failed");

        // receive line and check
        assert_eq!(
            local_io.recv_line().expect("recv_line() failed"),
            b"Test_Str_123?\n"
        );

        // receive line and check
        assert_eq!(
            local_io.recv_line().expect("recv_line() failed"),
            b"Test_Str_456?\n"
        );
    }

    #[test]
    fn test_sendline_recvline() {
        let mut local_io = LocalIO::new("cat", &[]).expect("Failed to create LocalIO object");

        // send test data
        local_io
            .send_line(b"Test_Str_123?")
            .expect("send_line() failed");
        local_io
            .send_line(b"Test_Str_456?")
            .expect("send_line() failed");

        // receive line and check
        assert_eq!(
            local_io.recv_line().expect("recv_line() failed"),
            b"Test_Str_123?\n"
        );

        // receive line and check
        assert_eq!(
            local_io.recv_line().expect("recv_line() failed"),
            b"Test_Str_456?\n"
        );
    }

    #[test]
    fn test_send_recvuntil() {
        let mut local_io = LocalIO::new("cat", &[]).expect("Failed to create LocalIO object");

        // send test data
        local_io
            .send(b"Test_Str_123?Test_Str_456?")
            .expect("send_line() failed");

        // receive until and check
        assert_eq!(
            local_io.recv_until(b"?").expect("recv_until() failed"),
            b"Test_Str_123?"
        );

        // receive until and check
        assert_eq!(
            local_io.recv_until(b"?").expect("recv_until() failed"),
            b"Test_Str_456?"
        );
    }
}
