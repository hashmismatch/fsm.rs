use proc_macro2::{Span, TokenStream};
use quote::{TokenStreamExt, quote};
use syn::spanned::Spanned;
use utils::remap_closure_inputs;

use crate::{parse::{FsmEvent, FsmFnInput, FsmTransitionState}, utils::{tokens_to_string, ty_append}};



pub fn generate_fsm_code(fsm: &FsmFnInput, attr: TokenStream, input: TokenStream) -> syn::Result<TokenStream> {
    let fsm_ty = &fsm.base.fsm_ty;
    let ctx_ty = &fsm.base.context_ty;

    let states_store_ty = ty_append(&fsm.base.fsm_ty, "States");
    let states_enum_ty = ty_append(&fsm.base.fsm_ty, "CurrentState");
    let event_enum_ty = ty_append(&fsm.base.fsm_ty, "Events");

    let states_store = {

        let mut code_fields = TokenStream::new();
        let mut new_state_fields = TokenStream::new();
        let mut state_variants = TokenStream::new();

        for (i, (_, state)) in fsm.decl.states.iter().enumerate() {
            let name = &state.state_storage_field;
            let ty = &state.ty;
            code_fields.append_all(quote! { #name: #ty, });
            new_state_fields.append_all(quote! { #name: #ty::new_state(context)?, });
            state_variants.append_all(quote!{ #ty, });
        }

        quote! {
            pub struct #states_store_ty {
                #code_fields
            }

            impl crate::fsm_core::FsmStateFactory for #states_store_ty {
                fn new_state<C>(context: &C) -> crate::fsm_core::FsmResult<Self> {
                    let s = Self {
                        #new_state_fields
                    };
                    Ok(s)
                }
            }

            #[derive(Copy, Clone, Debug, PartialEq)]
            pub enum #states_enum_ty {
                #state_variants
            }

            impl crate::fsm_core::FsmStates for #states_store_ty {
                type StateKind = #states_enum_ty;
            }
        }
    };

    let events_enum = {

        let mut variants = TokenStream::new();
        for (ty, ev) in  fsm.decl.events.iter() {
            variants.append_all(quote! { #ty ( #ty ),  });
        }
        
        let evs = quote! {
            #[derive(fsm_core::bundled::derive_more::TryInto)]
            pub enum #event_enum_ty {
                #variants
            }
        };

        evs
    };

    let transition_types = {
        let mut t = TokenStream::new();
        
        for transition in &fsm.decl.transitions {

            let ty = &transition.transition_ty;
            let mut q = quote! {
                pub struct #ty;
            };

            match transition.event {
                crate::parse::FsmTransitionEvent::Event(ref ev) => {
                    let event_ty = &ev.ty;

                    if let Some(ref guard) = ev.guard {
                        let remap = remap_closure_inputs(&guard.inputs, vec![
                            quote! { event }, quote! { context }
                        ].as_slice())?;

                        let body = &guard.body;

                        let g = quote! {
                            impl crate::fsm_core::FsmTransitionGuard<#fsm_ty, #event_ty> for #ty {
                                fn guard<'a, Q>(event: & #event_ty, context: &crate::fsm_core::EventContext<'a, #fsm_ty, Q>) -> bool {
                                    #remap
                                    let result = { #body };
                                    result
                                }
                            }
                        };

                        q.append_all(g);
                    }

                    if let Some(ref action) = ev.action {
                        let remap = remap_closure_inputs(&action.inputs, vec![
                            quote! { event }, quote! { context }, quote! { from }, quote! { to }
                        ].as_slice())?;

                        let body = &action.body;
                        let state_from = transition.from.get_fsm_state()?;
                        let state_to = transition.to.get_fsm_state()?;                        

                        let state_from_ty = &state_from.ty;
                        let state_to_ty = &state_to.ty;
                        let body = &action.body;
                        let a = quote! {
                            impl crate::fsm_core::FsmTransitionAction<#fsm_ty, #event_ty, #state_from_ty, #state_to_ty> for #ty {
                                fn action<'a, Q>(event: & #event_ty , context: &mut crate::fsm_core::EventContext<'a, #fsm_ty, Q >, from: &mut #state_from_ty, to: &mut #state_to_ty) {
                                    #remap
                                    { #body }
                                }
                            }
                        };

                        q.append_all(a);
                    }

                    
                },
                _ => ()
            }

            t.append_all(q);
        }

        t
    };

    let dispatch = {        
        let mut transition_match = TokenStream::new();
        for transition in &fsm.decl.transitions {

            let transition_ty = &transition.transition_ty;
            
            let match_state = match transition.from {
                crate::parse::FsmTransitionState::None => quote! { crate::fsm_core::FsmCurrentState::Stopped },
                crate::parse::FsmTransitionState::State(ref st) => {
                    let kind = &st.ty;
                    quote! { crate::fsm_core::FsmCurrentState::State(#states_enum_ty :: #kind) }
                }
            };

            let match_event = match transition.event {
                crate::parse::FsmTransitionEvent::Start => quote! { crate::fsm_core::FsmEvent::Start },
                crate::parse::FsmTransitionEvent::Event(ref ev) => {
                    let kind = &ev.ty;
                    quote! { crate::fsm_core::FsmEvent::Event(#event_enum_ty::#kind(ref ev)) }
                }
                _ => todo!()
            };

            let state_to = match transition.to {
                crate::parse::FsmTransitionState::None => todo!("transition to"),
                crate::parse::FsmTransitionState::State(ref st) => {
                    let field = &st.state_storage_field;
                    let ty = &st.ty;
                    quote! {
                        let state_to = &mut backend.states. #field ;
                        <#ty>::on_entry(state_to, &mut context);
                    }
                }
            };

            let state_from = match transition.from {
                crate::parse::FsmTransitionState::None => TokenStream::new(),
                crate::parse::FsmTransitionState::State(ref st) => {
                    let field = &st.state_storage_field;
                    let ty = &st.ty;
                    quote! {
                        let state_from = &mut backend.states. #field;
                        <#ty>::on_exit(state_from, &mut context);
                    }
                }
            };

            let current_state_update = match transition.to {
                crate::parse::FsmTransitionState::None => {
                    let q = quote! {
                        backend.current_state = crate::fsm_core::FsmCurrentState::None;
                    };
                    q
                },
                crate::parse::FsmTransitionState::State(ref st) => {
                    let kind = &st.ty;
                    let q = quote! {
                        backend.current_state = crate::fsm_core::FsmCurrentState::State(#states_enum_ty :: #kind);
                    };
                    q
                }
            };

            let guard = match transition.event {
                crate::parse::FsmTransitionEvent::Event(FsmEvent { guard: Some(ref guard), .. }) => {
                    quote! {
                        if <#transition_ty>::guard(ev, &context)
                    }
                },
                _ => TokenStream::new()
            };

            let action = match transition.event {
                crate::parse::FsmTransitionEvent::Event(FsmEvent { action: Some(ref action), .. }) => {

                    let state_from = transition.from.get_fsm_state()?;
                    let state_to = transition.to.get_fsm_state()?;

                    let state_from_field = &state_from.state_storage_field;
                    let state_to_field = &state_to.state_storage_field;

                    quote! {
                        <#transition_ty>::action(ev, &mut context,
                            &mut backend.states. #state_from_field,
                            &mut backend.states. #state_to_field
                        );
                    }
                },
                _ => TokenStream::new()
            };

            let m = quote! {
                ( #match_state , #match_event ) #guard => {
                    { #state_from }

                    { #action }

                    { #state_to }

                    { #current_state_update }
                },
            };

            transition_match.append_all(m);
        }

        quote! {
              
            impl crate::fsm_core::FsmBackend for #fsm_ty
            {
                type Context = #ctx_ty;
                type States = #states_store_ty;
                type Events = #event_enum_ty;

                fn dispatch_event<Q>(backend: &mut crate::fsm_core::FsmBackendImpl<Self>, event: &FsmEvent<Self::Events>, queue: &mut Q) -> crate::fsm_core::FsmResult<()>
                    where Q: crate::fsm_core::FsmEventQueue<<Self as crate::fsm_core::FsmBackend>::Events>
                {
                    use crate::fsm_core::{FsmTransitionGuard, FsmTransitionAction, FsmState};

                    let mut context = crate::fsm_core::EventContext::<#fsm_ty, Q> {
                        context: &mut backend.context,
                        queue
                    };
                    
                    match (&backend.current_state, event) {
                        
                        #transition_match

                        _ => {
                            return Err(crate::fsm_core::FsmError::NoTransition);
                        }
                    }

                    Ok(())
                }
            }
        }
    };

    let states = {

        let mut states = TokenStream::new();
        for (ty, state) in fsm.decl.states.iter() {

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

            states.append_all(quote! {

                impl crate::fsm_core::FsmState<#fsm_ty> for #ty {
                    fn on_entry<'a, Q: crate::fsm_core::FsmEventQueue<<#fsm_ty as crate::fsm_core::FsmBackend>::Events>>(&mut self, context: &mut crate::fsm_core::EventContext<'a, #fsm_ty, Q>) {
                        #on_entry
                    }

                    fn on_exit<'a, Q: crate::fsm_core::FsmEventQueue<<#fsm_ty as crate::fsm_core::FsmBackend>::Events>>(&mut self, context: &mut crate::fsm_core::EventContext<'a, #fsm_ty, Q>) {
                        #on_exit
                    }
                }

            })
        }

        states
    };

    let builder = {

        let initial_state = &fsm.decl.initial_state;

        quote! {

            pub struct #fsm_ty {
                backend: crate::fsm_core::FsmBackendImpl<#fsm_ty>
            }
            
        }
    };

    let mut q = quote! {
        #states_store

        #states

        #events_enum

        #transition_types

        #dispatch

        #builder
    };

    // this goes in front of our definition function
    q.append_all(quote! {
        #[allow(dead_code)]
    });

    q.append_all(attr);
    q.append_all(input);

    Ok(q.into())
}