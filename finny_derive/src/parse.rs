use std::collections::{HashMap, HashSet};

use proc_macro2::{Span, TokenStream};
use syn::{Error, Expr, ExprMethodCall, GenericArgument, ItemFn, parse::{self, Parse, ParseStream}, spanned::Spanned};

use crate::{parse_blocks::{FsmBlock, decode_blocks, get_generics, get_method_receiver_ident}, parse_fsm::{FsmCodegenOptions, FsmParser}, utils::{assert_no_generics, get_closure, to_field_name, ty_append}};


pub struct FsmFnInput {
    pub base: FsmFnBase,
    pub fsm: ValidatedFsm,
}

#[derive(Debug, Clone)]
pub struct FsmFnBase {
    pub context_ty: syn::Type,
    pub fsm_ty: syn::Type,
    pub fsm_info_ty: syn::Type,
    pub builder_ident: proc_macro2::Ident,
    pub fsm_generics: syn::Generics
}


impl FsmFnInput {
    pub fn parse(attr: TokenStream, item: TokenStream) -> syn::Result<Self> {
        let input_fn: syn::ItemFn = syn::parse2(item)?;

        // builder name/generics
        let (builder_ident, fsm_ty, context_ty) = {
            let input_fsm_builder = match (input_fn.sig.inputs.len(), input_fn.sig.inputs.first()) {
                (1, Some(p)) => {
                    Ok(p)
                },
                (_, _) => {
                    Err(Error::new(input_fn.sig.inputs.span(), "Only a single input parameter is supported!"))
                }
            }?;

            let builder_input = match input_fsm_builder {                
                syn::FnArg::Typed(pt) => Ok(pt),
                _ => Err(Error::new(input_fsm_builder.span(), "Only a typed input is supported!"))
            }?;

            let builder_input_pat_ident = match *builder_input.pat {
                syn::Pat::Ident(ref pi) => Ok(pi),
                _ => Err(Error::new(builder_input.pat.span(), "Only a type ascripted input arg is supported!"))
            }?;
            
            let builder_input_type = match *builder_input.ty {
                syn::Type::Path(ref type_path) => Ok(type_path),
                _ => Err(Error::new(builder_input.ty.span(), "The builder's type is incorrect!"))
            }?;

            let path_segment = match (builder_input_type.path.segments.len(), builder_input_type.path.segments.first()) {
                (1, Some(s)) => Ok(s),
                (_, _) => Err(Error::new(builder_input_type.path.segments.span(), "Only one segment is supported!"))
            }?;

            let generic_arguments = match &path_segment.arguments {
                syn::PathArguments::AngleBracketed(g) => Ok(g),
                _ => Err(Error::new(path_segment.arguments.span(), "Only one segment is supported!"))
            }?;


            let generic_tys: Vec<_> = generic_arguments.args.iter().collect();

            let (fsm_ty, context_ty) = match (generic_tys.get(0), generic_tys.get(1)) {
                (Some(GenericArgument::Type(fsm_ty)), Some(GenericArgument::Type(context_ty))) => {
                    Ok((fsm_ty, context_ty))
                },
                _ => Err(Error::new(generic_arguments.args.span(), "Expected a pair of generic arguments!"))
            }?;

            // remove the generics
            let fsm_ty = {
                let mut fsm_ty = fsm_ty.clone();
                match fsm_ty {
                    syn::Type::Path(ref mut tp) => {
                        let seg = tp.path.segments.first_mut().unwrap();
                        seg.arguments = syn::PathArguments::None;
                    },
                    _ => { return Err(syn::Error::new(fsm_ty.span(), "Unsupported FSM type.")); }
                }

                fsm_ty
            };

            (builder_input_pat_ident.ident.clone(), fsm_ty, context_ty.clone())
        };


        // return type check
        {
            let output_ty = match input_fn.sig.output {
                syn::ReturnType::Type(_, ref ty) => Ok(ty),
                _ => Err(syn::Error::new(input_fn.sig.output.span(), "The return type has to be 'BuiltFsm'!"))
            }?;

            let tp = match **output_ty {
                syn::Type::Path(ref tp) => Ok(tp),
                _ => Err(syn::Error::new(output_ty.span(), "The return type has to be 'BuiltFsm'!"))
            }?;

            match tp.path.get_ident() {
                Some(ident) if ident == "BuiltFsm" => Ok(()),
                _ => Err(syn::Error::new(tp.path.span(), "The return type has to be 'BuiltFsm'!"))
            }?
        }

        let base = FsmFnBase {
            builder_ident,
            context_ty,
            fsm_info_ty: crate::utils::ty_append(&fsm_ty, "Info"),
            fsm_ty,            
            fsm_generics: input_fn.sig.generics.clone()
        };

        let blocks = decode_blocks(&base, &input_fn)?;

        let fsm_declarations = FsmDeclarations::parse(&base, &input_fn, &blocks)?;

        Ok(FsmFnInput {
            base,
            fsm: fsm_declarations
        })
    }
}

#[derive(Debug)]
pub struct FsmDeclarations {
    pub initial_states: Vec<syn::Type>,
    pub states: HashMap<syn::Type, FsmState>,
    pub events: HashMap<syn::Type, FsmEvent>,
    pub transitions: Vec<FsmTransition>
}

#[derive(Debug)]
pub struct ValidatedFsm {
    pub codegen_options: FsmCodegenOptions,
    pub regions: Vec<FsmRegion>,
    pub states: HashMap<syn::Type, FsmState>,
    pub events: HashMap<syn::Type, FsmEvent>
}

#[derive(Debug)]
pub struct FsmRegion {
    pub region_id: usize,
    pub initial_state: syn::Type,
    pub transitions: Vec<FsmTransition>,
    pub states: Vec<FsmState>
}

#[derive(Debug, Clone)]
pub enum FsmTransitionState {
    None,
    State(FsmState)
}

#[derive(Debug, Clone)]
pub enum FsmTransitionEvent {
    Stop,
    Start,
    Event(FsmEvent)
}

impl FsmTransitionEvent {
    pub fn get_event(&self) -> syn::Result<&FsmEvent> {
        match self {            
            FsmTransitionEvent::Event(ev) => Ok(ev),
            _ => Err(syn::Error::new(Span::call_site(), "Missing event here, codegen bug!"))
        }
    }
}

#[derive(Debug, Clone)]
pub struct FsmTransition {
    pub ty: FsmTransitionType,
    pub transition_ty: syn::Type
}
#[derive(Debug, Clone)]
pub enum FsmTransitionType {
    /// Doesn't trigger the state's actions
    InternalTransition(FsmStateAction),
    /// Triggers the state's actions
    SelfTransition(FsmStateAction),
    /// From State A to State B
    StateTransition(FsmStateTransition)
}

impl FsmTransitionType {
    pub fn get_states(&self) -> Vec<syn::Type> {
        let mut ret = vec![];

        match self {
            FsmTransitionType::InternalTransition(s) | FsmTransitionType::SelfTransition(s) => {
                if let Ok(s) = s.state.get_fsm_state() {
                    ret.push(s.ty.clone());
                }
            },
            FsmTransitionType::StateTransition(s) => {
                
                let from = s.state_from.get_fsm_state();
                let to = s.state_to.get_fsm_state();

                if let Ok(from) = from { ret.push(from.ty.clone()); }
                if let Ok(to) = to { ret.push(to.ty.clone()); }
            }
        }

        ret
    }
}

#[derive(Debug, Clone)]
pub struct FsmStateAction {
    pub state: FsmTransitionState,
    pub event: FsmTransitionEvent,
    pub action: EventGuardAction
}

#[derive(Debug, Clone)]
pub struct FsmStateTransition {
    pub state_from: FsmTransitionState,
    pub state_to: FsmTransitionState,
    pub action: EventGuardAction,
    pub event: FsmTransitionEvent,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FsmStateKind {
    Normal,
    SubMachine(FsmSubMachineOptions)
}

#[derive(Default, Debug, Clone, PartialEq)]
pub struct FsmSubMachineOptions {
    pub context_constructor: Option<syn::ExprClosure>
}

#[derive(Debug, Clone)]
pub struct FsmState {
    pub ty: syn::Type,
    pub kind: FsmStateKind,
    pub state_storage_field: syn::Ident,
    pub on_entry_closure: Option<syn::ExprClosure>,
    pub on_exit_closure: Option<syn::ExprClosure>,
    pub timers: Vec<FsmTimer>
}

#[derive(Debug, Clone)]
pub struct FsmTimer {
    pub id: usize,
    pub setup: syn::ExprClosure,
    pub trigger: syn::ExprClosure,
    pub type_hint: Option<syn::Type>
}

impl FsmTimer {
    pub fn get_ty(&self, fsm: &FsmFnBase) -> syn::Type {
        if let Some(ref type_hint) = self.type_hint {
            type_hint.clone()
        } else {
            ty_append(&fsm.fsm_ty, &format!("Timer{}", self.id))
        }
    }

    pub fn get_field(&self, fsm: &FsmFnBase) -> syn::Ident {
        to_field_name(&self.get_ty(fsm))
    }
}

#[derive(Debug, Clone)]
pub struct FsmEvent {
    pub ty: syn::Type,
    pub transitions: Vec<FsmEventTransition>
}

#[derive(Debug, Clone)]
pub enum FsmEventTransition {
    /// A transition from one state to another.
    State(syn::Type, syn::Type, EventGuardAction),
    /// Triggers the state's exit/enter actions
    InternalTransition(syn::Type, EventGuardAction),
    /// Triggers the state's exit/enter actions
    SelfTransition(syn::Type, EventGuardAction)
}

#[derive(Default, Debug, Clone)]
pub struct EventGuardAction{
    pub guard: Option<syn::ExprClosure>,
    pub action: Option<syn::ExprClosure>,
    pub type_hint: Option<syn::Type>
}

impl FsmDeclarations {
    pub fn parse(base: &FsmFnBase, input_fn: &ItemFn, blocks: &Vec<FsmBlock>) -> syn::Result<ValidatedFsm> {
        let mut parser = FsmParser::new(base.clone());
        parser.parse(input_fn, blocks)?;
        return parser.validate(input_fn);
    }
}
