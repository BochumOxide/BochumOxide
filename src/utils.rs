use crate::{command, program_io::*};

use anyhow::anyhow;
use anyhow::{Context, Result};

use std::collections::HashMap;

pub struct State {
    pub program: Box<dyn ProgramIO>,
    pub program_path: String,
    pub do_exit: bool,
    pub registers: Registers,
    pub output: String,
}

pub enum Target {
    Local,
    Network,
}

impl State {
    pub fn new(target_type: Target, target: &str, args: &[&str]) -> Result<Self> {
        match target_type {
            Target::Local => {
                let state = State {
                    program: Box::new(
                        LocalIO::new(target, args).context("Failed to spawn program")?,
                    ),
                    program_path: target.to_string(),
                    registers: Registers::new(),
                    do_exit: false,
                    output: String::new(),
                };
                Ok(state)
            }
            Target::Network => {
                let state = State {
                    program: Box::new(NetworkIO::new(target).context("Failed to spawn program")?),
                    program_path: "No binary path in network mode".to_string(),
                    registers: Registers::new(),
                    do_exit: false,
                    output: String::new(),
                };
                Ok(state)
            }
        }
    }
}

#[derive(Debug)]
pub struct Registers {
    pub map: HashMap<String, Vec<u8>>,
}

impl Registers {
    pub fn new() -> Registers {
        Registers {
            map: HashMap::new(),
        }
    }

    pub fn set(&mut self, name: &str, val: Vec<u8>) {
        self.map.insert(name.to_owned(), val);
    }

    pub fn get(&self, name: &str) -> Option<&[u8]> {
        let vec = self.map.get(name).map(|l| l.as_slice());
        vec
    }
    pub fn exists(&self, name: &str) -> bool {
        self.map.contains_key(name)
    }

    pub fn available_registers(&self) -> Vec<String> {
        self.map.keys().cloned().collect()
    }
}

pub fn print_registers(regs: &Registers) {
    println!("{:?}", regs);
}
