use anyhow::{bail, Context, Result};

use std::io::{Read, Write};
use std::net::{Shutdown, TcpStream};
use std::time::Duration;

use super::ProgramIO;

pub struct NetworkIO {
    stream: TcpStream,
}

impl NetworkIO {
    /// connection must be of form ip:port
    pub fn new(connection: &str) -> Result<Self> {
        // create a TCP stream with the given connection parameter
        let stream = TcpStream::connect(connection)
            .context(format!("Failed to open connection to {}", connection))?;

        stream
            .set_read_timeout(Some(Duration::new(5, 0)))
            .expect("failed to set read timeout for TCP connection");
        stream
            .set_write_timeout(Some(Duration::new(5, 0)))
            .expect("failed to set read timeout for TCP connection");

        Ok(NetworkIO { stream })
    }
}

impl ProgramIO for NetworkIO {
    fn send(&mut self, data: &[u8]) -> Result<()> {
        self.stream
            .write_all(data.as_ref())
            .context("Failed to send to process")?;

        Ok(())
    }

    fn send_line(&mut self, data: &[u8]) -> Result<()> {
        let data = data.as_ref();
        self.stream
            .write_all(data)
            .context("Failed to send to process")?;
        self.stream
            .write_all(b"\n")
            .context("Failed to send newline")?;

        Ok(())
    }

    fn recv(&mut self, num_bytes: usize) -> Result<Vec<u8>> {
        // create a new vector that holds up to num_bytes
        let mut x = Vec::new();
        x.resize(num_bytes, 0);

        // read up to num_bytes many bytes
        let read_size = self
            .stream
            .read(&mut x)
            .context("Failed to read from process")?;
        // cut of unwritten bytes
        x.resize(read_size, 0);

        Ok(x)
    }

    fn recv_until(&mut self, terminator: &[u8]) -> Result<Vec<u8>> {
        // temporary buffer
        let mut temp: Vec<u8> = Vec::new();
        temp.resize(4096, 0);

        loop {
            // read data from the stream without removing it
            let read_size = self.stream.peek(&mut temp)?;

            // if peeked data contains terminator, read bytes from the stream up to and including the terminator
            if let Some(pos) = temp[0..read_size]
                .windows(terminator.len())
                .position(|x| x == terminator)
            {
                return self.recv(pos + terminator.len());
            }
        }
    }

    fn attach_debugger(&self) -> Result<()> {
        bail!("Not implemented")
    }
}

impl Drop for NetworkIO {
    fn drop(&mut self) {
        // close connection on drop
        self.stream
            .shutdown(Shutdown::Both)
            .expect("Failed to shutdown TCP stream");
    }
}

#[cfg(test)]
mod tests {
    use std::net::{SocketAddr, TcpListener};
    use std::thread;

    use super::*;

    fn echo_server(mut stream: TcpStream) {
        // read incoming data into 50 byte blocks at maxx
        let mut data = [0 as u8; 50];

        while match stream.read(&mut data) {
            Ok(size) => {
                // send back the received data
                stream.write_all(&data[0..size]).unwrap();
                true
            }
            Err(_) => {
                println!("Failed to read data from the stream");
                stream.shutdown(Shutdown::Both).unwrap();
                false
            }
        } {}
    }

    fn setup_server() -> SocketAddr {
        // listen on 127.0.0.1 with OS chosen port
        let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to set up listener");
        let local_addr = listener
            .local_addr()
            .expect("Failed to unwrap local address");

        // accept incoming connections in another thread so that we do not block here
        thread::spawn(move || {
            let (stream, _) = listener
                .accept()
                .expect("Failed to accept incoming connection");
            thread::spawn(move || echo_server(stream));
        });

        // return local address so that the caller knows the OS chosen port
        local_addr
    }

    #[test]
    fn test_send_recv() {
        // spawn the tcp echo server
        let local_addr = setup_server();
        let test_data = b"AAAA";

        // open connection to the server and send test data
        let mut network_io =
            NetworkIO::new(&local_addr.to_string()).expect("Failed to create NetworkIO object");
        network_io.send(test_data).expect("send() failed");

        // first read a single char
        let recv_data = network_io.recv(1).expect("recv() failed");
        assert_eq!(recv_data, b"A");

        // read remaining chars
        let recv_data = network_io.recv(100).expect("recv() failed");

        // read may read up to 3 more bytes which all should be b'A'
        assert!(recv_data.len() <= 3);
        assert!(recv_data.iter().all(|&x| x == b'A'));
    }

    #[test]
    fn test_send_recvline() {
        // spawn the tcp echo server
        let local_addr = setup_server();
        let test_data = b"test_data\n";

        // open connection to the server and send two lines
        let mut network_io =
            NetworkIO::new(&local_addr.to_string()).expect("Failed to create NetworkIO object");
        network_io.send(test_data).expect("send() failed");
        network_io.send(test_data).expect("send() failed");

        // we should be able to receive two lines containing b"test_data\n"
        assert_eq!(
            network_io.recv_line().expect("recv_line() failed"),
            test_data
        );
        assert_eq!(
            network_io.recv_line().expect("recv_line() failed"),
            test_data
        );
    }

    #[test]
    fn test_sendline_recvline() {
        // spawn the tcp echo server
        let local_addr = setup_server();
        let test_data = b"test_data\n";

        // open connection to the server and call send_line without passing the \n
        let mut network_io =
            NetworkIO::new(&local_addr.to_string()).expect("Failed to create NetworkIO object");
        network_io
            .send_line(&test_data[0..(test_data.len() - 1)])
            .expect("send_line() failed");

        // we should be able to receive a single line containing the newline character
        assert_eq!(
            network_io.recv_line().expect("recv_line() failed"),
            test_data
        );
    }

    #[test]
    fn test_send_recvuntil() {
        // spawn the tcp echo server
        let local_addr = setup_server();
        let test_data = b"ABCDEFGHIJKLMNOP";

        // open connection to the server and send test data
        let mut network_io =
            NetworkIO::new(&local_addr.to_string()).expect("Failed to create NetworkIO object");
        network_io.send(test_data).expect("send() failed");

        // receive until specified data and check if data matches
        assert_eq!(
            network_io.recv_until(b"FGH").expect("recv_until() failed"),
            b"ABCDEFGH"
        );
        assert_eq!(
            network_io.recv_until(b"O").expect("recv_until() failed"),
            b"IJKLMNO"
        );
        assert_eq!(
            network_io.recv_until(b"P").expect("recv_until() failed"),
            b"P"
        );
    }

    #[test]
    #[should_panic]
    fn test_send_recvuntil_empty_terminator() {
        // spawn the tcp echo server
        let local_addr = setup_server();
        let test_data = b"test_data";

        // open connection to the server and send test data
        let mut network_io =
            NetworkIO::new(&local_addr.to_string()).expect("Failed to create NetworkIO object");
        network_io.send(test_data).expect("send() failed");

        // this call should panic because empty terminators are not valid
        network_io.recv_until(b"").expect("recv_until() failed");
    }
}
