use anyhow::{anyhow, bail, Context, Result};
use goblin::{
    container::Ctx,
    elf::{
        dynamic::DT_PLTGOT,
        header::{EM_386, EM_AARCH64, EM_ARM, EM_X86_64},
        program_header::PT_INTERP,
        reloc::RelocSection,
        section_header::{SectionHeader, SHN_UNDEF, SHT_DYNSYM, SHT_REL, SHT_RELA, SHT_SYMTAB},
        sym::Symtab,
    },
    strtab::Strtab,
    Object,
};
use std::fs;
use std::rc::Rc;
use std::{cell::Cell, collections::HashMap};
use unicorn::{
    unicorn_const::{Arch, Mode, Permission, SECOND_SCALE},
    RegisterX86,
};

use super::Binary;

#[derive(Debug)]
pub struct ELFBinary {
    /// raw bytes of the parsed ELF
    pub raw_bytes: Vec<u8>,
    /// global offset table
    pub got: HashMap<String, u64>,
    /// procedure linkage table
    pub plt: HashMap<String, u64>,
    /// symbol name to address map
    pub symbols: HashMap<String, u64>,
}

impl ELFBinary {
    pub fn new(path: &str) -> Result<Self> {
        // read in ELF binary given a path
        let path = which::which(path)?;
        let raw_bytes = fs::read(path)?;

        // parse required information from the elf
        let got = Self::parse_got(&raw_bytes).context("Failed to populate got")?;
        let plt = Self::parse_plt(&raw_bytes, &got).context("Failed to populate plt")?;
        let symbols =
            Self::parse_symbols(&raw_bytes, &plt, &got).context("Failed to populate symbols")?;

        Ok(ELFBinary {
            raw_bytes,
            got,
            plt,
            symbols,
        })
    }

    /// get goblin Ctx struct based on elf binary
    fn get_binary_ctx(raw_data: &[u8]) -> Result<Ctx> {
        // parse in raw bytes as ELF binary
        let elf = match Object::parse(raw_data).context("Failed to parse raw data")? {
            Object::Elf(elf) => elf,
            _ => bail!("No valid ELF"),
        };

        Ok(Ctx {
            container: if elf.is_64 {
                goblin::container::Container::Big
            } else {
                goblin::container::Container::Little
            },
            le: if elf.little_endian {
                goblin::container::Endian::Little
            } else {
                goblin::container::Endian::Big
            },
        })
    }

    /// get SectionHeader by name
    fn get_section_by_name(raw_data: &[u8], name: &str) -> Result<Option<SectionHeader>> {
        // parse in raw bytes as ELF binary
        let elf = match Object::parse(raw_data).context("Failed to parse raw data")? {
            Object::Elf(elf) => elf,
            _ => bail!("No valid ELF"),
        };

        // iterate over all sections
        for section in elf.section_headers.iter() {
            if let Some(section_name) = elf.shdr_strtab.get(section.sh_name) {
                if section_name? != name {
                    continue;
                }
                return Ok(Some(section.to_owned()));
            }
        }
        Ok(None)
    }

    /// parse got of elf binary
    fn parse_got(raw_data: &[u8]) -> Result<HashMap<String, u64>> {
        // parse in raw bytes as ELF binary
        let elf = match Object::parse(raw_data).context("Failed to parse raw data")? {
            Object::Elf(elf) => elf,
            _ => bail!("No valid ELF"),
        };

        let mut got_symbols = HashMap::new();

        // if binary is statically linked, do not attempt to parse got
        if elf.program_headers.iter().all(|x| x.p_type != PT_INTERP) {
            return Ok(HashMap::new());
        }

        for section in elf.section_headers.iter() {
            // ignore non-relocation sections and relocations that do not link to another section
            if (section.sh_type != SHT_REL && section.sh_type != SHT_RELA)
                || section.sh_link == SHN_UNDEF
            {
                continue;
            }

            // get symbol section
            let symbol_section = elf
                .section_headers
                .get(section.sh_link as usize)
                .context("Failed to get symbol section")?;
            let symbols = Symtab::parse(
                raw_data,
                symbol_section.sh_offset as usize,
                (symbol_section.sh_size / symbol_section.sh_entsize) as usize,
                Self::get_binary_ctx(raw_data)?,
            )?;

            // get symbol string section
            let symbol_string_section = elf
                .section_headers
                .get(symbol_section.sh_link as usize)
                .context("Failed to get symbol string section")?;
            let symbol_strings = Strtab::parse(
                raw_data,
                symbol_string_section.sh_offset as usize,
                symbol_string_section.sh_size as usize,
                b'\0',
            )?;

            // get relocation section
            let ctx = Self::get_binary_ctx(raw_data)?;
            let reloc_section = RelocSection::parse(
                raw_data,
                section.sh_offset as usize,
                section.sh_size as usize,
                section.sh_type == SHT_RELA,
                ctx,
            )
            .context("Failed to get reloc section")?;

            // iterate over all relocations
            for reloc in reloc_section.iter() {
                // filter out invalid symbol indices
                let sym_idx = reloc.r_sym;
                if sym_idx == 0 {
                    continue;
                }

                // lookup name and store in hashmap together with address
                if let Some(symbol) = symbols.get(sym_idx) {
                    if let Some(symbol_name) = symbol_strings.get(symbol.st_name) {
                        let symbol_name = symbol_name.context("Failed to get symbol string")?;

                        got_symbols.insert(symbol_name.to_string(), reloc.r_offset);
                    }
                }
            }
        }

        Ok(got_symbols)
    }

    /// emulate instructions in a plt section and trace memory accesses
    fn emulate_plt_instructions(
        raw_data: &[u8],
        got: u64,
        plt_section_address: u64,
        plt_section_data: &[u8],
    ) -> Result<Vec<(u64, u64)>> {
        // parse in raw bytes as ELF binary
        let elf = match Object::parse(raw_data).context("Failed to parse raw data")? {
            Object::Elf(elf) => elf,
            _ => bail!("No valid ELF"),
        };

        // we only support arm (32/64 bit) and x86 (32/64 bit) binaries
        match elf.header.e_machine {
            EM_ARM | EM_AARCH64 | EM_386 | EM_X86_64 => (),
            _ => bail!("Unsupported architecture"),
        };

        // create unicorn emulator instance
        let mut unicorn = if elf.header.e_machine == EM_ARM || elf.header.e_machine == EM_AARCH64 {
            unicorn::Unicorn::new(
                if elf.is_64 { Arch::ARM64 } else { Arch::ARM },
                if elf.little_endian {
                    Mode::LITTLE_ENDIAN
                } else {
                    Mode::BIG_ENDIAN
                },
                0,
            )
        } else {
            unicorn::Unicorn::new(
                Arch::X86,
                if elf.is_64 {
                    Mode::MODE_64
                } else {
                    Mode::MODE_32
                },
                0,
            )
        }
        .map_err(|_err| anyhow!("Failed to create unicorn instance"))?;

        // map section data we are going to execute ceiled to page size
        let mut emu = unicorn.borrow();
        let mem_start = plt_section_address & (!0xfff);
        let mem_end = (plt_section_address + plt_section_data.len() as u64 + 0xfff) & !0xfff;

        // map page as RX page (code)
        emu.mem_map(
            mem_start,
            (mem_end - mem_start) as usize,
            Permission::READ | Permission::EXEC,
        )
        .map_err(|_err| anyhow!("Failed to allocate executable memory"))?;
        // write section data at correct offset (note that it is written to plt_section_address and not the start of the page)
        emu.mem_write(plt_section_address, plt_section_data)
            .map_err(|_err| anyhow!("Failed to write to executable memory"))?;

        // when executing we are going to fault whenever memory (usually in the got) is accessed
        // we want to save the address to create a link between the plt address and the got entry later on
        let mem_fault_addr = Rc::new(Cell::new(None));
        let mem_fault_addr_clone = mem_fault_addr.clone();

        // hook that is called on every access to unmapped memory and every read access in general
        let hook = move |uc: unicorn::UnicornHandle<u8>,
                         access: unicorn::unicorn_const::MemType,
                         addr: u64,
                         size: usize,
                         value: i64| {
            mem_fault_addr_clone.set(Some(addr));
        };

        // add the hook closure to the emulation
        emu.add_mem_hook(
            unicorn::unicorn_const::HookType::MEM_UNMAPPED,
            0,
            std::u64::MAX,
            hook.clone(),
        )
        .map_err(|_err| anyhow!("Failed to add hook"))?;
        emu.add_mem_hook(
            unicorn::unicorn_const::HookType::MEM_READ,
            0,
            std::u64::MAX,
            hook.clone(),
        )
        .map_err(|_err| anyhow!("Failed to add hook"))?;

        // save context so it can be quickly restored after each execution attempt
        let mut saved_ctx = emu
            .context_init()
            .map_err(|_err| anyhow!("Failed to allocate context"))?;
        emu.context_save(&mut saved_ctx)
            .map_err(|_err| anyhow!("Failed to save context"))?;

        // (plt, got) vector, where plt is the address of the plt stub and got the address which the stub resolves/calls
        let mut plt_got_addresses: Vec<(u64, u64)> = vec![];

        // assumption is that each plt stub is 4-byte aligned
        for begin in 0..(plt_section_data.len() as u64 / 4) {
            // restore to the clean context and restore the faulting address
            emu.context_restore(&saved_ctx)
                .map_err(|_err| anyhow!("Failed to restore context"))?;
            mem_fault_addr.set(None);

            // when dealing with 32-bit binaries the plt stub expects got address in ebx
            if !elf.is_64 && elf.header.e_machine != EM_ARM {
                emu.reg_write(RegisterX86::EBX as i32, got)
                    .map_err(|_err| anyhow!("Failed to write to EBX"))?;
            }

            // start the emulation
            let starting_address = plt_section_address + begin * 4;
            let _ = emu.emu_start(
                starting_address,
                mem_end,
                1 * SECOND_SCALE,
                (mem_end - mem_start) as usize,
            );

            // save the plt address where we started execution and the faulting memory access (maybe got entry)
            if let Some(addr) = mem_fault_addr.get() {
                // ignore null pointer accesses
                if addr != 0 {
                    plt_got_addresses.push((starting_address, addr));
                }
            }
        }

        Ok(plt_got_addresses)
    }

    /// parse plt of elf binary
    fn parse_plt(raw_data: &[u8], got: &HashMap<String, u64>) -> Result<HashMap<String, u64>> {
        // parse in raw bytes as ELF binary
        let elf = match Object::parse(raw_data).context("Failed to parse raw data")? {
            Object::Elf(elf) => elf,
            _ => bail!("No valid ELF"),
        };

        // symbol to plt map
        let mut plt_symbols = HashMap::new();

        // if binary is statically linked, do not attempt to parse got
        if elf.program_headers.iter().all(|x| x.p_type != PT_INTERP) {
            return Ok(plt_symbols);
        }

        // search all plt sections
        let sections = [
            Self::get_section_by_name(raw_data, ".plt").context("Failed to get section")?,
            Self::get_section_by_name(raw_data, ".plt.got").context("Failed to get section")?,
            Self::get_section_by_name(raw_data, ".plt.sec").context("Failed to get section")?,
        ];

        // got section address which might be required by plt code for 32-bit binaries
        let dt_pltgot = elf
            .dynamic
            .context("Failed to get dynamic linking information")?
            .dyns
            .iter()
            .find(|x| x.d_tag == DT_PLTGOT)
            .context("Failed to get DT_PLTGOT information")?
            .d_val;

        // got entry address to symbol name hashmap
        // we want to find all plt entries that reference one of these got entries
        let got_targets = got
            .iter()
            .map(|x| (*x.1, x.0.to_owned()))
            .collect::<HashMap<_, _>>();

        // try emulation for all possible plt sections
        for section in sections.iter() {
            if let Some(section) = section {
                // get vector of all referenced addresses by possible plt entries
                let plt_got_addresses = Self::emulate_plt_instructions(
                    raw_data,
                    dt_pltgot,
                    section.sh_addr,
                    &raw_data[section.sh_offset as usize
                        ..section.sh_offset as usize + section.sh_size as usize],
                )?;

                // now whenever a target got entry was referenced, assume that we found a valid plt entry
                for (plt_addr, got_addr) in plt_got_addresses {
                    if let Some(key) = got_targets.get(&got_addr) {
                        plt_symbols.insert(key.to_owned(), plt_addr);
                    }
                }
            }
        }

        Ok(plt_symbols)
    }

    /// parse all symbols of elf binary
    fn parse_symbols(
        raw_data: &[u8],
        plt: &HashMap<String, u64>,
        got: &HashMap<String, u64>,
    ) -> Result<HashMap<String, u64>> {
        // parse in raw bytes as ELF binary
        let elf = match Object::parse(raw_data).context("Failed to parse raw data")? {
            Object::Elf(elf) => elf,
            _ => bail!("No valid ELF"),
        };

        let mut symbols = HashMap::new();

        // first, populate all normal symbols (ignore symbols that have zero value)
        for section in elf.section_headers.iter() {
            // ignore non-symbol sections
            // note that goblin is missing a type here: SHT_SUNW_LDYNSYM which is 0x6ffffff3
            if section.sh_type != SHT_SYMTAB
                && section.sh_type != SHT_DYNSYM
                && section.sh_type != 0x6fff_fff3
            {
                continue;
            }

            let symtab = Symtab::parse(
                raw_data,
                section.sh_offset as usize,
                (section.sh_size / section.sh_entsize) as usize,
                Self::get_binary_ctx(raw_data)?,
            )?;

            // get symbol string section
            let symbol_string_section = elf
                .section_headers
                .get(section.sh_link as usize)
                .context("Failed to get symbol string section")?;

            let symbol_strings = Strtab::parse(
                raw_data,
                symbol_string_section.sh_offset as usize,
                symbol_string_section.sh_size as usize,
                b'\0',
            )?;

            symbols.extend(
                symtab
                    .iter()
                    .filter(|x| x.st_value != 0)
                    .map(|x| {
                        let symbol = symbol_strings
                            .get(x.st_name)
                            .context("Strtab entry not found")?
                            .context("Failed to get Strtab entry")?;
                        Ok((symbol.to_string(), x.st_value))
                    })
                    .collect::<Result<HashMap<_, _>>>()?,
            );
        }

        // process plt symbols
        for (name, addr) in plt {
            let mut plt_symbol_name = "plt.".to_string();
            plt_symbol_name.push_str(name);

            // insert plt.name symbol
            symbols.insert(plt_symbol_name, *addr);

            // do not overwrite already existing symbols
            if symbols.contains_key(name) {
                continue;
            }

            symbols.insert(name.to_owned(), *addr);
        }

        // process got symbols
        for (name, addr) in got {
            let mut got_symbol_name = "got.".to_string();
            got_symbol_name.push_str(name);

            // insert got.name symbol
            symbols.insert(got_symbol_name, *addr);

            // do not overwrite already existing symbols
            if symbols.contains_key(name) {
                continue;
            }

            symbols.insert(name.to_owned(), *addr);
        }

        Ok(symbols)
    }
}

impl Binary for ELFBinary {
    /// given a symbol name, retrieve the address
    fn get_sym_addr(&self, sym: &str) -> Result<u64> {
        let sym = self.symbols.get(sym).context("Symbol not found")?;
        Ok(*sym)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_got_parser() {
        // start 32-bit tests

        // x86
        let bin = ELFBinary::new("test_data/bin32").unwrap();
        assert_eq!(*bin.got.get("_ITM_deregisterTMCloneTable").unwrap(), 0x1fec);
        assert_eq!(*bin.got.get("__libc_start_main").unwrap(), 0x1fe8);
        assert_eq!(*bin.got.get("puts").unwrap(), 0x1fe4);
        assert!(bin.got.get("nonexistingentry").is_none());

        let bin = ELFBinary::new("test_data/libc-2.27-32.so").unwrap();
        assert_eq!(*bin.got.get("_IO_stdin_").unwrap(), 0x1d8ed4);
        assert_eq!(*bin.got.get("calloc").unwrap(), 0x1d8030);
        assert_eq!(*bin.got.get("__libc_stack_end").unwrap(), 0x1d7ea4);
        assert!(bin.got.get("nonexistingentry").is_none());

        // arm
        let bin = ELFBinary::new("test_data/bin_arm32").unwrap();
        assert_eq!(*bin.got.get("puts").unwrap(), 0x2100c);
        assert_eq!(*bin.got.get("__libc_start_main").unwrap(), 0x21010);
        assert_eq!(*bin.got.get("abort").unwrap(), 0x21018);
        assert!(bin.got.get("nonexistingentry").is_none());

        // start 64-bit tests

        // x86
        let bin = ELFBinary::new("test_data/bin64").unwrap();
        assert_eq!(
            *bin.got.get("_ITM_deregisterTMCloneTable").unwrap(),
            0x200fd8
        );
        assert_eq!(*bin.got.get("__libc_start_main").unwrap(), 0x200fe0);
        assert_eq!(*bin.got.get("puts").unwrap(), 0x200fd0);
        assert!(bin.got.get("nonexistingentry").is_none());

        let bin = ELFBinary::new("test_data/libc-2.27-64.so").unwrap();
        assert_eq!(*bin.got.get("free").unwrap(), 0x3eaf98);
        assert_eq!(*bin.got.get("calloc").unwrap(), 0x3eb118);
        assert_eq!(*bin.got.get("_IO_funlockfile").unwrap(), 0x3eaf60);
        assert!(bin.got.get("nonexistingentry").is_none());

        // arm
        let bin = ELFBinary::new("test_data/bin_arm64").unwrap();
        assert_eq!(*bin.got.get("puts").unwrap(), 0x10fb8);
        assert_eq!(*bin.got.get("__libc_start_main").unwrap(), 0x10fa0);
        assert_eq!(*bin.got.get("abort").unwrap(), 0x10fb0);
        assert!(bin.got.get("nonexistingentry").is_none());
    }

    #[test]
    fn test_plt_parser() {
        // start 32-bit tests

        // x86
        let bin = ELFBinary::new("test_data/bin32").unwrap();
        assert_eq!(*bin.plt.get("__libc_start_main").unwrap(), 0x3c0);
        assert_eq!(*bin.plt.get("puts").unwrap(), 0x3b0);
        assert!(bin.plt.get("nonexistingentry").is_none());

        let bin = ELFBinary::new("test_data/libc-2.27-32.so").unwrap();
        assert_eq!(*bin.plt.get("calloc").unwrap(), 0x185e0);
        assert_eq!(*bin.plt.get("free").unwrap(), 0x18608);
        assert_eq!(*bin.plt.get("memalign").unwrap(), 0x18580);
        assert!(bin.plt.get("nonexistingentry").is_none());

        // arm
        let bin = ELFBinary::new("test_data/bin_arm32").unwrap();
        assert_eq!(*bin.plt.get("puts").unwrap(), 0x102dc);
        assert_eq!(*bin.plt.get("__libc_start_main").unwrap(), 0x102e8);
        assert_eq!(*bin.plt.get("abort").unwrap(), 0x10300);
        assert!(bin.plt.get("nonexistingentry").is_none());

        // start 64-bit tests

        // x86
        let bin = ELFBinary::new("test_data/bin64").unwrap();
        assert_eq!(*bin.plt.get("__cxa_finalize").unwrap(), 0x520);
        assert_eq!(*bin.plt.get("puts").unwrap(), 0x510);
        assert!(bin.plt.get("nonexistingentry").is_none());

        let bin = ELFBinary::new("test_data/libc-2.27-64.so").unwrap();
        assert_eq!(*bin.plt.get("free").unwrap(), 0x212c8);
        assert_eq!(*bin.plt.get("calloc").unwrap(), 0x211e0);
        assert_eq!(*bin.plt.get("__tunable_get_val").unwrap(), 0x210f0);
        assert!(bin.plt.get("nonexistingentry").is_none());

        // arm
        let bin = ELFBinary::new("test_data/bin_arm64").unwrap();
        assert_eq!(*bin.plt.get("puts").unwrap(), 0x610);
        assert_eq!(*bin.plt.get("__libc_start_main").unwrap(), 0x5e0);
        assert_eq!(*bin.plt.get("abort").unwrap(), 0x600);
    }

    #[test]
    fn test_symbol_parser() {
        // start 32-bit tests

        // x86
        let bin = ELFBinary::new("test_data/bin32").unwrap();
        assert_eq!(
            *bin.symbols.get("_ITM_deregisterTMCloneTable").unwrap(),
            0x1fec
        );
        assert_eq!(*bin.symbols.get("__init_array_start").unwrap(), 0x1ed8);
        assert_eq!(*bin.symbols.get("got.__cxa_finalize").unwrap(), 0x1ff0);
        assert!(bin.symbols.get("nonexistingentry").is_none());

        let bin = ELFBinary::new("test_data/libc-2.27-32.so").unwrap();
        assert_eq!(*bin.symbols.get("semget").unwrap(), 0xfad40);
        assert_eq!(*bin.symbols.get("random").unwrap(), 0x31080);
        assert_eq!(*bin.symbols.get("plt.___tls_get_addr").unwrap(), 0x185f0);
        assert!(bin.symbols.get("nonexistingentry").is_none());

        // arm
        let bin = ELFBinary::new("test_data/bin_arm32").unwrap();
        assert_eq!(*bin.symbols.get("_edata").unwrap(), 0x21028);
        assert_eq!(*bin.symbols.get("__libc_csu_fini").unwrap(), 0x10478);
        assert_eq!(*bin.symbols.get("got.__gmon_start__").unwrap(), 0x21014);
        assert!(bin.symbols.get("nonexistingentry").is_none());

        // start 64-bit tests

        // x86
        let bin = ELFBinary::new("test_data/bin64").unwrap();
        assert_eq!(*bin.symbols.get("__init_array_end").unwrap(), 0x200dc0);
        assert_eq!(*bin.symbols.get("main").unwrap(), 0x63a);
        assert_eq!(*bin.symbols.get("plt.__cxa_finalize").unwrap(), 0x520);
        assert!(bin.symbols.get("nonexistingentry").is_none());

        let bin = ELFBinary::new("test_data/libc-2.27-64.so").unwrap();
        assert_eq!(*bin.symbols.get("strtod_l").unwrap(), 0x4c080);
        assert_eq!(*bin.symbols.get("__res_randomid").unwrap(), 0x145b80);
        assert_eq!(*bin.symbols.get("got.stdout").unwrap(), 0x3eaf40);
        assert!(bin.symbols.get("nonexistingentry").is_none());

        // arm
        let bin = ELFBinary::new("test_data/bin_arm64").unwrap();
        assert_eq!(*bin.symbols.get("__init_array_end").unwrap(), 0x10d88);
        assert_eq!(*bin.symbols.get("_DYNAMIC").unwrap(), 0x10d90);
        assert_eq!(*bin.symbols.get("got.abort").unwrap(), 0x10fb0);
        assert!(bin.symbols.get("nonexistingentry").is_none());
    }
}
