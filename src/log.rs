use log::*;
use simplelog::*;

use std::fs::File;

pub fn init_logger() {
    let config = ConfigBuilder::new()
        .set_time_level(LevelFilter::Off)
        .set_target_level(LevelFilter::Off)
        .set_max_level(LevelFilter::Trace)
        .set_thread_level(LevelFilter::Trace)
        .add_filter_allow_str("BochumOxide")
        .build();

    let _ = WriteLogger::init(
        LevelFilter::Trace,
        config,
        File::create(r"log.log").unwrap(),
    );
}
