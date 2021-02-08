use std::collections::HashSet;

use proc_macro2::{Span, TokenStream};
use quote::{TokenStreamExt, quote};
use crate::{codegen_meta::generate_fsm_meta, fsm::FsmTypes, parse::{FsmState, FsmStateAction, FsmStateKind}, utils::{remap_closure_inputs, to_field_name}};

use crate::{parse::{FsmFnInput, FsmStateTransition, FsmTransitionState, FsmTransitionType}, utils::ty_append};

pub fn generate_fsm_code(fsm: &FsmFnInput, _attr: TokenStream, _input: TokenStream) -> syn::Result<TokenStream> {
    let fsm_ty = &fsm.base.fsm_ty;
    let fsm_types = FsmTypes::new(&fsm.base.fsm_ty, &fsm.base.fsm_generics);
    //let fsm_mod = to_field_name(&ty_append(fsm_ty, "Finny"))?;
    let ctx_ty = &fsm.base.context_ty;

    let states_store_ty = ty_append(&fsm.base.fsm_ty, "States");
    let states_enum_ty = ty_append(&fsm.base.fsm_ty, "CurrentState");
    let timers_enum_ty = fsm_types.get_fsm_timers_ty();
    let timers_enum_iter_ty = fsm_types.get_fsm_timers_iter_ty();
    let timers_storage_ty = fsm_types.get_fsm_timers_storage_ty();
    let event_enum_ty = fsm_types.get_fsm_events_ty();

    let region_count = fsm.fsm.regions.len();

    let (fsm_generics_impl, fsm_generics_type, fsm_generics_where) = fsm.base.fsm_generics.split_for_impl();

    let states_store = {

        let mut code_fields = TokenStream::new();
        let mut new_state_fields = TokenStream::new();
        let mut state_variants = TokenStream::new();
        let mut state_accessors = TokenStream::new();

        for (i, (_, state)) in fsm.fsm.states.iter().enumerate() {
            let name = &state.state_storage_field;
            let state_ty = FsmTypes::new(&state.ty,&fsm.base.fsm_generics);
            let ty = state_ty.get_fsm_ty();
            let ty_name = state_ty.get_fsm_no_generics_ty();

            for timer in &state.timers {
                let timer_ty = timer.get_ty(&fsm.base);
                let timer_field = timer.get_field(&fsm.base);

                code_fields.append_all(quote! { #timer_field: #timer_ty #fsm_generics_type, });
                new_state_fields.append_all(quote! { #timer_field: #timer_ty::default(), });

                state_accessors.append_all(quote! {
                    impl #fsm_generics_impl core::convert::AsRef<#timer_ty #fsm_generics_type> for #states_store_ty #fsm_generics_type #fsm_generics_where {
                        fn as_ref(&self) -> & #timer_ty #fsm_generics_type {
                            &self. #timer_field
                        }
                    }
    
                    impl #fsm_generics_impl core::convert::AsMut<#timer_ty #fsm_generics_type> for #states_store_ty #fsm_generics_type #fsm_generics_where {
                        fn as_mut(&mut self) -> &mut #timer_ty #fsm_generics_type {
                            &mut self. #timer_field
                        }
                    }
                });
            }

            code_fields.append_all(quote! { #name: #ty, });
            state_variants.append_all(quote!{ #ty_name, });

            let new_state_field = match state.kind {
                FsmStateKind::Normal => {
                    quote! {
                        #name: < #ty as finny::FsmStateFactory< #fsm_ty #fsm_generics_type > >::new_state(context)?,
                    }
                }
                FsmStateKind::SubMachine(ref sub) => {

                    let ctx_codegen = match &sub.context_constructor {
                        Some(c) => {
                            let remap = remap_closure_inputs(&c.inputs, &[quote!{ context }])?;
                            let body = &c.body;
                            quote! {
                                #remap
                                {
                                    #body
                                }
                            }
                        },
                        None => {
                            quote! {
                                Default::default()
                            }
                        }
                    };

                    quote! {
                        #name: {
                            use finny::{FsmFactory};
                            
                            let sub_ctx = {
                                #ctx_codegen
                            };
                            let fsm_backend = finny::FsmBackendImpl::<#ty>::new(sub_ctx)?;
                            let fsm = <#ty>::new_submachine_backend(fsm_backend)?;
                            fsm
                        },
                    }
                }
            };
            new_state_fields.append_all(new_state_field);

            state_accessors.append_all(quote! {
                impl #fsm_generics_impl core::convert::AsRef<#ty> for #states_store_ty #fsm_generics_type #fsm_generics_where {
                    fn as_ref(&self) -> & #ty {
                        &self. #name
                    }
                }

                impl #fsm_generics_impl core::convert::AsMut<#ty> for #states_store_ty #fsm_generics_type #fsm_generics_where {
                    fn as_mut(&mut self) -> &mut #ty {
                        &mut self. #name
                    }
                }
            });
        }

        let mut transition_states = TokenStream::new();


        let mut transitions_seen = HashSet::new();
        for region in &fsm.fsm.regions {
            for transition in &region.transitions {
                match transition.ty {
                    FsmTransitionType::StateTransition(ref s) => {
                        match (s.state_from.get_fsm_state(), s.state_to.get_fsm_state()) {
                            (Ok(state_from), Ok(state_to)) => {

                                let state_from_ty = &state_from.ty;
                                let state_to_ty = &state_to.ty;

                                let state_from_field = &state_from.state_storage_field;
                                let state_to_field = &state_to.state_storage_field;

                                let key = (state_from_ty.clone(), state_to_ty.clone());
                                if transitions_seen.contains(&key) { continue; }
                                transitions_seen.insert(key);

                                transition_states.append_all(quote! {
                                    impl #fsm_generics_impl finny::FsmStateTransitionAsMut<#state_from_ty, #state_to_ty> for #states_store_ty #fsm_generics_type #fsm_generics_where {
                                        fn as_state_transition_mut(&mut self) -> (&mut #state_from_ty, &mut #state_to_ty) {
                                            (&mut self. #state_from_field, &mut self. #state_to_field)
                                        }
                                    }
                                });

                            },
                            _ => ()
                        }
                    },
                    _ => ()
                }
            }
        }

        quote! {
            pub struct #states_store_ty #fsm_generics_type #fsm_generics_where {
                #code_fields
                _fsm: core::marker::PhantomData< #fsm_ty #fsm_generics_type >
            }
            
            impl #fsm_generics_impl finny::FsmStateFactory< #fsm_ty #fsm_generics_type > for #states_store_ty #fsm_generics_type #fsm_generics_where {
                fn new_state(context: & #ctx_ty ) -> finny::FsmResult<Self> {
                    let s = Self {
                        #new_state_fields
                        _fsm: core::marker::PhantomData::default()
                    };
                    Ok(s)
                }
            }
            
            #[derive(Copy, Clone, Debug, PartialEq)]
            pub enum #states_enum_ty {
                #state_variants
            }

            impl #fsm_generics_impl finny::FsmStates< #fsm_ty #fsm_generics_type > for #states_store_ty #fsm_generics_type #fsm_generics_where {
                type StateKind = #states_enum_ty;
                type CurrentState = [finny::FsmCurrentState<Self::StateKind>; #region_count];
            }

            #state_accessors

            #transition_states
        }
    };
    

    let events_enum = {

        let submachines: Vec<_> = fsm.fsm.states.iter().filter_map(|(_, state)| {
            match &state.kind {
                FsmStateKind::Normal => None,
                FsmStateKind::SubMachine(ref sub) => {
                    Some((sub, state))
                }
            }
        }).collect();

        let mut variants = TokenStream::new();
        let mut as_ref_str = TokenStream::new();

        for (ty, _ev) in  fsm.fsm.events.iter() {
            let ty_str = crate::utils::tokens_to_string(ty);

            variants.append_all(quote! { #ty ( #ty ),  });            
            as_ref_str.append_all(quote! { #event_enum_ty:: #ty(_) => #ty_str, });
        }
        for (_sub, state) in submachines {
            let sub_fsm = FsmTypes::new(&state.ty, &fsm.base.fsm_generics);
            let sub_fsm_event_ty = sub_fsm.get_fsm_events_ty();
            let sub_fsm_ty = sub_fsm.get_fsm_no_generics_ty();            

            let sub_fsm_event_ty_str = crate::utils::tokens_to_string(&sub_fsm_event_ty);

            variants.append_all(quote! {
                #sub_fsm_ty ( #sub_fsm_event_ty ),
            });
            as_ref_str.append_all(quote! {
                #event_enum_ty :: #sub_fsm_ty(_) => #sub_fsm_event_ty_str ,
            });
        }

        let mut derives = TokenStream::new();
        if fsm.fsm.codegen_options.event_debug {
            derives.append_all(quote! {
                #[derive(Debug)]
            });
        }
        
        let evs = quote! {
            #[derive(finny::bundled::derive_more::From)]
            #[derive(Clone)]
            #derives
            pub enum #event_enum_ty {
                #variants
            }

            impl core::convert::AsRef<str> for #event_enum_ty {
                fn as_ref(&self) -> &'static str {
                    match self {
                        #as_ref_str
                    }
                }
            }
        };

        evs
    };
    
    let transition_types = {
        let mut t = TokenStream::new();
        
        for region in &fsm.fsm.regions {
            for transition in &region.transitions {

                let ty = &transition.transition_ty;
                let mut q = quote! {
                    pub struct #ty;
                };

                match &transition.ty {
                    // internal or self transtion (only the current state)
                    FsmTransitionType::InternalTransition(s) | FsmTransitionType::SelfTransition(s) => {
                        
                        let state = s.state.get_fsm_state()?;
                        let event_ty = &s.event.get_event()?.ty;

                        let is_self_transition = if let FsmTransitionType::SelfTransition(_) = &transition.ty { true } else { false };

                        if let Some(ref guard) = s.action.guard {
                            let remap = remap_closure_inputs(&guard.inputs, vec![
                                quote! { event }, quote! { context }, quote! { states }
                            ].as_slice())?;

                            let body = &guard.body;

                            let g = quote! {
                                impl #fsm_generics_impl finny::FsmTransitionGuard<#fsm_ty #fsm_generics_type, #event_ty> for #ty #fsm_generics_where {
                                    fn guard<'fsm_event, Q>(event: & #event_ty, context: &finny::EventContext<'fsm_event, #fsm_ty #fsm_generics_type, Q>, states: & #states_store_ty #fsm_generics_type ) -> bool
                                        where Q: finny::FsmEventQueue<#fsm_ty #fsm_generics_type>
                                    {
                                        #remap
                                        let result = { #body };
                                        result
                                    }
                                }
                            };

                            q.append_all(g);
                        }
                        
                        let action_body = if let Some(ref action) = s.action.action {
                            let remap = remap_closure_inputs(&action.inputs, vec![
                                quote! { event }, quote! { context }, quote! { state }
                            ].as_slice())?;

                            let body = &action.body;
                            
                            quote! { 
                                #remap
                                { #body }
                            }
                        } else {
                            TokenStream::new()
                        };

                        let state_ty = &state.ty;
                        q.append_all(quote! {
                            impl #fsm_generics_impl finny::FsmAction<#fsm_ty #fsm_generics_type, #event_ty, #state_ty > for #ty #fsm_generics_where {
                                fn action<'fsm_event, Q>(event: & #event_ty , context: &mut finny::EventContext<'fsm_event, #fsm_ty #fsm_generics_type, Q >, state: &mut #state_ty)
                                    where Q: finny::FsmEventQueue<#fsm_ty #fsm_generics_type>
                                {
                                    #action_body
                                }

                                fn should_trigger_state_actions() -> bool {
                                    #is_self_transition
                                }
                            }
                        });
                    },

                    // fsm start transition
                    FsmTransitionType::StateTransition(s @ FsmStateTransition { state_from: FsmTransitionState::None, .. }) => {
                        let initial_state_ty = &s.state_to.get_fsm_state()?.ty;

                        q.append_all(quote! {
                            impl #fsm_generics_impl finny::FsmTransitionFsmStart<#fsm_ty #fsm_generics_type, #initial_state_ty > for #ty #fsm_generics_where {

                            }
                        });

                    },

                    // normal state transition
                    FsmTransitionType::StateTransition(s) => {

                        if let Some(ref guard) = s.action.guard {
                            let event_ty = &s.event.get_event()?.ty;

                            let remap = remap_closure_inputs(&guard.inputs, vec![
                                quote! { event }, quote! { context }, quote! { states }
                            ].as_slice())?;

                            let body = &guard.body;

                            let g = quote! {
                                impl #fsm_generics_impl finny::FsmTransitionGuard<#fsm_ty #fsm_generics_type, #event_ty> for #ty #fsm_generics_where {
                                    fn guard<'fsm_event, Q>(event: & #event_ty, context: &finny::EventContext<'fsm_event, #fsm_ty #fsm_generics_type, Q>, states: & #states_store_ty #fsm_generics_type) -> bool
                                        where Q: finny::FsmEventQueue<#fsm_ty #fsm_generics_type>
                                    {
                                        #remap
                                        let result = { #body };
                                        result
                                    }
                                }
                            };

                            q.append_all(g);
                        }

                        let action_body = if let Some(ref action) = s.action.action {
                            let remap = remap_closure_inputs(&action.inputs, vec![
                                quote! { event }, quote! { context }, quote! { from }, quote! { to }
                            ].as_slice())?;

                            let body = &action.body;

                            quote! {
                                #remap
                                { #body }
                            }
                        } else {
                            TokenStream::new()
                        };

                        let event_ty = &s.event.get_event()?.ty;
                        let state_from = s.state_from.get_fsm_state()?;
                        let state_to = s.state_to.get_fsm_state()?;
                        
                        let state_from_ty = &state_from.ty;
                        let state_to_ty = &state_to.ty;

                        let a = quote! {
                            impl #fsm_generics_impl finny::FsmTransitionAction<#fsm_ty #fsm_generics_type, #event_ty, #state_from_ty, #state_to_ty> for #ty #fsm_generics_where {
                                fn action<'fsm_event, Q>(event: & #event_ty , context: &mut finny::EventContext<'fsm_event, #fsm_ty #fsm_generics_type, Q >, from: &mut #state_from_ty, to: &mut #state_to_ty)
                                    where Q: finny::FsmEventQueue<#fsm_ty #fsm_generics_type>
                                {
                                    #action_body
                                }
                            }
                        };

                        q.append_all(a);
                    }
                }
                
                t.append_all(q);
            }
        }

        t
    };

    let dispatch = {        
        

        let mut regions = TokenStream::new();
        for region in &fsm.fsm.regions {
            let mut region_transitions = TokenStream::new();

            let region_id = region.region_id;
            for transition in &region.transitions {

                let transition_ty = &transition.transition_ty;
                
                let match_state = {
                    let state_from = match &transition.ty {
                        FsmTransitionType::InternalTransition(s) | FsmTransitionType::SelfTransition(s) => {
                            &s.state
                        }
                        FsmTransitionType::StateTransition(s) => &s.state_from
                    };

                    match state_from {
                        FsmTransitionState::None => quote! { finny::FsmCurrentState::Stopped },
                        FsmTransitionState::State(st) => {
                            let state_ty = FsmTypes::new(&st.ty, &fsm.base.fsm_generics);
                            let variant = state_ty.get_fsm_no_generics_ty();
                            quote! { finny::FsmCurrentState::State(#states_enum_ty :: #variant) }
                        }
                    }
                };
                
                let match_event = {                
                    let event = match &transition.ty {
                        FsmTransitionType::InternalTransition(s) | FsmTransitionType::SelfTransition(s) => &s.event,
                        FsmTransitionType::StateTransition(s) => &s.event
                    };

                    match event {
                        crate::parse::FsmTransitionEvent::Start => quote! { ev @ finny::FsmEvent::Start },
                        crate::parse::FsmTransitionEvent::Stop => quote ! { ev @ finny::FsmEvent::Stop },
                        crate::parse::FsmTransitionEvent::Event(ref ev) => {
                            let kind = &ev.ty;
                            quote! { finny::FsmEvent::Event(#event_enum_ty::#kind(ref ev)) }
                        }
                    }
                };

                let guard = {
                    let has_guard = match &transition.ty {
                        FsmTransitionType::StateTransition(s) => {
                            s.action.guard.is_some()
                        }
                        FsmTransitionType::InternalTransition(s) | FsmTransitionType::SelfTransition(s) => {
                            s.action.guard.is_some()
                        }
                    };

                    if has_guard {
                        quote! {
                            if <#transition_ty>::execute_guard(&mut ctx, &ev, #region_id, &mut inspect_event_ctx)
                        }
                    } else {
                        TokenStream::new()
                    }
                };
                
                let fsm_sub_entry = match &transition.ty {
                    FsmTransitionType::StateTransition(FsmStateTransition {state_to: FsmTransitionState::State(s @ FsmState { kind: FsmStateKind::SubMachine(_), .. }), .. }) => {
                        quote! {
                            {
                                <#transition_ty>::execute_on_sub_entry(&mut ctx, #region_id, &mut inspect_event_ctx);
                            }
                        }
                    },
                    _ => TokenStream::new()
                };

                let timers_enter = {
                    let mut timers_enter = TokenStream::new();

                    let state = match &transition.ty {
                        FsmTransitionType::SelfTransition(FsmStateAction { state: FsmTransitionState::State(st @ FsmState { kind: FsmStateKind::Normal, .. }), .. }) => {
                            Some(st)
                        },
                        FsmTransitionType::StateTransition(FsmStateTransition { state_to: FsmTransitionState::State(st @ FsmState { kind: FsmStateKind::Normal, .. }), .. }) => {
                            Some(st)
                        },
                        _ => None
                    };

                    if let Some(state) = state {
                        for timer in &state.timers {
                            let timer_field = timer.get_field(&fsm.base);
                            let timer_ty = timer.get_ty(&fsm.base);

                            timers_enter.append_all(quote! {
                                {
                                    use finny::FsmTimer;
                                    ctx.backend.states. #timer_field . execute_on_enter( #timers_enum_ty :: #timer_ty , &mut ctx.backend.context, &mut inspect_event_ctx, ctx.timers );
                                }
                            });
                        }
                    }

                    timers_enter
                };

                let timers_exit = {
                    let mut timers_exit = TokenStream::new();

                    let state = match &transition.ty {
                        FsmTransitionType::SelfTransition(FsmStateAction { state: FsmTransitionState::State(st @ FsmState { kind: FsmStateKind::Normal, .. }), .. }) => {
                            Some(st)
                        },
                        FsmTransitionType::StateTransition(FsmStateTransition { state_from: FsmTransitionState::State(st @ FsmState { kind: FsmStateKind::Normal, .. }), .. }) => {
                            Some(st)
                        },
                        _ => None
                    };

                    if let Some(state) = state {
                        for timer in &state.timers {
                            let timer_field = timer.get_field(&fsm.base);
                            let timer_ty = timer.get_ty(&fsm.base);

                            timers_exit.append_all(quote! {
                                {
                                    use finny::FsmTimer;
                                    ctx.backend.states. #timer_field . execute_on_exit( #timers_enum_ty :: #timer_ty , &mut inspect_event_ctx, ctx.timers );
                                }
                            });
                        }
                    }

                    timers_exit
                };

                let m = quote! {
                    ( #match_state , #match_event ) #guard => {

                        #timers_exit

                        <#transition_ty>::execute_transition(&mut ctx, &ev, #region_id, &mut inspect_event_ctx);

                        #fsm_sub_entry
                        
                        #timers_enter                        
                    },
                };

                region_transitions.append_all(m);
            }

            // match and dispatch to submachines
            let region_submachines = {

                let mut sub_matches = TokenStream::new();

                let submachines: Vec<_> = region.transitions.iter().filter_map(|t| match &t.ty {
                    FsmTransitionType::InternalTransition(_) => None,
                    FsmTransitionType::SelfTransition(_) => None,
                    FsmTransitionType::StateTransition(FsmStateTransition { state_to: FsmTransitionState::State(s @ FsmState { kind: FsmStateKind::SubMachine(_), .. }), .. }) => {
                        Some(s)
                    },
                    _ => None
                }).collect();

                for submachine in submachines {
                    let kind = &submachine.ty;
                    let fsm_sub = FsmTypes::new(&submachine.ty, &fsm.base.fsm_generics);
                    let kind_variant = fsm_sub.get_fsm_no_generics_ty();

                    let sub = quote! {
                        ( finny::FsmCurrentState::State(#states_enum_ty :: #kind_variant), finny::FsmEvent::Event(#event_enum_ty::#kind_variant(ev))  ) => {
                            return finny::dispatch_to_submachine::<_, #kind, _, _, _>(&mut ctx, finny::FsmEvent::Event(ev.clone()), &mut inspect_event_ctx);
                        },
                    };

                    sub_matches.append_all(sub);
                }

                sub_matches
            };

            // match and dispatch timer events
            let timers = {
                let mut timer_dispatch = TokenStream::new();

                // our timers
                for state in &region.states {
                    for timer in &state.timers {
                        let timer_ty = timer.get_ty(&fsm.base);

                        timer_dispatch.append_all(quote! {
                            (_, finny::FsmEvent::Timer( timer_id @ #timers_enum_ty :: #timer_ty )) => {
                                {
                                    use finny::FsmTimer;
                                    < #timer_ty #fsm_generics_type > :: execute_trigger(*timer_id, &mut ctx, &mut inspect_event_ctx);
                                }
                            },
                        });
                    }
                }

                // sub machines
                for state in region.states.iter().filter(|s| if let FsmStateKind::SubMachine(_) = s.kind { true } else { false })
                {
                    let sub = &state.ty;
                    let sub_ty = FsmTypes::new(sub, &fsm.base.fsm_generics);
                    let sub_variant = sub_ty.get_fsm_no_generics_ty();

                    timer_dispatch.append_all(quote! {
                        (_, finny::FsmEvent::Timer( #timers_enum_ty :: #sub_variant (timer_id))) => {
                            {
                                let ev = finny::FsmEvent::Timer(*timer_id);
                                return finny::dispatch_to_submachine::<_, #sub, _, _, _>(&mut ctx, ev, &mut inspect_event_ctx);
                            }
                        },
                    });
                }

                timer_dispatch
            };

            regions.append_all(quote! {
                match (ctx.backend.current_states[#region_id], &event) {

                    #region_submachines
                    
                    #region_transitions

                    // do not dispatch timers if the machine is stopped
                    (finny::FsmCurrentState::Stopped, finny::FsmEvent::Timer(_)) => (),

                    #timers

                    _ => {
                        transition_misses += 1;
                    }
                }
            });
        }

        quote! {
              
            impl #fsm_generics_impl finny::FsmBackend for #fsm_ty #fsm_generics_type
                #fsm_generics_where
            {
                type Context = #ctx_ty;
                type States = #states_store_ty #fsm_generics_type;
                type Events = #event_enum_ty;
                type Timers = #timers_enum_ty;

                fn dispatch_event<Q, I, T>(mut ctx: finny::DispatchContext<Self, Q, I, T>, event: finny::FsmEvent<Self::Events, Self::Timers>) -> finny::FsmDispatchResult
                    where Q: finny::FsmEventQueue<Self>,
                    I: finny::Inspect, T: finny::FsmTimers<Self>
                {
                    use finny::{FsmTransitionGuard, FsmTransitionAction, FsmAction, FsmState, FsmTransitionFsmStart};

                    let mut transition_misses = 0;

                    let mut inspect_event_ctx = ctx.inspect.new_event::<Self>(&event, &ctx.backend);

                    #regions

                    let result = if transition_misses == #region_count {
                        Err(finny::FsmError::NoTransition)
                    } else {
                        Ok(())
                    };

                    inspect_event_ctx.event_done(&ctx.backend);

                    result
                }
            }
        }
    };
    
    let states = {

        let mut states = TokenStream::new();
        for (ty, state) in fsm.fsm.states.iter() {

            let remap_closure = |c: &Option<syn::ExprClosure>| -> syn::Result<TokenStream> {
                if let Some(c) = &c {
                    let remap = remap_closure_inputs(&c.inputs, &vec![ quote! { self }, quote! { context } ])?;
                    let b = &c.body;
                    
                    let q = quote! {                                        
                        #remap
                        { #b }
                    };
                    Ok(q)
                } else {
                    Ok(TokenStream::new())
                }
            };

            let on_entry = remap_closure(&state.on_entry_closure)?;
            let on_exit = remap_closure(&state.on_exit_closure)?;

            let state_ty = FsmTypes::new(&ty, &fsm.base.fsm_generics);
            let variant = state_ty.get_fsm_no_generics_ty();            

            let state = quote! {

                impl #fsm_generics_impl finny::FsmState<#fsm_ty #fsm_generics_type> for #ty #fsm_generics_where {
                    fn on_entry<'fsm_event, Q: finny::FsmEventQueue<#fsm_ty #fsm_generics_type>>(&mut self, context: &mut finny::EventContext<'fsm_event, #fsm_ty #fsm_generics_type, Q>) {
                        #on_entry
                    }

                    fn on_exit<'fsm_event, Q: finny::FsmEventQueue<#fsm_ty #fsm_generics_type>>(&mut self, context: &mut finny::EventContext<'fsm_event, #fsm_ty #fsm_generics_type, Q>) {
                        #on_exit
                    }

                    fn fsm_state() -> #states_enum_ty {
                        #states_enum_ty :: #variant
                    }
                }

            };

            states.append_all(state);

        }

        states
    };

    let builder = {

        quote! {

            pub struct #fsm_ty #fsm_generics_type #fsm_generics_where {
                backend: finny::FsmBackendImpl<#fsm_ty #fsm_generics_type >
            }

            impl #fsm_generics_impl finny::FsmFactory for #fsm_ty #fsm_generics_type #fsm_generics_where {
                type Fsm = #fsm_ty #fsm_generics_type;

                fn new_submachine_backend(backend: finny::FsmBackendImpl<Self::Fsm>) -> finny::FsmResult<Self> where Self: Sized {
                    Ok(Self {
                        backend
                    })
                }
            }

            impl #fsm_generics_impl core::ops::Deref for #fsm_ty #fsm_generics_type #fsm_generics_where {
                type Target = finny::FsmBackendImpl<#fsm_ty #fsm_generics_type >;

                fn deref(&self) -> &Self::Target {
                    &self.backend
                }
            }

            impl #fsm_generics_impl core::ops::DerefMut for #fsm_ty #fsm_generics_type #fsm_generics_where {
                fn deref_mut(&mut self) -> &mut Self::Target {
                    &mut self.backend
                }
            }
            
        }
    };

    let timers = {

        let mut code = TokenStream::new();

        let mut enum_variants = vec![];
        let mut submachines = vec![];
        let mut our_timers = vec![];

        let states = fsm.fsm.states.iter().map(|s| s.1);
        for state in states {

            if let FsmStateKind::SubMachine(_) = &state.kind {
                let sub_fsm_ty = FsmTypes::new(&state.ty, &fsm.base.fsm_generics);
                let n = sub_fsm_ty.get_fsm_no_generics_ty();
                let t = sub_fsm_ty.get_fsm_timers_ty();
                enum_variants.push(quote! { #n ( #t )  });
                submachines.push(sub_fsm_ty.clone());

                code.append_all(quote! {

                    impl From<#t> for #timers_enum_ty {
                        fn from(t: #t) -> Self {
                            #timers_enum_ty :: #n ( t )
                        }
                    }

                });
            }

            for timer in &state.timers {
                let state_ty = &state.ty;
                let timer_ty = timer.get_ty(&fsm.base);

                enum_variants.push(quote! { #timer_ty });
                our_timers.push(timer_ty.clone());

                let setup = remap_closure_inputs(&timer.setup.inputs, &[quote! { ctx }, quote! { settings }])?;
                let setup_body = &timer.setup.body;

                let trigger = remap_closure_inputs(&timer.trigger.inputs, &[quote! { ctx }, quote! { state }])?;
                let trigger_body = &timer.trigger.body;

                code.append_all(quote! {

                    pub struct #timer_ty #fsm_generics_type #fsm_generics_where {
                        instance: Option<finny::TimerInstance < #fsm_ty #fsm_generics_type > >
                    }

                    impl #fsm_generics_impl core::default::Default for #timer_ty #fsm_generics_type #fsm_generics_where {
                        fn default() -> Self {
                            Self {
                                instance: None
                            }
                        }
                    }

                    impl #fsm_generics_impl finny::FsmTimer< #fsm_ty #fsm_generics_type , #state_ty > for #timer_ty #fsm_generics_type #fsm_generics_where {
                        fn setup(ctx: &mut #ctx_ty, settings: &mut finny::TimerFsmSettings) {
                            #setup
                            {
                                #setup_body
                            }
                        }

                        fn trigger(ctx: & #ctx_ty, state: & #state_ty ) -> Option< #event_enum_ty > {
                            #trigger
                            let ret = {
                                #trigger_body
                            };
                            ret
                        }

                        fn get_instance(&self) -> &Option<finny::TimerInstance < #fsm_ty #fsm_generics_type > > {
                            &self.instance
                        }

                        fn get_instance_mut(&mut self) -> &mut Option<finny::TimerInstance < #fsm_ty #fsm_generics_type > > {
                            &mut self.instance
                        }
                    }

                });
            }
        }


        let variants = {
            let mut t = TokenStream::new();
            t.append_separated(&enum_variants, quote! { , });
            t
        };

        code.append_all(quote! {
            #[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
            pub enum #timers_enum_ty {
                #variants
            }
        });

        let submachine_iters: Vec<_> = submachines.iter().map(|s| {
            let ty = s.get_fsm_timers_iter_ty();
            let field = to_field_name(&ty);
            (ty, field)
        }).collect();

        let enum_iter_matches_variants = our_timers.iter().enumerate().map(|(i, variant)| quote! {
            #i => { self.position += 1; Some(#timers_enum_ty :: #variant) }
        });
        let mut enum_iter_matches = TokenStream::new();
        enum_iter_matches.append_separated(enum_iter_matches_variants, quote! { , });
        enum_iter_matches.append_separated(submachine_iters.iter().enumerate().map(|(i, (ty, field))| {
            let i = our_timers.len() + i;
            quote! {
                #i if self.#field.is_some() => { 
                    if let Some(ref mut iter) = self.#field {
                        let r = iter.next();
                        if let Some(r) = r {
                            let r = r.into();
                            return Some(r);
                        } else {
                            self.#field = None;
                            self.position += 1;
                            return self.next();
                        }
                    } else { None }
                }
            }
        }), quote! { , });
        
        let mut submachine_iter_struct = TokenStream::new();
        submachine_iter_struct.append_separated(submachine_iters.iter().map(|(ty, field)| quote! {
            #field : Option< #ty >
        }), quote! { , });

        let mut submachine_iter_new = TokenStream::new();
        submachine_iter_new.append_separated(submachine_iters.iter().map(|(ty, field)| quote! {
            #field : Some ( <#ty> :: new() )
        }), quote! { , });

        // timers iterator
        code.append_all(quote! {
            impl finny::AllVariants for #timers_enum_ty {
                type Iter = #timers_enum_iter_ty;

                fn iter() -> #timers_enum_iter_ty {
                    #timers_enum_iter_ty::new()
                }
            }

            pub struct #timers_enum_iter_ty {
                position: usize,
                #submachine_iter_struct
            }

            impl #timers_enum_iter_ty {
                pub fn new() -> Self {
                    Self {
                        position: 0,
                        #submachine_iter_new
                    }
                }
            }

            impl core::iter::Iterator for #timers_enum_iter_ty {
                type Item = #timers_enum_ty;

                fn next(&mut self) -> Option<Self::Item> {
                    match self.position {
                        #enum_iter_matches
                        _ => None
                    }
                }
            }
        });

        // timers storage
        let our_timers_storage: Vec<_> = our_timers.iter().map(|t| {
            let field = to_field_name(t);
            (field, t.clone())
        }).collect();
        
        let mut timers_storage_struct_fields = Vec::new();
        timers_storage_struct_fields.extend(our_timers_storage.iter().map(|(field, ty)| {
            quote! {
                #field: Option < TTimerStorage >
            }
        }));
        timers_storage_struct_fields.extend(submachines.iter().map(|s| {
            let ty = s.get_fsm_timers_storage_ty();
            let field = to_field_name(&ty);
            quote! {
                #field: #ty < TTimerStorage >
            }
        }));
        let mut fields = TokenStream::new();
        fields.append_separated(timers_storage_struct_fields, quote! { , });

        let mut new_fields_vec = vec![];
        new_fields_vec.extend(our_timers_storage.iter().map(|(field, ty)| quote! {
            #field: None
        }));
        new_fields_vec.extend(submachines.iter().map(|s| {
            let ty = s.get_fsm_timers_storage_ty();
            let field = to_field_name(&ty);
            quote! {
                #field: #ty :: default()
            }
        }));
        let mut new_fields = TokenStream::new();
        new_fields.append_separated(new_fields_vec, quote! { , });

        
        let mut timers_storage_matches = vec![];
        timers_storage_matches.extend(our_timers_storage.iter().map(|(field, ty)| {
            quote! {
                #timers_enum_ty :: #ty => &mut self. #field
            }
        }));
        timers_storage_matches.extend(submachines.iter().map(|s| {
            let ty = s.get_fsm_timers_storage_ty();
            //let t = s.get_fsm_timers_ty();
            let t = s.get_fsm_no_generics_ty();
            let field = to_field_name(&ty);
            quote! {
                #timers_enum_ty :: #t (ref sub) => {
                    self. #field .get_timer_storage_mut(sub)
                }
            }
        }));

        let matches = if timers_storage_matches.len() == 0 {
            quote! {
                panic!("Not supported in this FSM.");
            }
        } else {
            let mut m = TokenStream::new();
            m.append_separated(timers_storage_matches, quote! { , });

            quote! {
                match *id {
                    #m
                }
            }
        };

        code.append_all(quote! {

            pub struct #timers_storage_ty<TTimerStorage> {
                _storage: core::marker::PhantomData<TTimerStorage>,
                #fields
            }

            impl<TTimerStorage> core::default::Default for #timers_storage_ty<TTimerStorage> {
                fn default() -> Self {
                    Self {
                        _storage: core::marker::PhantomData::default(),
                        #new_fields
                    }
                }
            }

            impl<TTimerStorage> finny::TimersStorage<#timers_enum_ty , TTimerStorage> for #timers_storage_ty<TTimerStorage>
            {
                fn get_timer_storage_mut(&mut self, id: & #timers_enum_ty ) -> &mut Option<TTimerStorage> {
                    #matches
                }
            }

        });



        code
    };

    let fsm_meta = generate_fsm_meta(&fsm);

    let mut q = quote! {
        #states_store

        #states

        #events_enum

        #transition_types

        #dispatch

        #builder

        #timers

        #fsm_meta
    };

    /*
    // this goes in front of our definition function
    q.append_all(quote! {
        #[allow(dead_code)]
    });

    q.append_all(attr);
    q.append_all(input);
    */

    Ok(q.into())
}