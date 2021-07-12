#![allow(dead_code)]
#![allow(non_fmt_panic)]
#![allow(non_snake_case)]
#![allow(unused_imports)]
#![allow(unused_variables)]

use crate::gui::App;
use anyhow::Context;
use anyhow::Result;
use iced::Application;
use iced::Settings;

mod binary_handling;
mod command;
mod gui;
mod lang;
mod log;
mod misc;
mod program_io;
mod recipe;
mod utils;

fn main() -> Result<()> {
    crate::log::init_logger();
    App::run(Settings::default()).context("Failed to launch gui")
}
