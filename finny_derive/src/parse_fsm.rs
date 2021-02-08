use std::collections::HashMap;

use proc_macro2::Span;
use syn::{ExprMethodCall, ItemFn, Type, spanned::Spanned};

use crate::{parse::{EventGuardAction, FsmDeclarations, FsmEvent, FsmEventTransition, FsmFnBase, FsmState, FsmStateAction, FsmStateKind, FsmStateTransition, FsmSubMachineOptions, FsmTimer, FsmTransition, FsmTransitionEvent, FsmTransitionState, FsmTransitionType, ValidatedFsm}, parse_blocks::{FsmBlock, get_generics}, utils::{assert_no_generics, to_field_name, get_closure}, validation::create_regions};

#[derive(Copy, Clone, Debug)]
pub struct FsmCodegenOptions {
    pub event_debug: bool
}

impl FsmCodegenOptions {
    pub fn new() -> Self {
        Self {
            event_debug: false
        }
    }
}

pub struct FsmParser {
    initial_states: Vec<syn::Type>,
    states: HashMap<Type, FsmState>,
    events: HashMap<Type, FsmEvent>,
    options: FsmCodegenOptions,
    base: FsmFnBase,
    timer_id: usize
}

impl FsmParser {
    pub fn new(base: FsmFnBase) -> Self {
        FsmParser {
            initial_states: vec![],
            states: HashMap::new(),
            events: HashMap::new(),
            options: FsmCodegenOptions::new(),
            base,
            timer_id: 1
        }
    }

    pub fn parse(&mut self, _input_fn: &ItemFn, blocks: &Vec<FsmBlock>) -> syn::Result<()> {
        for block in blocks {
            match block {
                FsmBlock::MethodCall(mc) => {

                    let methods = mc.method_calls
                        .iter()
                        .map(|m| MethodOverview::parse(m))
                        .collect::<syn::Result<Vec<_>>>()?;

                    let methods: Vec<_> = methods.iter().map(|m| m.as_ref()).collect();

                    match methods.as_slice() {
                        [MethodOverviewRef { name: "build", .. } ] => {
                            
                        },
                        [MethodOverviewRef { name: "events_debug", generics: [], .. }] => {
                            self.options.event_debug = true;
                        },
                        [MethodOverviewRef { name: "initial_state", generics: [ty], .. }] => {
                            assert_no_generics(ty)?;
                            if self.initial_states.len() > 0 { return Err(syn::Error::new(ty.span(), "Duplicate initial_state!")); }
                            self.initial_states.push(ty.clone());
                        },
                        [MethodOverviewRef { name: "initial_states", generics: [ty_tuple], .. }] => {

                            if self.initial_states.len() > 0 { return Err(syn::Error::new(ty_tuple.span(), "Duplicate initial_state!")); }

                            match ty_tuple {
                                Type::Tuple(tuple) => {
                                    for ty in &tuple.elems {
                                        assert_no_generics(ty)?;
                                        self.initial_states.push(ty.clone());
                                    }
                                }
                                _ => { return Err(syn::Error::new(ty_tuple.span(), "Expected a tuple of states!")); }
                            }
                        },

                        [MethodOverviewRef { name: "sub_machine", generics: [ty_sub_fsm], ..}, st @ .. ] => {

                            //assert_no_generics(ty_sub_fsm)?;
                            let field_name = to_field_name(&ty_sub_fsm);
                            
                            let state = self.states
                                .entry(ty_sub_fsm.clone())
                                .or_insert(FsmState {
                                    ty: ty_sub_fsm.clone(),
                                    state_storage_field: field_name,
                                    on_entry_closure: None,
                                    on_exit_closure: None,
                                    kind: FsmStateKind::SubMachine(FsmSubMachineOptions::default()),
                                    timers: vec![]
                                });
                            let mut sub_options = match state.kind {                                
                                FsmStateKind::SubMachine(ref sub) => sub.clone(),
                                _ => { return Err(syn::Error::new(ty_sub_fsm.span(), "Internal error with sub machines.")); }
                            };

                            match st {
                                [with_context @ MethodOverviewRef { name: "with_context", .. }, st @ .. ] => {
                                    let closure = get_closure(&with_context.call)?;
                                    if sub_options.context_constructor.is_some() {
                                        return Err(syn::Error::new(closure.span(), "Duplicate constructor for the context!"));
                                    }
                                    sub_options.context_constructor = Some(closure.clone());

                                    self.state_builder_parser(&ty_sub_fsm, st, true)?;
                                },
                                [st @ ..] => {
                                    self.state_builder_parser(&ty_sub_fsm, st, true)?;
                                },
                                _ => { return Err(syn::Error::new(ty_sub_fsm.span(), "Missing with_context?")); }
                            }                          

                            // update the options
                            self.states.entry(ty_sub_fsm.clone()).and_modify(|s| {
                                s.kind = FsmStateKind::SubMachine(sub_options);
                            });
                            
                        },

                        [MethodOverviewRef { name: "state", generics: [ty_state], .. }, st @ .. ] => {

                            self.state_builder_parser(ty_state, st, false)?;
                            
                        },

                        _ => { return Err(syn::Error::new(mc.expr_call.span(), "Unsupported method.")); }
                    }

                },
                _ => todo!("unsupported block!")
            }
        }

        Ok(())
    }

    fn parse_event_guard_action(event_method_calls: &[MethodOverviewRef]) -> syn::Result<EventGuardAction> {
        let mut guard_action = EventGuardAction { guard: None, action: None, type_hint: None };
        
        for method in event_method_calls {
            match method {
                MethodOverviewRef { name: "guard", .. } => {
                    let closure = get_closure(method.call)?;

                    if guard_action.guard.is_some() {
                        return Err(syn::Error::new(closure.span(), "Duplicate 'guard'!"));
                    }

                    guard_action.guard = Some(closure.clone());
                },
                MethodOverviewRef { name: "action", .. } => {
                    let closure = get_closure(method.call)?;

                    if guard_action.action.is_some() {
                        return Err(syn::Error::new(closure.span(), "Duplicate 'action'!"));
                    }

                    guard_action.action = Some(closure.clone());
                },
                MethodOverviewRef { name: "with_transition_ty", generics: [transition_ty], ..}  => {

                    if guard_action.type_hint.is_some() {
                        return Err(syn::Error::new(method.call.span(), "Duplicate 'with_transition_ty'!"));
                    }

                    guard_action.type_hint = Some(transition_ty.clone());

                },
                _ => { return Err(syn::Error::new(method.call.span(), "Unsupported method.")); }
            }
        }

        Ok(guard_action)
    }

    fn parse_state_on_event(state: &FsmState, event: &mut FsmEvent, method_calls: &[MethodOverviewRef]) -> syn::Result<()> {
        match method_calls {
            [MethodOverviewRef { name: "transition_to", generics: [ty_to], .. }, ev @ .. ] => {
                event.transitions.push(FsmEventTransition::State(state.ty.clone(), ty_to.clone(), Self::parse_event_guard_action(ev)?));                
            },
            [MethodOverviewRef { name: "internal_transition", generics: [], ..}, ev @ ..] => {
                event.transitions.push(FsmEventTransition::InternalTransition(state.ty.clone(), Self::parse_event_guard_action(ev)?));
            },
            [MethodOverviewRef { name: "self_transition", generics: [], ..}, ev @ ..] => {
                event.transitions.push(FsmEventTransition::SelfTransition(state.ty.clone(), Self::parse_event_guard_action(ev)?));
            },
            [] => (),
            _ => { return Err(syn::Error::new(method_calls.first().map(|m| m.call.span()).unwrap_or(Span::call_site()), "Unsupported methods.")); }
        }

        Ok(())
    }

    pub fn validate(mut self, input_fn: &ItemFn) -> syn::Result<ValidatedFsm> {
        let mut transitions = vec![];

        if self.initial_states.len() == 0 {
            return Err(syn::Error::new(input_fn.span(), "Missing the initial state declaration! Use the method 'initial_state' or 'initial_states'."));
        }
        
        // build and validate the transitions table
        {
            let mut i = 0;

            fn generate_transition_ty(base: &FsmFnBase, i: &mut usize, ty_hint: &Option<syn::Type>) -> syn::Type {
                if let Some(type_hint) = ty_hint {
                    type_hint.clone()
                } else {
                    *i = *i + 1;
                    crate::utils::ty_append(&base.fsm_ty, &format!("Transition{}", i))
                }
            }

            // start transition
            for initial_state in &self.initial_states {
                let fsm_initial_state = self.states.get(&initial_state).ok_or(syn::Error::new(initial_state.span(), "The initial state is not refered in the builder. Use the 'state' method on the builder."))?;

                transitions.push(FsmTransition {
                    transition_ty: generate_transition_ty(&self.base, &mut i, &None),
                    ty: FsmTransitionType::StateTransition(FsmStateTransition {
                        action: EventGuardAction::default(),
                        event: FsmTransitionEvent::Start,
                        state_from: FsmTransitionState::None,
                        state_to: FsmTransitionState::State(fsm_initial_state.clone())
                    })
                });
            }

            for (ty, ev) in self.events.iter() {
                for t in &ev.transitions {
                    match t {
                        FsmEventTransition::State(from, to, action) => {

                            let from = self.states.get(from).ok_or(syn::Error::new(from.span(), "State not found."))?;
                            let to = self.states.get(to).ok_or(syn::Error::new(to.span(), "State not found."))?;

                            transitions.push(FsmTransition {
                                transition_ty: generate_transition_ty(&self.base, &mut i, &action.type_hint),
                                ty: FsmTransitionType::StateTransition(FsmStateTransition {
                                    action: action.clone(),
                                    state_from: FsmTransitionState::State(from.clone()),
                                    state_to: FsmTransitionState::State(to.clone()),
                                    event: FsmTransitionEvent::Event(ev.clone())
                                })
                            });
                        }
                        FsmEventTransition::InternalTransition(state, action) => {
                            // todo: code duplication!
                            let state = self.states.get(state).ok_or(syn::Error::new(state.span(), "State not found."))?;
                            transitions.push(FsmTransition {
                                transition_ty: generate_transition_ty(&self.base, &mut i, &action.type_hint),
                                ty: FsmTransitionType::InternalTransition(FsmStateAction {
                                    state: FsmTransitionState::State(state.clone()),
                                    action: action.clone(),
                                    event: FsmTransitionEvent::Event(ev.clone())
                                })
                            });
                        }
                        FsmEventTransition::SelfTransition(state, action) => {
                            // todo: code duplication!
                            let state = self.states.get(state).ok_or(syn::Error::new(state.span(), "State not found."))?;
                            transitions.push(FsmTransition {
                                transition_ty: generate_transition_ty(&self.base, &mut i, &action.type_hint),
                                ty: FsmTransitionType::SelfTransition(FsmStateAction {
                                    state: FsmTransitionState::State(state.clone()),
                                    action: action.clone(),
                                    event: FsmTransitionEvent::Event(ev.clone())
                                })
                            });
                        }
                    }
                }
            }
        }
                
        let dec = FsmDeclarations {
            initial_states: self.initial_states,
            states: self.states,
            events: self.events,
            transitions
        };

        let regions = create_regions(dec, self.options)?;

        Ok(regions)
    }

    fn state_builder_parser(&mut self, ty_state: &syn::Type, st: &[MethodOverviewRef], is_sub_fsm: bool) -> syn::Result<()> {
        if !is_sub_fsm { assert_no_generics(ty_state)?; }
        let field_name = to_field_name(&ty_state);
        let state = self.states
            .entry(ty_state.clone())
            .or_insert(FsmState { 
                ty: ty_state.clone(),
                on_entry_closure: None,
                on_exit_closure: None,
                state_storage_field: field_name,
                kind: FsmStateKind::Normal,
                timers: vec![]
            });

            
        let mut timer = None;

        for (i, method) in st.iter().enumerate() {
            match method {
                MethodOverviewRef { name: "on_entry", .. } => {
                    let closure = get_closure(&method.call)?;

                    if state.on_entry_closure.is_some() {
                        return Err(syn::Error::new(closure.span(), "Duplicate 'on_entry'!"));
                    }
                    state.on_entry_closure = Some(closure.clone());
                },
                MethodOverviewRef { name: "on_exit", .. } => {
                    let closure = get_closure(&method.call)?;

                    if state.on_exit_closure.is_some() {
                        return Err(syn::Error::new(closure.span(), "Duplicate 'on_exit'!"));
                    }
                    state.on_exit_closure = Some(closure.clone());
                },
                MethodOverviewRef { name: "on_event", generics: [ty_event], .. } => {
                    assert_no_generics(ty_event)?;

                    let event = self.events
                        .entry(ty_event.clone())
                        .or_insert(FsmEvent { ty: ty_event.clone(), transitions: vec![] });

                    let other_method_calls = &st[(i+1)..];
                    Self::parse_state_on_event(state, event, other_method_calls)?;

                    break;
                },
                MethodOverviewRef { name: "on_entry_start_timer", generics: [], .. } => {

                    let call_args: Vec<_> = method.call.args.iter().collect();
                    match call_args.as_slice() {
                        [syn::Expr::Closure(ref setup), syn::Expr::Closure(ref trigger)] => {

                            if timer.is_some() { panic!("double timer bug!"); }
                            
                            timer = Some(FsmTimer {
                                setup: setup.clone(),
                                trigger: trigger.clone(),
                                id: self.timer_id,
                                type_hint: None
                            });                           
                            
                            self.timer_id += 1;
                            
                        },
                        _ => {
                            return Err(syn::Error::new(method.call.span(), "Unexpected arguments to the timer setup method."));
                        }
                    }
                },

                MethodOverviewRef { name: "with_timer_ty", generics: [timer_ty], .. } => {

                    if let Some(ref mut timer) = timer {
                        timer.type_hint = Some(timer_ty.clone());
                    } else {
                        return Err(syn::Error::new(method.call.span(), "Missing the timer to apply the type to."));
                    }

                },

                _ => { return Err(syn::Error::new(method.call.span(), format!("Unsupported method '{}'!", method.name))); }
            }
        }

        if let Some(timer) = timer {
            state.timers.push(timer);
        }

        Ok(())
    }    
}


struct MethodOverview {
    name: String,
    generics: Vec<syn::Type>,
    call: ExprMethodCall
}

impl MethodOverview {
    pub fn parse(m: &ExprMethodCall) -> syn::Result<Self> {
        let generics = get_generics(&m.turbofish)?;

        Ok(Self {
            name: m.method.to_string(),
            generics,
            call: m.clone()
        })
    }

    pub fn as_ref(&self) -> MethodOverviewRef {
        MethodOverviewRef {
            name: self.name.as_str(),
            generics: self.generics.as_slice(),
            call: &self.call
        }
    }
}

#[derive(Clone)]
struct MethodOverviewRef<'a> {
    name: &'a str,
    generics: &'a [syn::Type],
    call: &'a ExprMethodCall
}