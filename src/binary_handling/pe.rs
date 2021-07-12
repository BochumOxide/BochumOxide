use super::Binary;
use anyhow::{bail, Context, Result};
use goblin::{pe::export::Reexport::*, Object};
use std::collections::HashMap;
use std::fs;

pub struct PEBinary {
    /// raw bytes of the parsed ELF
    pub raw_bytes: Vec<u8>,
    /// import address table
    pub iat: HashMap<String, u64>,
    /// export address table
    pub eat: HashMap<String, u64>,
    /// symbol name to address map (iat and eat combined make up the symbols for PE binaries)
    pub symbols: HashMap<String, u64>,
}

impl PEBinary {
    pub fn new(path: &str) -> Result<Self> {
        // read in PE binary given a path
        let path = which::which(path)?;
        let raw_bytes = fs::read(path)?;

        // parse required information from the PE
        let iat = Self::parse_iat(&raw_bytes)?;
        let eat = Self::parse_eat(&raw_bytes)?;

        // there is no special symbol section such as for elf binaries
        // therefore, symbols are just the iat and eat entries combined
        // when combinding iat and eat entries for the final symbols, eat entries overwrite iat entries
        let mut symbols = HashMap::new();

        // process eat symbols
        for (name, addr) in &eat {
            let mut eat_symbol_name = "eat.".to_string();
            eat_symbol_name.push_str(name);

            // insert eat.name symbol and name symbol
            symbols.insert(eat_symbol_name, *addr);
            symbols.insert(name.to_owned(), *addr);
        }

        // process iat symbols
        for (name, addr) in &iat {
            let mut iat_symbol_name = "iat.".to_string();
            iat_symbol_name.push_str(name);

            // insert iat.name symbol
            symbols.insert(iat_symbol_name, *addr);

            // do not overwrite already existing symbols
            if symbols.contains_key(name) {
                continue;
            }

            // only if no symbol with that name from eat has been inserted, insert it without prefix
            symbols.insert(name.to_owned(), *addr);
        }

        Ok(PEBinary {
            raw_bytes,
            iat,
            eat,
            symbols,
        })
    }

    /// parse iat of pe binary
    fn parse_iat(raw_data: &[u8]) -> Result<HashMap<String, u64>> {
        // parse in raw bytes as PE binary
        let pe = match Object::parse(raw_data).context("Failed to parse raw data")? {
            Object::PE(elf) => elf,
            _ => bail!("No valid PE"),
        };

        // collect import name and offset to thunk ptr into hashmap
        let iat = pe
            .imports
            .iter()
            .map(|x| (x.name.to_string(), x.offset as u64))
            .collect::<HashMap<_, _>>();

        Ok(iat)
    }

    /// parse eat of pe binary
    fn parse_eat(raw_data: &[u8]) -> Result<HashMap<String, u64>> {
        // parse in raw bytes as PE binary
        let pe = match Object::parse(raw_data).context("Failed to parse raw data")? {
            Object::PE(elf) => elf,
            _ => bail!("No valid PE"),
        };

        // collect export name and rva into hashmap
        let eat = pe
            .exports
            .iter()
            .map(|x| {
                let name = match x.name {
                    // check if eat entry has a name
                    Some(x) => x.to_string(),
                    _ => match x.reexport {
                        // if entry is exported by ordinal, create an artifical name
                        Some(DLLOrdinal { ordinal, lib }) => {
                            format!("ordinal_{}", ordinal).to_string()
                        }
                        _ => bail!("Could not parse export entry"),
                    },
                };
                Ok((name, x.rva as u64))
            })
            .collect::<Result<HashMap<_, _>>>()?;

        Ok(eat)
    }
}

impl Binary for PEBinary {
    /// given a symbol name, retrieve the address
    fn get_sym_addr(&self, sym: &str) -> Result<u64> {
        let sym = self.symbols.get(sym).context("Symbol not found")?;
        Ok(*sym)
    }
}

#[cfg(test)]
mod tests {
    use super::PEBinary;

    #[test]
    fn test_iat_parser() {
        // start 32-bit tests
        let pe = PEBinary::new("test_data/kernel32_32.dll").unwrap();
        assert_eq!(*pe.iat.get("SearchPathA").unwrap(), 0x814c0);
        assert_eq!(*pe.iat.get("EnumDateFormatsW").unwrap(), 0x8131c);
        assert_eq!(*pe.iat.get("GetConsoleCommandHistoryA").unwrap(), 0x80e60);

        // start 64-bit tests
        let pe = PEBinary::new("test_data/kernel32_64.dll").unwrap();
        assert_eq!(*pe.iat.get("IdnToAscii").unwrap(), 0x81678);
        assert_eq!(*pe.iat.get("IsValidNLSVersion").unwrap(), 0x81740);
        assert_eq!(*pe.iat.get("CreateNamedPipeW").unwrap(), 0x81948);
    }

    #[test]
    fn test_eat_parser() {
        // start 32-bit tests
        let pe = PEBinary::new("test_data/kernel32_32.dll").unwrap();
        assert_eq!(*pe.eat.get("Module32NextW").unwrap(), 0x5a8c0);
        assert_eq!(*pe.eat.get("GetAtomNameA").unwrap(), 0x52020);
        assert_eq!(*pe.eat.get("PssFreeSnapshot").unwrap(), 0x341c0);

        // start 64-bit tests
        let pe = PEBinary::new("test_data/kernel32_64.dll").unwrap();
        assert_eq!(*pe.eat.get("PssWalkMarkerGetPosition").unwrap(), 0x3acd0);
        assert_eq!(*pe.eat.get("SetCurrentDirectoryA").unwrap(), 0x3b6e0);
        assert_eq!(*pe.eat.get("SetNamedPipeHandleState").unwrap(), 0x21f40);
    }

    #[test]
    fn test_symbol_parser() {
        // start 32-bit tests
        let pe = PEBinary::new("test_data/kernel32_32.dll").unwrap();
        assert_eq!(*pe.symbols.get("Module32NextW").unwrap(), 0x5a8c0);
        assert_eq!(*pe.symbols.get("iat.SearchPathA").unwrap(), 0x814c0);
        assert_eq!(*pe.symbols.get("eat.PssFreeSnapshot").unwrap(), 0x341c0);

        // start 64-bit tests
        let pe = PEBinary::new("test_data/kernel32_64.dll").unwrap();
        assert_eq!(
            *pe.symbols.get("PssWalkMarkerGetPosition").unwrap(),
            0x3acd0
        );
        assert_eq!(*pe.symbols.get("iat.IsValidNLSVersion").unwrap(), 0x81740);
        assert_eq!(
            *pe.symbols.get("eat.SetCurrentDirectoryA").unwrap(),
            0x3b6e0
        );
    }
}
