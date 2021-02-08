use std::collections::HashMap;

use proc_macro2::TokenStream;

use crate::{
    meta::{
        FinnyEvent, FinnyFsm, FinnyRegion, FinnyState, FinnyStateKind, FinnyTimer, FinnyTransition,
        FinnyTransitionKind, FinnyTransitionNormal,
    },
    parse::{FsmFnInput, FsmTransitionState},
    utils::tokens_to_string,
};
use quote::quote;

fn to_info_state(s: &FsmTransitionState, fsm: &FsmFnInput) -> FinnyStateKind {
    match s {
        FsmTransitionState::None => FinnyStateKind::Stopped,
        FsmTransitionState::State(s) => FinnyStateKind::State(FinnyState {
            state_id: tokens_to_string(&s.ty),
            timers: s
                .timers
                .iter()
                .map(|t| FinnyTimer {
                    timer_id: tokens_to_string(&t.get_ty(&fsm.base)),
                })
                .collect(),
        }),
    }
}

fn to_info(fsm: &FsmFnInput) -> FinnyFsm {
    let stopped_state = FinnyStateKind::Stopped;

    let finny_fsm = FinnyFsm {
        fsm_id: tokens_to_string(&fsm.base.fsm_ty),
        context_id: tokens_to_string(&fsm.base.context_ty),
        regions: fsm
            .fsm
            .regions
            .iter()
            .enumerate()
            .map(|(region_id, region)| {
                (
                    region_id,
                    FinnyRegion {
                        region_id,
                        states: region
                            .states
                            .iter()
                            .map(|state| {
                                let s =
                                    to_info_state(&FsmTransitionState::State(state.clone()), fsm);
                                (s.get_state_id(), s)
                            })
                            .chain(vec![(stopped_state.get_state_id(), stopped_state.clone())])
                            .collect(),
                        transitions: region
                            .transitions
                            .iter()
                            .map(|transition| {
                                let transition_id = tokens_to_string(&transition.transition_ty);

                                let (event, transition_ty) = match transition.ty {
                                    crate::parse::FsmTransitionType::InternalTransition(
                                        ref internal,
                                    ) => (
                                        internal.event.clone(),
                                        FinnyTransitionKind::InternalTransition { state_id: to_info_state(&internal.state, fsm).get_state_id() },
                                    ),
                                    crate::parse::FsmTransitionType::SelfTransition(
                                        ref self_transition,
                                    ) => (
                                        self_transition.event.clone(),
                                        FinnyTransitionKind::SelfTransition { state_id: to_info_state(&self_transition.state, fsm).get_state_id() },
                                    ),
                                    crate::parse::FsmTransitionType::StateTransition(ref st) => (
                                        st.event.clone(),
                                        FinnyTransitionKind::NormalTransition(
                                            FinnyTransitionNormal {
                                                from_state: to_info_state(&st.state_from, fsm)
                                                    .get_state_id(),
                                                to_state: to_info_state(&st.state_to, fsm)
                                                    .get_state_id(),
                                            },
                                        ),
                                    ),
                                };

                                let event = match event {
                                    crate::parse::FsmTransitionEvent::Stop => FinnyEvent::Stop,
                                    crate::parse::FsmTransitionEvent::Start => FinnyEvent::Start,
                                    crate::parse::FsmTransitionEvent::Event(ev) => {
                                        FinnyEvent::Event(tokens_to_string(&ev.ty))
                                    }
                                };

                                (
                                    transition_id.clone(),
                                    FinnyTransition {
                                        transition_id,
                                        event,
                                        transition: transition_ty,
                                    },
                                )
                            })
                            .collect(),
                    },
                )
            })
            .collect(),
    };

    finny_fsm
}

pub fn generate_fsm_meta(fsm: &FsmFnInput) -> TokenStream {
    let info = to_info(fsm);

    //let json = serde_json::to_string_pretty(&info).expect("Failed to serialize the FSM info JSON!");

    let fsm_ty = &fsm.base.fsm_ty;
    let fsm_info_ty = &fsm.base.fsm_info_ty;
    let fsm_ty_name_str = crate::utils::to_snake_case(&tokens_to_string(&fsm_ty));
    let (fsm_generics_impl, fsm_generics_type, fsm_generics_where) =
        fsm.base.fsm_generics.split_for_impl();

    let mut plant_uml_test_build = TokenStream::new();
    
    #[cfg(feature="generate_plantuml")]
    {
        let plant_uml = crate::meta::plantuml::to_plant_uml(&info);

        let test_fn_name = crate::utils::to_field_name(&crate::utils::ty_append(&fsm_ty, "PlantUML"));
        
        plant_uml_test_build = quote! {
            #[test]
            #[cfg(test)]
            fn #test_fn_name () {
                use std::io::prelude::*;
                use std::fs;

                let contents = #plant_uml;

                let mut f = fs::File::create(&format!("{}.plantuml", #fsm_ty_name_str )).unwrap();
                f.write_all(contents.as_bytes()).unwrap();
            }
        };
    }



    /*
    quote! {

        #[derive(Default)]
        pub struct #fsm_meta_ty;

        impl finny::FsmMetaJson for #fsm_info_ty {
            fn get_json_meta() -> &'static str {
                #json
            }

            fn get_plantuml_meta() -> &'static str {

            }
        }

    }
    */

    quote! {
        #plant_uml_test_build
    }
}
