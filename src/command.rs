use crate::misc::cyclic::{cyclic, cyclic_find, de_bruijn_string};
use crate::recipe::{CategoryView, IngredientView};
use crate::utils::State;
use log::*;
use regex::bytes::Regex;
use serde::{Deserialize, Serialize};

use crate::binary_handling::{self};
use crate::lang::Ast;

use anyhow::{anyhow, bail, Context, Result};

pub type CmdResult = Result<Option<Vec<u8>>>;
pub trait Command {
    fn execute(&self, state: &mut State) -> CmdResult;
    fn category() -> CommandCategory
    where
        Self: Sized;
    fn has_input() -> bool
    where
        Self: Sized;
    fn has_output() -> bool
    where
        Self: Sized;
    fn cmd_type() -> CommandType
    where
        Self: Sized;
    fn description() -> String
    where
        Self: Sized;
    fn title() -> String
    where
        Self: Sized;
    fn from_parameter(param: &[u8], state: &State) -> Self
    where
        Self: Sized;
}

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum CommandCategory {
    IO,
    Binary,
    Misc,
    Custom,
}

impl CommandCategory {
    pub fn title(self) -> String {
        match self {
            CommandCategory::IO => "IO".to_string(),
            CommandCategory::Binary => "Binary".to_string(),
            CommandCategory::Misc => "Misc".to_string(),
            CommandCategory::Custom => "Custom".to_string(),
        }
    }
}

macro_rules! simple_cmd {
    ($title:literal, $desc:literal, cat: $cat:ident, input: $input:expr, output: $output:expr, $name:ident => |$self:ident, $state:ident| $body:tt) => {
        pub struct $name {
            msg: Vec<u8>,
        }

        impl Command for $name {
            fn cmd_type() -> CommandType {
                CommandType::$name
            }

            fn execute(&$self, $state: &mut State) -> CmdResult {
                $body
            }

            fn category() -> CommandCategory {
                CommandCategory::$cat
            }

            fn has_input() -> bool {
                $input
            }

            fn has_output() -> bool {
                $output
            }

            fn description() -> String {
                $desc.to_string()
            }

            fn title() -> String {
                $title.to_string()
            }

            fn from_parameter(param: &[u8], state: &State) -> Self where Self: Sized {
                let mut msg = param.to_owned();

                let re = Regex::new(r"\{(.*?)\}").expect("failed to create regex.");
                let mut msg_str = msg.clone();
                while re.is_match(&msg_str) {
                    if let Some(expr) = re.find(&msg_str) {
                        let expr_start = expr.start();
                        let expr_end = expr.end();

                        let evaluated = {
                            let ast = &msg[expr_start+1..expr_end-1];
                            let evaluated = Ast::new(&String::from_utf8(ast.to_vec()).expect("Invalid utf8")).expect("Cannot parse as AST").get_result(state).expect("Cannot evaluate AST");
                            [&msg[0..expr_start], &evaluated, &msg[expr_end..]].concat()
                        };

                        msg = evaluated.clone();
                        msg_str = msg.clone();
                    }
                }

                $name {
                    msg
                }
            }
        }
    }
}

macro_rules! command_switch {
    ($command_type:ident: $($cmd:literal => $cls:ident,)*) => {
        pub fn parse_command(cmd_str: &str, param: &[u8], state: &State) -> Result<Box<dyn Command>> {
            let cmd = match cmd_str {
                $(
                    $cmd => Some(Box::new(<$cls>::from_parameter(param, state)) as Box<dyn Command>),
                )*
                _ => None
            };
            cmd.ok_or(anyhow!("Can't parse command"))
        }

        #[derive(Clone, Copy, Serialize, Deserialize, Debug)]
        pub enum $command_type {
            $($cls,)* Custom
        }

        pub fn create_command(cmd_type: CommandType, input: &[u8], state: &State) -> Box<dyn Command>{
            match cmd_type {
                $(
                    CommandType::$cls => Box::new(<$cls>::from_parameter(input, state)) as Box<dyn Command>,
                )*
                    CommandType::Custom => Box::new(CustomIngredient::from_parameter(input, state)) as Box<dyn Command>,
            }
        }

        pub fn available_categories() -> Vec<CategoryView> {
            let mut cat_io = CategoryView::new(CommandCategory::IO);
            let mut cat_binary = CategoryView::new(CommandCategory::Binary);
            let mut cat_misc = CategoryView::new(CommandCategory::Misc);
            let mut cat_custom = CategoryView::new(CommandCategory::Custom);
            for ingredient in available_ingredients() {
                match ingredient.category {
                    CommandCategory::IO => {
                        cat_io.push(ingredient);
                    }
                    CommandCategory::Misc => {
                        cat_misc.push(ingredient);
                    }
                    CommandCategory::Binary => {
                        cat_binary.push(ingredient);
                    }
                    CommandCategory::Custom => {
                        cat_custom.push(ingredient);
                    }
                }
            }

            vec![cat_io, cat_binary, cat_misc, cat_custom]
        }
        pub fn available_ingredients() -> Vec<IngredientView> {
            vec![
            $(
                IngredientView::new::<$cls>(),
            )*
            ]
        }

    }
}

simple_cmd!("Send", "Sends data to the process.", cat: IO, input: true, output: false, SendCmd => |self, state| {
        state
            .program
            .send(&self.msg)
            .context("Could not send to process.")?;
        Ok(None)
    }
);

simple_cmd!("Send Line", "Sends data with an appended Newline to the process.", cat: IO, input: true, output: false, SendLineCmd => |self, state| {
        state
            .program
            .send_line(&self.msg)
            .context("Could not send line to process.")?;
        Ok(None)
    }
);

simple_cmd!("Receive", "Receive data from the process.", cat: IO, input: true, output: true, RecvCmd => |self, state| {
        let read_size = if self.msg.is_empty() {
            4096
        } else {
            String::from_utf8(self.msg.clone())?.parse::<usize>()?
        };

        let received = state.program.recv(read_size).context("Could not read from process")?;
        state.output += &String::from_utf8_lossy(&received);
        Ok(Some(received))
    }
);

simple_cmd!("Receive Until", "Receive data from the process until a certain sequence is found.", cat: IO, input: true, output: true, RecvUntil => |self, state| {
        let received = state.program.recv_until(&self.msg).context("Could not read from process")?;
        state.output += &String::from_utf8(received.clone()).context("Invalid utf8")?;
        Ok(Some(received))
    }
);

simple_cmd!("Receive Line", "Receives a single line from the process.", cat: IO, input: false, output: true, RecvLineCmd => |self, state| {
        let received = state.program.recv_line().context("Could not read from process")?;
        state.output += &String::from_utf8(received.clone()).context("Invalid utf8")?;
        Ok(Some(received))
    }
);

simple_cmd!("Attach Debugger", "Attaches a debugger to the running process.", cat: Binary, input: false, output: false, AttachDbg => |self, state| {
        state.program.attach_debugger()?;
        Ok(None)
    }
);

simple_cmd!("Log", "Logs a message", cat: Misc, input: true, output: true, LogCmd => |self, state| {
    debug!("{}", String::from_utf8(self.msg.clone()).context("Invalid utf8")?);
    Ok(Some(self.msg.to_vec()))
});

simple_cmd!("Regex", "Parse register content using regex. Syntax: register@regex", cat: Misc, input: true, output: true, RegexCmd => |self, state| {
    // register@regex

    let as_str = String::from_utf8(self.msg.clone()).context("Invalid utf8")?;
    let split = as_str.split_once("@").context("Malformed Regex Cmd")?;
    let register = state.registers.get(&split.0).context("Invalid Register in Regex Cmd")?;
    let regex = split.1;

    let re = Regex::new(regex).context("Malformed Regex")?;

    if let Some(cpts) = re.captures(register) {
        let result = cpts.get(1).context("No group captured")?.as_bytes();
        return Ok(Some(result.to_vec()));
    }

    bail!("Could not capture anything.");
});

simple_cmd!("Send Padding", "Sends x amount of A", cat: IO, input: true, output: false, SendPaddingCmd => |self, state| {
    let nr: usize = String::from_utf8(self.msg.clone()).context("invalid utf8")?.parse().context("Unable to parse nr")?;
    let repeated_a = "A".repeat(nr);
    state
        .program
        .send(repeated_a.as_bytes())
        .context("Could not send to process.")?;
    Ok(None)
});

simple_cmd!("Log Registers", "Logs all available registers", cat: Misc, input: false, output: false, LogRegCmd => |self, state| {
    let strings: Vec<String> = state.registers.map.iter().map(|(key, value)| format!("{}: {:?}\n", key, value)).collect();
    debug!("{}", strings.join(""));
    Ok(None)
});

simple_cmd!("Get Symbol Address", "Gets address of a symbol", cat: Binary, input: true, output: true, GetSymAddrCmd => |self, state| {
    let binary = binary_handling::from_path(&state.program_path)?;
    Ok(Some(format!("{}", binary.get_sym_addr(&String::from_utf8(self.msg.clone())?)?).into_bytes()))
});

simple_cmd!("Pack Address", "Packs address into bytestring", cat: Misc, input: true, output: true, StringToAddrCmd => |self, state| {
    let address = u32::from_str_radix(&String::from_utf8(self.msg.clone())?, 10).expect("failed decoding string");
    Ok(Some(address.to_ne_bytes().to_vec()))
});

simple_cmd!("Generate Cyclic Sequence", "Generate cyclic sequence with substring size 4 and given length", cat: Misc, input: true, output: true, CyclicCmd => |self, state| {
    let len: usize = String::from_utf8(self.msg.clone())?.parse().context("Unable to parse len")?;
    Some(cyclic(len, 4)).transpose()
});

simple_cmd!("Find Cyclic Substring", "Calculates the position of a substring", cat: Misc, input: true, output: true, CyclicFindCmd => |self, state| {
    let substring= u32::from_str_radix(&String::from_utf8(self.msg.clone())?, 16)?.to_ne_bytes();

    let position = cyclic_find(&substring, 4);
    if let Some(pos) = position {
        let bytes = pos.to_string().as_bytes().to_vec();
        return Ok(Some(bytes));
    }
    Ok(None)
});

pub struct CustomIngredient {
    path: String,
}

impl Command for CustomIngredient {
    fn cmd_type() -> CommandType {
        CommandType::Custom
    }

    fn execute(&self, state: &mut State) -> CmdResult {
        let path = format!("ingredients/{}", self.path);
        let data = std::fs::read_to_string(&path).expect("Unable to read file");
        let deserialized: Vec<IngredientView> = serde_json::from_str(&data).unwrap();
        for ingredient in deserialized {
            ingredient.run(state)?;
        }

        Ok(None)
    }

    fn category() -> CommandCategory {
        CommandCategory::Custom
    }

    fn has_input() -> bool {
        false
    }

    fn has_output() -> bool {
        false
    }

    fn description() -> String {
        "".to_string()
    }

    fn title() -> String {
        "Custom".to_string()
    }
    fn from_parameter(param: &[u8], state: &State) -> Self {
        CustomIngredient {
            path: String::from_utf8(param.to_vec()).unwrap(),
        }
    }
}

command_switch!(CommandType:
    "send" => SendCmd,
    "sendln" => SendLineCmd,
    "recv" => RecvCmd,
    "recvuntil" => RecvUntil,
    "recvline" => RecvLineCmd,
    "sendpad" => SendPaddingCmd,
    "attach_debugger" => AttachDbg,
    "get_symbol_address" => GetSymAddrCmd,
    "log" => LogCmd,
    "regex" => RegexCmd,
    "logregs" => LogRegCmd,
    "string_to_address" => StringToAddrCmd,
    "cyclic" => CyclicCmd,
    "cyclicfind" => CyclicFindCmd,
);
