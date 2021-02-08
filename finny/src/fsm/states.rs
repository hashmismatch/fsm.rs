use crate::{FsmBackend, lib::*};

use crate::FsmResult;

/// The implementation should hold all of the FSM's states as fields.
pub trait FsmStates<TFsm>: FsmStateFactory<TFsm> where TFsm: FsmBackend {
    /// The enum type for all states that's used as the "current state" field in the FSM's backend.
    type StateKind: Clone + Copy + Debug + PartialEq;
    /// An array of current states for the machine, one for each region.
    type CurrentState: Clone + Copy + Debug + Default + AsRef<[FsmCurrentState<Self::StateKind>]> + AsMut<[FsmCurrentState<Self::StateKind>]>;
}

/// The current state of the FSM.
#[derive(Copy, Clone, PartialEq)]
pub enum FsmCurrentState<S> where S: Clone + Copy {
    /// The FSM is halted and has to be started using the `start()` method.
    Stopped,
    /// The FSM is in this state.
    State(S)
}

impl<S> FsmCurrentState<S> where S: Clone + Copy {
    pub fn all_stopped(current_states: &[Self]) -> bool {
        current_states.iter().all(|s| match s {
            FsmCurrentState::Stopped => true,
            _ => false
        })
    }
}

impl<S> Debug for FsmCurrentState<S> where S: Debug + Copy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FsmCurrentState::Stopped => f.write_str("Fsm::Stopped"),
            FsmCurrentState::State(s) => s.fmt(f)
        }
    }
}

impl<S> Default for FsmCurrentState<S> where S: Clone + Copy {
    fn default() -> Self {
        Self::Stopped
    }
}

/// Create a new state from the shared global context.
pub trait FsmStateFactory<TFsm> where Self: Sized, TFsm: FsmBackend {
    /// Constructor for building this state from the shared global context.
    fn new_state(context: &<TFsm as FsmBackend>::Context) -> FsmResult<Self>;
}

impl<TState, TFsm> FsmStateFactory<TFsm> for TState where TState: Default, TFsm: FsmBackend {
    fn new_state(_context: &<TFsm as FsmBackend>::Context) -> FsmResult<Self> {
        Ok(Default::default())
    }
}

/// Retrieve a pair of states as immutable references. Used in state transitions.
pub trait FsmStateTransitionAsRef<T1, T2> {
    fn as_state_transition_ref(&self) -> (&T1, &T2);
}

/// Retrieve a pair of states as mutable references. Used in state transitions.
pub trait FsmStateTransitionAsMut<T1, T2> {
    fn as_state_transition_mut(&mut self) -> (&mut T1, &mut T2);
}