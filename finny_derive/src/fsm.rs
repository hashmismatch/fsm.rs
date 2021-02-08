use crate::utils::{strip_generics, ty_append};

#[derive(Clone)]
pub struct FsmTypes {
    fsm: syn::Type,
    fsm_no_generics: syn::Type,
    generics: syn::Generics
}

impl FsmTypes {
    pub fn new(ty: &syn::Type, generics: &syn::Generics) -> Self {
        Self {
            fsm: ty.clone(),
            fsm_no_generics: strip_generics(ty.clone()),
            generics: generics.clone()
        }
    }

    pub fn get_fsm_ty(&self) -> &syn::Type {
        &self.fsm
    }

    pub fn get_fsm_no_generics_ty(&self) -> &syn::Type {
        &self.fsm_no_generics
    }

    pub fn get_fsm_events_ty(&self) -> syn::Type {
        ty_append(&self.fsm_no_generics, "Events")
    }

    pub fn get_fsm_timers_ty(&self) -> syn::Type {
        ty_append(&self.fsm_no_generics, "Timers")
    }

    pub fn get_fsm_timers_iter_ty(&self) -> syn::Type {
        ty_append(&self.fsm_no_generics, "TimersIter")
    }

    pub fn get_fsm_timers_storage_ty(&self) -> syn::Type {
        ty_append(&self.fsm_no_generics, "TimersStorage")
    }    
}