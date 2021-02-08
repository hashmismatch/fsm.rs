extern crate finny;

use std::{thread::{sleep, sleep_ms}, time::Duration};

use finny::{FsmCurrentState, FsmEvent, FsmEventQueueVec, FsmFactory, FsmResult, decl::{BuiltFsm, FsmBuilder}, finny_fsm, inspect::slog::InspectSlog, timers::std::{TimersStd}, AllVariants};
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
    fsm.initial_states::<(StateA, BlinkerMachine)>();
    fsm.sub_machine::<BlinkerMachine>();

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
            timer.timeout = Duration::from_millis(100);
            timer.renew = true;
            timer.cancel_on_state_exit = true;
        }, |ctx, state| {
            Some( EventTimer {n: 0}.into() )
        })
        .with_timer_ty::<Timer1>();

    fsm.state::<StateA>()
        .on_entry_start_timer(|_ctx, timer| {
            timer.timeout = Duration::from_millis(200);
            timer.renew = false;
            timer.cancel_on_state_exit = true;
        }, |ctx, state| {
            Some( EventTimer {n: 1}.into() )
        })
        .with_timer_ty::<Timer2>();

    fsm.state::<StateB>();

    fsm.build()
}

#[derive(Default, Debug)]
pub struct LightOn;
#[derive(Default, Debug)]
pub struct LightOff;
#[derive(Default, Debug)]
pub struct BlinkingOn;
#[derive(Default, Clone, Debug)]
pub struct BlinkToggle;
#[derive(Default)]
pub struct BlinkerContext {
    toggles: usize
}

#[finny_fsm]
fn build_blinker_fsm(mut fsm: FsmBuilder<BlinkerMachine, BlinkerContext>) -> BuiltFsm {
    fsm.events_debug();
    fsm.initial_states::<(LightOff, BlinkingOn)>();

    fsm.state::<LightOff>()
        .on_event::<BlinkToggle>()
        .transition_to::<LightOn>()
        .action(|_, ctx, _, _| {
            ctx.toggles += 1;
        });

    fsm.state::<LightOn>()
        .on_event::<BlinkToggle>()
        .transition_to::<LightOff>()
        .action(|_, ctx, _, _| {
            ctx.toggles += 1;
        });

    fsm.state::<BlinkingOn>()
        .on_entry_start_timer(|ctx, settings| {
            settings.timeout = Duration::from_millis(100);
            settings.renew = true;
        }, |ctx, state| {
            Some( BlinkToggle.into() )
        })
        .with_timer_ty::<BlinkingTimer>();

    fsm.build()
}



#[test]
fn test_timers_fsm() -> FsmResult<()> {
    let decorator = slog_term::TermDecorator::new().build();
    let drain = slog_term::CompactFormat::new(decorator).build().fuse();
    let drain = slog_async::Async::new(drain).build().fuse();
    let logger = slog::Logger::root(drain, o!());
    
    let ctx = TimersMachineContext { exit_a: false };
    

    let sub_timers_variants: Vec<_> = BlinkerMachineTimers::iter().collect();
    assert_eq!(&[BlinkerMachineTimers::BlinkingTimer], sub_timers_variants.as_slice());

    let timers_variants: Vec<_> = TimersMachineTimers::iter().collect();
    assert_eq!(&[TimersMachineTimers::Timer1, TimersMachineTimers::Timer2, TimersMachineTimers::BlinkerMachine(BlinkerMachineTimers::BlinkingTimer)], timers_variants.as_slice());

    let mut fsm = TimersMachine::new_with(ctx, FsmEventQueueVec::new(), InspectSlog::new(Some(logger)), TimersStd::new())?;
    
    fsm.start()?;
    
    sleep(Duration::from_millis(450));

    fsm.dispatch_timer_events()?;

    let state_a: &StateA = fsm.get_state();
    assert_eq!(5, state_a.timers);
    fsm.dispatch(EventClick)?;

    sleep(Duration::from_millis(200));

    fsm.dispatch_timer_events()?;    

    assert_eq!(FsmCurrentState::State(TimersMachineCurrentState::StateB), fsm.get_current_states()[0]);

    let state_a: &StateA = fsm.get_state();
    assert_eq!(5, state_a.timers);
    assert_eq!(true, fsm.exit_a);

    let sub_machine: &BlinkerMachine = fsm.get_state();
    assert_eq!(6, sub_machine.toggles);

    Ok(())
}