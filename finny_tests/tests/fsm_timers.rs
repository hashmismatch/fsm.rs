extern crate finny;

use std::{thread::{sleep, sleep_ms}, time::Duration};

use finny::{FsmCurrentState, FsmEvent, FsmEventQueueVec, FsmFactory, FsmResult, decl::{BuiltFsm, FsmBuilder}, finny_fsm, inspect::slog::InspectSlog, timers::std::{TimersStd}};
use slog::{Drain, Logger, info, o};

#[derive(Debug)]
pub struct TimersMachineContext {
    exit_a: bool
}

#[derive(Default)]
pub struct StateA {
    timers: usize
}
#[derive(Default)]
pub struct StateB {

}
#[derive(Default)]
pub struct StateC;
#[derive(Clone, Debug)]
pub struct EventClick;
#[derive(Clone, Debug)]
pub struct EventTimer { n: usize }

#[derive(Clone, Debug)]
pub struct EventEnter { shift: bool }

#[finny_fsm]
fn build_fsm(mut fsm: FsmBuilder<TimersMachine, TimersMachineContext>) -> BuiltFsm {
    fsm.events_debug();
    fsm.initial_state::<StateA>();

    fsm.state::<StateA>();

    fsm.state::<StateA>()
        .on_exit(|state, ctx| {
            ctx.exit_a = true;
        })
        .on_event::<EventClick>()
        .transition_to::<StateB>()
        .guard(|ev, ctx, states| {
            let state: &StateA = states.as_ref();
            state.timers >= 5
        });

    fsm.state::<StateA>()
        .on_event::<EventTimer>()
        .internal_transition()
        .action(|ev, ctx, state| {
            state.timers += 1;
        });

    fsm.state::<StateA>()
        .on_entry_start_timer(|_ctx, timer| {
            timer.timeout = Duration::from_millis(50);
            timer.renew = true;
            timer.cancel_on_state_exit = true;
        }, |ctx, state| {
            Some( EventTimer {n: 0}.into() )
        });

    fsm.state::<StateA>()
        .on_entry_start_timer(|_ctx, timer| {
            timer.timeout = Duration::from_millis(100);
            timer.renew = false;
            timer.cancel_on_state_exit = true;
        }, |ctx, state| {
            Some( EventTimer {n: 1}.into() )
        });

    fsm.state::<StateB>();

    fsm.build()
}


#[test]
fn test_timers_fsm() -> FsmResult<()> {
    let decorator = slog_term::TermDecorator::new().build();
    let drain = slog_term::CompactFormat::new(decorator).build().fuse();
    let drain = std::sync::Mutex::new(drain).fuse();
    let logger = slog::Logger::root(drain, o!());
    
    let ctx = TimersMachineContext { exit_a: false };
    
    let mut fsm = TimersMachine::new_with(ctx, FsmEventQueueVec::new(), InspectSlog::new(Some(logger)), TimersStd::new())?;
    
    fsm.start()?;
    
    sleep(Duration::from_millis(225));

    fsm.dispatch_timer_events()?;

    let state_a: &StateA = fsm.get_state();
    assert_eq!(5, state_a.timers);
    fsm.dispatch(EventClick)?;

    sleep(Duration::from_millis(100));

    fsm.dispatch_timer_events()?;    

    assert_eq!(FsmCurrentState::State(TimersMachineCurrentState::StateB), fsm.get_current_states()[0]);

    let state_a: &StateA = fsm.get_state();
    assert_eq!(5, state_a.timers);
    assert_eq!(true, fsm.exit_a);

    Ok(())
}