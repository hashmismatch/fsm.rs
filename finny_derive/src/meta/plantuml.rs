use super::FinnyFsm;
use std::fmt::Write;


pub fn to_plant_uml(fsm: &FinnyFsm) -> String {
    let mut output = String::new();

    output.push_str("@startuml\n");

    for region in fsm.regions.values() {
        for state in region.states.values() {
            match state {
                super::FinnyStateKind::Stopped => {

                }
                super::FinnyStateKind::State(state) => {

                    writeln!(&mut output, "state {} {{", state.state_id);
                    

                    writeln!(&mut output, "}}");
                }
            }
        }

        for transition in region.transitions.values() {

            let event = match transition.event {
                super::FinnyEvent::Start => "Start".to_string(),
                super::FinnyEvent::Stop => "Stop".to_string(),
                super::FinnyEvent::Event(ref ev) => ev.clone()
            };

            match &transition.transition {
                super::FinnyTransitionKind::SelfTransition { state_id } => {
                    writeln!(&mut output, "{state} --> {state} : {event} (Self)", state = state_id, event = event);
                }
                super::FinnyTransitionKind::InternalTransition { state_id } => {
                    writeln!(&mut output, "{state} --> {state} : {event} (Internal)", state = state_id, event = event);
                }
                super::FinnyTransitionKind::NormalTransition(t) => {
                    let state_from = match t.from_state.as_str() {
                        "Stopped" => "[*]",
                        _ => &t.from_state
                    };

                    writeln!(&mut output, "{state_from} --> {state_to} : {event}", state_from = state_from, state_to = t.to_state, event = event);
                }
            }
        }
    }

    output.push_str("@enduml\n");

    output
}