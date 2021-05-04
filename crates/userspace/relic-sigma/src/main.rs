//! Relis OS - Sigma space: OS behavior process.

#![cfg_attr(target_os = "none", no_std)]
#![cfg_attr(not(test), no_main)]
#![feature(prelude_import)]

use relic_abi::bootstrap::{BootstrapInfo, FrameBufferInfo};

extern crate relic_std as std;

#[allow(unused_imports)]
#[prelude_import]
use relic_std::prelude::*;

#[macro_use]
extern crate log;

pub mod graphics;

#[no_mangle]
pub fn user_main(bootstrap_info: &BootstrapInfo) {
    load_graphics(&bootstrap_info.fb_info);
    info!("Welcome to Relic OS!");
}

fn load_graphics(fb_info: &FrameBufferInfo) {
    // Load graphics
    let display;
    unsafe {
        let fb_raw = fb_info.frame_buffer_vaddr as *mut u32;
        assert!(
            fb_info.frame_buffer_scanline == fb_info.frame_buffer_width * 4,
            "Scanline must be the same size as width * 4. Not implemented the non equal scenario."
        );
        let fb = core::slice::from_raw_parts_mut(
            fb_raw,
            (fb_info.frame_buffer_height * fb_info.frame_buffer_width) as usize,
        );
        display = graphics::fb::FrameBrufferDisplay::new(
            fb,
            fb_info.frame_buffer_width as u16,
            fb_info.frame_buffer_height as u16,
        );
    }

    info!("Initializing the UI");
    let terminal = tui::Terminal::new(display).unwrap();
    info!("Terminal created");
    graphics::gui::initialize(terminal);
    info!("Turining on GUI");

    log::set_logger(&graphics::gui::GuiLogger)
        .map(|()| log::set_max_level(log::LevelFilter::Trace))
        .unwrap();
}
