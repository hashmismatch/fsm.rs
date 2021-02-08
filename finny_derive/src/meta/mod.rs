//! Structures that describe the FSM. Used by inspection frontends and documentation.

use std::collections::HashMap;
use serde::{Serialize, Deserialize};

pub mod plantuml;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FinnyFsm {
    pub fsm_id: String,
    pub context_id: String,
    pub regions: HashMap<usize, FinnyRegion>
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FinnyRegion {
    pub region_id: usize,
    pub states: HashMap<String, FinnyStateKind>,
    pub transitions: HashMap<String, FinnyTransition>
}



#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum FinnyStateKind {
    Stopped,
    State(FinnyState),
    //SubMachine(String)
}

impl FinnyStateKind {
    pub fn get_state_id(&self) -> String {
        match self {
            FinnyStateKind::Stopped => "Stopped".into(),
            FinnyStateKind::State(s) => s.state_id.clone()
        }
    }
}


#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FinnyState {
    pub state_id: String,
    pub timers: Vec<FinnyTimer>
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FinnyTransition {
    pub transition_id: String,
    pub event: FinnyEvent,
    pub transition: FinnyTransitionKind
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum FinnyEvent {
    Start,
    Stop,
    Event(String)
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum FinnyTransitionKind {
    SelfTransition { state_id: String },
    InternalTransition { state_id: String },
    NormalTransition(FinnyTransitionNormal)
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FinnyTransitionNormal {
    pub from_state: String,
    pub to_state: String
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FinnyTimer {
    pub timer_id: String
}