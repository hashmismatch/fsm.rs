use crate::{TimerFsmSettings, lib::*};

use crate::{EventContext, FsmBackend};
use super::{FsmQueueMock, event::FsmEventBuilderState};

pub struct FsmStateBuilder<TFsm, TContext, TState> {
	pub (crate) _state: PhantomData<TState>,
	pub (crate) _fsm: PhantomData<TFsm>,
	pub (crate) _context: PhantomData<TContext>
}

impl<TFsm, TContext, TState> FsmStateBuilder<TFsm, TContext, TState>
	where TFsm: FsmBackend
{
	/// Execute this action when entering the state.
	pub fn on_entry<'a, TAction: Fn(&mut TState, &mut EventContext<'a, TFsm, FsmQueueMock<TFsm>>)>(&self, _action: TAction) -> &Self {
		self
	}

	/// Execute this action when exiting the state.
	pub fn on_exit<'a, TAction: Fn(&mut TState, &mut EventContext<'a, TFsm, FsmQueueMock<TFsm>>)>(&self, _action: TAction) -> &Self {
		self
	}

	/// What happens if we receive this event and we are in this state right now?
	pub fn on_event<TEvent>(&self) -> FsmEventBuilderState<TFsm, TContext, TEvent, TState> {
		FsmEventBuilderState {
			_state_builder: self,
			_event: PhantomData::default()
		}
	}

	/// Start a new timer when entering this state. The timer should be unit struct with a implemented
	/// Default trait. The timer is setup within a closure and the trigger is another closure
	/// that returns an event to be enqueued in the FSM.
	pub fn on_entry_start_timer<FSetup, FTrigger>(&self, _setup: FSetup, _trigger: FTrigger) -> FsmStateTimerBuilder<TFsm, TContext, TState>
		where 
			FSetup: Fn(&mut TContext, &mut TimerFsmSettings),
			FTrigger: Fn(&TContext, &TState) -> Option< <TFsm as FsmBackend>::Events >
	{
		FsmStateTimerBuilder {
			_state: self
		}
	}
}

pub struct FsmStateTimerBuilder<'a, TFsm, TContext, TState> {
	_state: &'a FsmStateBuilder<TFsm, TContext, TState>
}

impl<'a, TFsm, TContext, TState> FsmStateTimerBuilder<'a, TFsm, TContext, TState>
	where TFsm: FsmBackend
{
	/// Assign this type to the timer. The struct for it will be auto-generated.
	pub fn with_timer_ty<TTimer>(self) {

	}
}