use anyhow::Result;

mod local_io;
mod network_io;

// make sure that LocalIO can be imported using crate::program_io::LocalIO
// otherwise we would need to import it using the "full path" to the type
pub use local_io::LocalIO;
pub use network_io::NetworkIO;

/// trait that must be implemented for all kind of I/O
pub trait ProgramIO {
    /// send bytes to the stream
    fn send(&mut self, data: &[u8]) -> Result<()>;
    /// send bytes and additional newline to the stream
    fn send_line(&mut self, data: &[u8]) -> Result<()>;

    /// receive up to num_bytes of data and return as soon as any data is read
    fn recv(&mut self, num_bytes: usize) -> Result<Vec<u8>>;
    /// receive until terminator is read
    fn recv_until(&mut self, terminator: &[u8]) -> Result<Vec<u8>>;
    /// receive until newline is found
    fn recv_line(&mut self) -> Result<Vec<u8>> {
        self.recv_until(b"\n")
    }
    /// attach a debugger to the process (only works for localio)
    fn attach_debugger(&self) -> Result<()>;

    fn restart(&mut self) -> Result<()> {
        Ok(())
    }
}
