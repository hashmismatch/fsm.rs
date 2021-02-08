use crate::{DispatchContext, FsmTimers, Inspect, lib::*};
use crate::{FsmBackend, FsmEvent, FsmEventQueue, FsmResult, FsmStates};

use super::FsmStateFactory;

/// The struct that holds the core context and state of the given Finny FSM. Doesn't include
/// environmental traits that can be changed at runtime.
pub struct FsmBackendImpl<F: FsmBackend> {
    pub context: <F as FsmBackend>::Context,
    pub states: <F as FsmBackend>::States,
    pub current_states: <<F as FsmBackend>::States as FsmStates<F>>::CurrentState
}

impl<F: FsmBackend> FsmBackendImpl<F> {
    pub fn new(context: <F as FsmBackend>::Context) -> FsmResult<Self> {

        let states = <<F as FsmBackend>::States>::new_state(&context)?;
        let current_states = <<<F as FsmBackend>::States as FsmStates<F>>::CurrentState>::default();

        let backend = FsmBackendImpl::<F> {
            context,
            states,
            current_states
        };

        Ok(backend)
    }
    
    pub fn get_context(&self) -> &<F as FsmBackend>::Context {
        &self.context
    }

    pub fn get_current_states(&self) -> <<F as FsmBackend>::States as FsmStates<F>>::CurrentState {
        self.current_states
    }

    pub fn get_state<S>(&self) -> &S
        where <F as FsmBackend>::States : AsRef<S>
    {
        self.states.as_ref()
    }
}

impl<F: FsmBackend> Deref for FsmBackendImpl<F> {
    type Target = <F as FsmBackend>::Context;

    fn deref(&self) -> &Self::Target {
        &self.context
    }
}

impl<F: FsmBackend> DerefMut for FsmBackendImpl<F> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.context
    }
}


/// The frontend of a state machine which also includes environmental services like queues
/// and inspection. The usual way to use the FSM.
pub struct FsmFrontend<F, Q, I, T> 
    where F: FsmBackend, Q: FsmEventQueue<F>, I: Inspect, T: FsmTimers<F>
{
    pub backend: FsmBackendImpl<F>,
    pub queue: Q,
    pub inspect: I,
    pub timers: T
}

impl<F, Q, I, T> FsmFrontend<F, Q, I, T>
    where F: FsmBackend, Q: FsmEventQueue<F>, I: Inspect, T: FsmTimers<F>
{
    /// Start the FSM, initiates the transition to the initial state.
    pub fn start(&mut self) -> FsmResult<()> {
        Self::dispatch_single_event(self, FsmEvent::Start)
    }

    /// Dispatch any pending timer events into the queue, then run all the
    /// events from the queue until completition.
    pub fn dispatch_timer_events(&mut self) -> FsmResult<()> {
        loop {
            if let Some(timer_id) = self.timers.get_triggered_timer() {
                self.dispatch_single_event(FsmEvent::Timer(timer_id))?;
            } else {
                break;
            }
        }

        self.dispatch_queue()
    }

    /// Dispatch this event and run it to completition.
    pub fn dispatch<E>(&mut self, event: E) -> FsmResult<()>
        where E: Into<<F as FsmBackend>::Events>
    {
        let ev = event.into();
        let ev = FsmEvent::Event(ev);
        Self::dispatch_single_event(self, ev)?;

        self.dispatch_queue()
    }

    /// Dispatch only this event, do not run it to completition.
    pub fn dispatch_single_event(&mut self, event: FsmEvent<<F as FsmBackend>::Events, <F as FsmBackend>::Timers>) -> FsmResult<()> {
        let dispatch_ctx = DispatchContext {
            backend: &mut self.backend,
            inspect: &mut self.inspect,
            queue: &mut self.queue,
            timers: &mut self.timers
        };

        F::dispatch_event(dispatch_ctx, event)
    }

    /// Dispatch the entire event queue and run it to completition.
    pub fn dispatch_queue(&mut self) -> FsmResult<()> {
        while let Some(ev) = self.queue.dequeue() {
            let ev: <F as FsmBackend>::Events = ev.into();
            // todo: log?
            Self::dispatch(self, ev);
        }

        Ok(())
    }
}

impl<F, Q, I, T> Deref for FsmFrontend<F, Q, I, T>
    where F: FsmBackend, Q: FsmEventQueue<F>, I: Inspect, T: FsmTimers<F>
{
    type Target = FsmBackendImpl<F>;

    fn deref(&self) -> &Self::Target {
        &self.backend
    }
}

impl<F, Q, I, T> DerefMut for FsmFrontend<F, Q, I, T>
    where F: FsmBackend, Q: FsmEventQueue<F>, I: Inspect, T: FsmTimers<F>
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.backend
    }
}