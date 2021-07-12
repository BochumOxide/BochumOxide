use anyhow::{bail, Context, Result};

/// trait that must be implemented for all kind of binary format handlers
pub trait Binary {
    fn get_sym_addr(&self, sym: &str) -> Result<u64>;
}

#[cfg(feature = "unicorn")]
mod elf;

#[cfg(feature = "unicorn")]
mod pe;

#[cfg(not(feature = "unicorn"))]
mod without_unicorn {
    use super::*;
    pub fn from_path(path: &str) -> Result<Box<dyn Binary>> {
        bail!("Activate the 'uni' feature to get access to binary parsing")
    }
}

#[cfg(feature = "unicorn")]
mod with_unicorn {
    use super::*;

    pub use elf::ELFBinary;
    pub use pe::PEBinary;

    pub fn from_path(path: &str) -> Result<Box<dyn Binary>> {
        if let Ok(pe) = PEBinary::new(path) {
            return Ok(Box::new(pe));
        }

        let elf = ELFBinary::new(path);
        Ok(Box::new(elf.context(
            "Illegal binary type or running in network mode",
        )?))
    }
}

#[cfg(feature = "unicorn")]
pub use with_unicorn::from_path;

#[cfg(not(feature = "unicorn"))]
pub use without_unicorn::from_path;
