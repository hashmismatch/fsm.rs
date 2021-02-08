extern crate finny;

use finny::{FsmCurrentState, FsmError, FsmEvent, FsmEventQueueVec, FsmFactory, FsmResult, FsmTimersNull, decl::{BuiltFsm, FsmBuilder}, finny_fsm, inspect::slog::InspectSlog};
use slog::{Drain, Logger, info, o};

#[derive(Debug)]
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
#[derive(Clone, Debug)]
pub struct EventClick { time: usize }
#[derive(Clone, Debug)]
pub struct EventEnter { shift: bool }

#[finny_fsm]
fn build_fsm(mut fsm: FsmBuilder<StateMachine, StateMachineContext>) -> BuiltFsm {
    fsm.events_debug();
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
        .guard(|ev, _, states| {
            let state_a: &StateA = states.as_ref();
            ev.time > 100 && state_a.enter == 1
        })
        .action(|ev, ctx, _state_from, _state_to| {
            ctx.context.total_time += ev.time;
        });

    fsm.state::<StateB>()
        .on_entry(|state_b, _| {
            state_b.counter += 1;
        })
        .on_event::<EventEnter>()
        .internal_transition()
        .guard(|ev, _, _| {
            ev.shift == false
        })
        .action(|_, _, state_b| {
            state_b.counter += 1;
        });

    fsm.build()
}


#[test]
fn test_fsm() -> FsmResult<()> {
    let plain = slog_term::PlainSyncDecorator::new(std::io::stdout());
    let logger = Logger::root(
        slog_term::FullFormat::new(plain)
        .build().fuse(), o!()
    );

    let ctx = StateMachineContext { count: 0, total_time: 0 };
    
    let mut fsm = StateMachine::new_with(ctx, FsmEventQueueVec::new(), InspectSlog::new(Some(logger)), FsmTimersNull)?;
    
    let current_state = fsm.get_current_states()[0];
    let state: &StateA = fsm.get_state();
    assert_eq!(0, state.enter);
    assert_eq!(FsmCurrentState::Stopped, current_state);
    assert_eq!(0, fsm.get_context().count);

    fsm.start()?;

    assert_eq!(FsmCurrentState::State(StateMachineCurrentState::StateA), fsm.get_current_states()[0]);
    assert_eq!(1, fsm.get_context().count);
    let state: &StateA = fsm.get_state();
    assert_eq!(1, state.enter);

    let ret = fsm.dispatch(EventClick { time: 99 });
    assert_eq!(Err(FsmError::NoTransition), ret);
    
    fsm.dispatch(EventClick { time: 123 })?;

    assert_eq!(2, fsm.get_context().count);
    assert_eq!(123, fsm.get_context().total_time);

    let state_b: &StateB = fsm.get_state();
    assert_eq!(1, state_b.counter);

    let ret = fsm.dispatch(EventEnter { shift: true });
    assert_eq!(Err(FsmError::NoTransition), ret);
    
    fsm.dispatch(EventEnter { shift: false })?;
    let state_b: &StateB = fsm.get_state();
    assert_eq!(2, state_b.counter);
    
    Ok(())
}