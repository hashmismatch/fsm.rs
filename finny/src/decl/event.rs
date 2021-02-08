use crate::lib::*;

use crate::{FsmBackend, fsm::EventContext};
use super::{FsmQueueMock, FsmStateBuilder};

pub struct FsmEventBuilderState<'a, TFsm, TContext, TEvent, TState> {
    pub (crate) _state_builder: &'a FsmStateBuilder<TFsm, TContext, TState>,
    pub (crate) _event: PhantomData<TEvent>
}

impl<'a, TFsm, TContext, TEvent, TState> FsmEventBuilderState<'a, TFsm, TContext, TEvent, TState> {
    /// An internal transition doesn't trigger the state's entry and exit actions, as opposed to self-transitions.
    pub fn internal_transition<'b>(&'b self) -> FsmEventBuilderTransition<'b, TFsm, TContext, TEvent, TState> {
        FsmEventBuilderTransition {
            _state_event_builder: self
        }
    }

    /// A self transition triggers this state's entry and exit actions, while an internal transition does not.
    pub fn self_transition<'b>(&'b self) -> FsmEventBuilderTransition<'b, TFsm, TContext, TEvent, TState> {
        FsmEventBuilderTransition {
            _state_event_builder: self
        }
    }

    /// Transition into this state. The transition can have a guard and an action.
    pub fn transition_to<'b, TStateTo>(&'b self) -> FsmEventBuilderTransitionFull<'b, TFsm, TContext, TEvent, TState, TStateTo> {
        FsmEventBuilderTransitionFull {
            _transition_from: self,
            _state_to: PhantomData::default()
        }
    }
}


pub struct FsmEventBuilderTransition<'a, TFsm, TContext, TEvent, TState> {
    _state_event_builder: &'a FsmEventBuilderState<'a, TFsm, TContext, TEvent, TState>
}

impl<'a, TFsm, TContext, TEvent, TState> FsmEventBuilderTransition<'a, TFsm, TContext, TEvent, TState>
    where TFsm: FsmBackend
{
    /// An action that happens when the currently active state receives this event. No transitions.
    pub fn action<TAction: Fn(&TEvent, &mut EventContext<'a, TFsm, FsmQueueMock<TFsm>>, &mut TState)>(&mut self, _action: TAction) -> &mut Self {
        self
    }
    
    /// A guard for executing this action.
    pub fn guard<TGuard: Fn(&TEvent, &EventContext<'a, TFsm, FsmQueueMock<TFsm>>, &<TFsm as FsmBackend>::States) -> bool>(&mut self, _guard: TGuard) -> &mut Self {
        self
    }

    /// A type for this transition. The struct for the transition will be generated.
    pub fn with_transition_ty<TTransition>(&mut self) -> &mut Self {
        self
    }
}


pub struct FsmEventBuilderTransitionFull<'a, TFsm, TContext, TEvent, TStateFrom, TStateTo> {
    _transition_from: &'a FsmEventBuilderState<'a, TFsm, TContext, TEvent, TStateFrom>,
    _state_to: PhantomData<TStateTo>
}

impl<'a, TFsm, TContext, TEvent, TStateFrom, TStateTo> FsmEventBuilderTransitionFull<'a, TFsm, TContext, TEvent, TStateFrom, TStateTo> 
    where TFsm: FsmBackend
{
    /// An action that happens between the transitions from the two states.
    pub fn action<TAction: Fn(&TEvent, &mut EventContext<'a, TFsm, FsmQueueMock<TFsm>>, &mut TStateFrom, &mut TStateTo)>(&mut self, _action: TAction) -> &mut Self {
        self
    }

    /// A guard for starting this transition from one state to another, including executing the action.
    pub fn guard<TGuard: Fn(&TEvent, &EventContext<'a, TFsm, FsmQueueMock<TFsm>>, &<TFsm as FsmBackend>::States) -> bool>(&mut self, _guard: TGuard) -> &mut Self {
        self
    }

    /// A type for this transition. The struct for the transition will be generated.
    pub fn with_transition_ty<TTransition>(&mut self) -> &mut Self {
        self
    }    
}