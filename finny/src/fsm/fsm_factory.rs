use crate::{FsmBackend, FsmBackendImpl, FsmEventQueue, FsmFrontend, FsmResult, FsmTimers, FsmTimersNull, Inspect, InspectNull, timers::std::TimersStd};

#[cfg(feature="std")]
use crate::FsmEventQueueVec;

/// Builds a frontend for running your FSM.
pub trait FsmFactory {
    type Fsm: FsmBackend;

    /// For submachines, for use with codegen.
    fn new_submachine_backend(backend: FsmBackendImpl<Self::Fsm>) -> FsmResult<Self> where Self: Sized;

    /// Build a new frontend for the FSM with all the environmental services provided by the caller.
    fn new_with<Q, I, T>(context: <Self::Fsm as FsmBackend>::Context, queue: Q, inspect: I, timers: T) -> FsmResult<FsmFrontend<Self::Fsm, Q, I, T>>
        where Q: FsmEventQueue<Self::Fsm>, I: Inspect, T: FsmTimers<Self::Fsm>
    {
        let frontend = FsmFrontend {
            queue,
            inspect,
            backend: FsmBackendImpl::new(context)?,
            timers
        };
        
        Ok(frontend)
    }

    /// Build a new frontend for the FSM with a `FsmEventQueueVec` queue, `TimersStd` for timers and no logging.
    #[cfg(feature="std")]
    fn new(context: <Self::Fsm as FsmBackend>::Context) -> FsmResult<FsmFrontend<Self::Fsm, FsmEventQueueVec<Self::Fsm>, InspectNull, TimersStd<Self::Fsm>>> {
        let frontend = FsmFrontend {
            queue: FsmEventQueueVec::new(),
            backend: FsmBackendImpl::new(context)?,
            inspect: InspectNull::new(),
            timers: TimersStd::new(),
        };

        Ok(frontend)
    }
}