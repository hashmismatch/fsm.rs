#![feature(proc_macro)]

#[macro_use]
extern crate fsm;
#[macro_use]
extern crate fsm_codegen;

extern crate serde;
#[macro_use]
extern crate serde_derive;

extern crate fsm_timers_std;

use fsm::*;
use fsm::console_inspect::*;

use fsm_timers_std::*;


use fsm_codegen::fsm_fn;

#[fsm_fn]
fn create_it() -> () {
    let fsm = FsmDecl::new_fsm::<Lights>()
        .initial_state::<LightOff>();

    fsm.new_unit_event::<LightOffTimedOut>();
    fsm.new_unit_event::<LightOnTimedOut>();

    fsm.new_unit_state::<LightOff>();
    fsm.new_unit_state::<LightOn>();

    fsm.new_state_timeout::<LightOff, LightOffTimedOut>(|ctx| {
        Some(TimerSettings {
            timeout: TimerDuration::from_millis(500),
            cancel_on_state_exit: true,
            event_on_timeout: LightOffTimedOut
        })
    });

    fsm.new_state_timeout::<LightOn, LightOnTimedOut>(|ctx| {
        Some(TimerSettings {
            timeout: TimerDuration::from_millis(100),
            cancel_on_state_exit: true,
            event_on_timeout: LightOnTimedOut
        })
    });


    fsm.on_event::<LightOffTimedOut>()
        .transition_from::<LightOff>()
        .to::<LightOn>();

    fsm.on_event::<LightOnTimedOut>()
        .transition_from::<LightOn>()
        .to::<LightOff>();
}


fn main() {
    use std::time::*;

    let mut lights = Lights::new_custom((), FsmInspectStdOut, FsmTimersStd::new()).unwrap();
    lights.start();

    let run_time = Duration::from_secs(3);
    let started_at = Instant::now();

    loop {        
        let events = {
            let mut timers = lights.get_timers_mut();
            timers.receive_events()
        };

        if events.len() > 0 {
            //println!("events: {:#?}", events);

            for event in events {
                lights.process_timer_event(&event);
            }
        }

        std::thread::sleep_ms(50);

        if started_at.elapsed() > run_time {
            println!("example finished.");
            return;
        }
    }
}

