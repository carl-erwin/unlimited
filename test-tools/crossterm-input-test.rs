use std::io::{stdout, Write};

use crossterm::{
    cursor::{DisableBlinking, EnableBlinking, Hide, MoveTo, RestorePosition, SavePosition, Show},
    event,
    event::{DisableMouseCapture, EnableMouseCapture},
    queue,
    style::Styler,
    style::{
        Attribute, Color, Print, ResetColor, SetAttribute, SetBackgroundColor, SetForegroundColor,
    },
    terminal::{Clear, ClearType},
    ExecutableCommand, Result,
};

use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen},
};

use std::time::Duration;
use std::time::Instant;

fn main() {
    crossterm::terminal::enable_raw_mode();
    execute!(
        stdout(),
        EnterAlternateScreen,
        EnableMouseCapture,
        Hide,
        SetAttribute(Attribute::Reset)
    );

    println!("crossterm input test");
    print!("\r");

    loop {
        get_input_events();
    }
}

fn get_input_events() {
    println!("enter read\r");

    let mut accum = Vec::with_capacity(4096 * 4);
    let sleep_val: u64 = 16;

    let start = Instant::now();

    loop {
        if ::crossterm::event::poll(Duration::from_millis(sleep_val)).unwrap_or_default() {
            // It's guaranteed that the `read()` won't block when the `poll()`
            // function returns `true`
            let cross_evt = ::crossterm::event::read().ok().unwrap();

            accum.push(cross_evt);

            if start.elapsed() > Duration::from_millis(16) {
                println!("max time to acuum\r");
                break;
            }
        } else {
            println!("timeout\r");

            if !accum.is_empty() {
                // flush
                print!("\r");
                break;
            }
        }
    }

    println!("flush : accum len = {}", accum.len());

    println!("exit read\r");
}
