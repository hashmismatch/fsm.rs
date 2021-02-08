#![no_std]
#![no_main]

use finny::{finny_fsm, FsmFactory, FsmEventQueueArray, InspectNull, FsmTimersNull};
use finny::decl::{FsmBuilder, BuiltFsm};
use heapless::consts::*;

#[no_mangle]
pub extern "C" fn main(_argc: isize, _argv: *const *const u8) -> isize {
    // Since we are passing a C string the final null character is mandatory
    const HELLO: &'static str = "Hello, world!\n\0";
    unsafe {
        libc::printf(HELLO.as_ptr() as *const _);
    }

    {
        let ctx = StateMachineContext::default();
        let queue = FsmEventQueueArray::<_, [_; 16]>::new(); 
        let inspect = InspectNull::new();
        let timers = FsmTimersNull;
        let mut fsm = StateMachine::new_with(ctx, queue, inspect, timers).unwrap();
        fsm.start().unwrap();
    }

    0
}

#[panic_handler]
fn my_panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}


///////////////////////////////////////////////////

#[derive(Debug, Default)]
pub struct StateMachineContext {
    count: usize,
    total_time: usize
}

#[derive(Default)]
pub struct StateA {
    enter: usize,
    exit: usize
}
#[derive(Default)]
pub struct StateB {
    counter: usize
}
#[derive(Default)]
pub struct StateC;

#[derive(Clone)]
pub struct EventClick { time: usize }
#[derive(Clone)]
pub struct EventEnter { shift: bool }

#[finny_fsm]
fn build_fsm(mut fsm: FsmBuilder<StateMachine, StateMachineContext>) -> BuiltFsm {
    fsm.initial_state::<StateA>();

    fsm.state::<StateA>()
        .on_entry(|state_a, ctx| {
            ctx.context.count += 1;
            state_a.enter += 1;
        })
        .on_exit(|state_a, ctx| {
            ctx.context.count += 1;
            state_a.exit += 1;
        })
        .on_event::<EventClick>()
        .transition_to::<StateB>()
        .guard(|ev, ctx, _| {
            ev.time > 100
        })
        .action(|ev, ctx, state_from, state_to| {
            ctx.context.total_time += ev.time;
        });

    fsm.state::<StateB>()
        .on_entry(|state_b, ctx| {
            state_b.counter += 1;
        })
        .on_event::<EventEnter>()
        .internal_transition()
        .guard(|ev, ctx, _| {
            ev.shift == false
        })
        .action(|ev, ctx, state_b| {
            state_b.counter += 1;
        });

    fsm.build()
}