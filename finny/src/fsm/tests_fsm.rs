//! A minimal, internal FSM for unit tests, manually written.

use crate::{AllVariants, FsmBackend, FsmCurrentState, FsmStates};
use derive_more::From;

#[derive(Default)]
pub struct StateA;
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct EventA { pub n: usize }

pub struct TestFsm;

#[derive(Default)]
pub struct States {
    state_a: StateA
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum StateKind {
    StateA
}

impl FsmStates<TestFsm> for States {
    type StateKind = StateKind;
    type CurrentState = [FsmCurrentState<StateKind>; 1];
}

#[derive(Debug, Copy, Clone, PartialEq, From)]
pub enum Events {
    EventA(EventA)
}

impl AsRef<str> for Events {
    fn as_ref(&self) -> &'static str {
        todo!()
    }
}
#[derive(Debug, Clone, PartialEq)]
pub enum FsmBackendTimers {

}

impl AllVariants for FsmBackendTimers {
    type Iter = std::iter::Once<FsmBackendTimers>;

    fn iter() -> Self::Iter {
        todo!()
    }
}


impl FsmBackend for TestFsm {
    type Context = ();
    type States = States;
    type Events = Events;
    type Timers = FsmBackendTimers;

    fn dispatch_event<Q, I, T>(_ctx: crate::DispatchContext<Self, Q, I, T>, _event: crate::FsmEvent<Self::Events, Self::Timers>) -> crate::FsmDispatchResult
        where Q: crate::FsmEventQueue<Self>,
            I: crate::Inspect, T: crate::FsmTimers<Self>
     {
        todo!()
    }
}