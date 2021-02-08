use slog::{info, o, error};
use crate::{FsmBackend, FsmBackendImpl, FsmEvent, Inspect};
use crate::lib::*;
use AsRef;

pub struct InspectSlog {
    pub logger: slog::Logger
}

impl InspectSlog {
    pub fn new(logger: Option<slog::Logger>) -> Self {
        InspectSlog {
            logger: logger.unwrap_or(slog::Logger::root(slog::Discard, o!()))
        }
    }
}

impl Inspect for InspectSlog
{
    fn new_event<F: FsmBackend>(&self, event: &FsmEvent<<F as FsmBackend>::Events, <F as FsmBackend>::Timers>, fsm: &FsmBackendImpl<F>) -> Self {
        let event_display = match event {
            FsmEvent::Timer(t) => format!("Fsm::Timer({:?})", t),
            _ => event.as_ref().to_string()
        };

        let current_state = format!("{:?}", fsm.get_current_states());

        let kv = o!("event" => event_display, "start_state" => current_state);
        info!(self.logger, "Dispatching"; &kv);
        InspectSlog {
            logger: self.logger.new(kv)
        }
    }

    fn for_transition<T>(&self) -> Self {
        let transition = type_name::<T>();
        let kv = o!("transition" => transition);
        info!(self.logger, "Matched transition"; &kv);
        InspectSlog {
            logger: self.logger.new(kv)
        }
    }
 
    fn for_sub_machine<FSub: FsmBackend>(&self) -> Self {
        let sub_fsm = type_name::<FSub>();
        let kv = o!("sub_fsm" => sub_fsm);
        info!(self.logger, "Dispatching to a submachine"; &kv);
        InspectSlog {
            logger: self.logger.new(kv)
        }
    }

    fn for_timer<F>(&self, timer_id: <F as FsmBackend>::Timers) -> Self where F: FsmBackend {
        let kv = o!("timer_id" => format!("{:?}", timer_id));
        InspectSlog {
            logger: self.logger.new(kv)
        }
    }    

    fn on_guard<T>(&self, guard_result: bool) {
        let guard = type_name::<T>();
        info!(self.logger, "Guard {guard} evaluated to {guard_result}", guard = guard, guard_result = guard_result);
    }

    fn on_state_enter<S>(&self) {
        let state = type_name::<S>();
        info!(self.logger, "Entering {state}", state = state);
    }

    fn on_state_exit<S>(&self) {
        let state = type_name::<S>();
        info!(self.logger, "Exiting {state}", state = state);
    }

    fn on_action<S>(&self) {
        let action = type_name::<S>();
        info!(self.logger, "Executing {action}", action = action);
    }

    fn event_done<F: FsmBackend>(self, fsm: &FsmBackendImpl<F>) {
        let states = format!("{:?}", fsm.get_current_states());
        info!(self.logger, "Dispatch done"; "stop_state" => states);
    }

    fn on_error<E>(&self, msg: &str, error: &E) where E: Debug {
        let kv = o!("error" => format!("{:?}", error));
        error!(self.logger, "{}", msg; kv);
    }

    fn info(&self, msg: &str) {
        info!(self.logger, "{}", msg);
    }
}