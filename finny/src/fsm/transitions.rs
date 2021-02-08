//! All of these traits will be implemented by the procedural code generator.

use crate::{FsmBackendImpl, FsmDispatchResult, FsmEventQueueSub, FsmTimers, FsmTimersSub, lib::*};
use crate::{DispatchContext, EventContext, FsmBackend, FsmCurrentState, FsmEvent, FsmEventQueue, FsmRegionId, FsmStateTransitionAsMut, FsmStates, Inspect};

/// A state's entry and exit actions.
pub trait FsmState<F: FsmBackend> {
    /// Action that is executed whenever this state is being entered.
    fn on_entry<'a, Q: FsmEventQueue<F>>(&mut self, context: &mut EventContext<'a, F, Q>);
    /// Action that is executed whenever this state is being exited.
    fn on_exit<'a, Q: FsmEventQueue<F>>(&mut self, context: &mut EventContext<'a, F, Q>);

    fn execute_on_entry<'a, 'b, 'c, 'd, Q, I, T>(context: &'d mut DispatchContext<'a, 'b, 'c, F, Q, I, T>, region: FsmRegionId) 
        where Q: FsmEventQueue<F>, I: Inspect, <F as FsmBackend>::States: AsMut<Self>, T: FsmTimers<F>
    {
        let mut event_context = EventContext {
            context: &mut context.backend.context,
            region,
            queue: context.queue
        };

        let state: &mut Self = context.backend.states.as_mut();
        state.on_entry(&mut event_context);
    }

    fn execute_on_exit<'a, 'b, 'c, 'd, Q, I, T>(context: &'d mut DispatchContext<'a, 'b, 'c, F, Q, I, T>, region: FsmRegionId) 
        where Q: FsmEventQueue<F>, I: Inspect, <F as FsmBackend>::States: AsMut<Self>, T: FsmTimers<F>
    {
        let mut event_context = EventContext {
            context: &mut context.backend.context,
            queue: context.queue,
            region
        };

        let state: &mut Self = context.backend.states.as_mut();
        state.on_exit(&mut event_context);
    }

    fn fsm_state() -> <<F as FsmBackend>::States as FsmStates<F>>::StateKind;
}

/// Check if this transition is allowed to be entered.
pub trait FsmTransitionGuard<F: FsmBackend, E> {
    /// Return a boolean value whether this transition is usable at the moment. The check shouln't mutate any structures.
    fn guard<'a, Q: FsmEventQueue<F>>(event: &E, context: &EventContext<'a, F, Q>, states: &'a <F as FsmBackend>::States) -> bool;

    fn execute_guard<'a, 'b, 'c, 'd, Q: FsmEventQueue<F>, I, T>(context: &'d mut DispatchContext<'a, 'b, 'c, F, Q, I, T>, event: &E, region: FsmRegionId, inspect_event_ctx: &mut I) -> bool
        where I: Inspect, Self: Sized, T: FsmTimers<F>
    {
        let event_context = EventContext {
            context: &mut context.backend.context,
            queue: context.queue,
            region
        };

        let guard_result = Self::guard(event, &event_context, &context.backend.states);

        inspect_event_ctx.on_guard::<Self>(guard_result);

        guard_result
    }
}


/// The transition that starts the machine, triggered using the `start()` method.
pub trait FsmTransitionFsmStart<F: FsmBackend, TInitialState> {
    fn execute_transition<'a, 'b, 'c, 'd, Q: FsmEventQueue<F>, I, T>(context: &'d mut DispatchContext<'a, 'b, 'c, F, Q, I, T>, 
        _fsm_event: &FsmEvent<<F as FsmBackend>::Events, <F as FsmBackend>::Timers>,
        region: FsmRegionId,
        inspect_event_ctx: &mut I)
        where
            I: Inspect,
            TInitialState: FsmState<F>,
            <F as FsmBackend>::States: AsMut<TInitialState>,
            <F as FsmBackend>::States: AsRef<TInitialState>,
            Self: Sized,
            T: FsmTimers<F>
    {
        let ctx = inspect_event_ctx.for_transition::<Self>();
        ctx.on_state_enter::<TInitialState>();
        
        <TInitialState>::execute_on_entry(context, region);
        
        let cs = context.backend.current_states.as_mut();
        cs[region] = FsmCurrentState::State(<TInitialState>::fsm_state());
    }

    /// Executed after the transition on the parent FSM (F) and triggers the first `start()` call if necessary. Subsequent
    /// dispatches are handled using the main dispatch table.
    fn execute_on_sub_entry<'a, 'b, 'c, 'd, Q, I, T>(context: &'d mut DispatchContext<'a, 'b, 'c, F, Q, I, T>, _region: FsmRegionId, inspect_event_ctx: &mut I) 
        -> FsmDispatchResult
        where
        TInitialState: FsmBackend,
            Q: FsmEventQueue<F>,
            I: Inspect,
            <F as FsmBackend>::Events: From<<TInitialState as FsmBackend>::Events>,
            <F as FsmBackend>::States: AsMut<TInitialState>,
            TInitialState: DerefMut<Target = FsmBackendImpl<TInitialState>>,
            T: FsmTimers<F>,
            <F as FsmBackend>::Timers: From<<TInitialState as FsmBackend>::Timers>
    {
        let sub_backend: &mut TInitialState = context.backend.states.as_mut();
        let states = sub_backend.get_current_states();
        if FsmCurrentState::all_stopped(states.as_ref()) {
            let mut queue_adapter = FsmEventQueueSub {
                parent: context.queue,
                _parent_fsm: PhantomData::<F>::default(),
                _sub_fsm: PhantomData::<TInitialState>::default()
            };

            let mut timers_adapter = FsmTimersSub {
                parent: context.timers,
                _parent_fsm: core::marker::PhantomData::<F>::default(),
                _sub_fsm: core::marker::PhantomData::<TInitialState>::default()
            };

            let mut inspect = inspect_event_ctx.for_sub_machine::<TInitialState>();

            let sub_dispatch_context = DispatchContext {
                backend: sub_backend,
                inspect: &mut inspect,
                queue: &mut queue_adapter,
                timers: &mut timers_adapter
            };

            return TInitialState::dispatch_event(sub_dispatch_context, FsmEvent::Start);
        }

        Ok(())
    }
}

/// A transition's action that operates on both the exit and entry states.
pub trait FsmTransitionAction<F: FsmBackend, E, TStateFrom, TStateTo> {
    /// This action is executed after the first state's exit event, and just before the second event's entry action. It can mutate both states.
    fn action<'a, Q: FsmEventQueue<F>>(event: &E, context: &mut EventContext<'a, F, Q>, from: &mut TStateFrom, to: &mut TStateTo);

    fn execute_transition<'a, 'b, 'c, 'd, Q: FsmEventQueue<F>, I, T>(context: &'d mut DispatchContext<'a, 'b, 'c, F, Q, I, T>, event: &E, region: FsmRegionId, inspect_event_ctx: &mut I)
        where 
            I: Inspect,
            <F as FsmBackend>::States: FsmStateTransitionAsMut<TStateFrom, TStateTo>,
            <F as FsmBackend>::States: AsMut<TStateFrom>,
            <F as FsmBackend>::States: AsMut<TStateTo>,
            TStateFrom: FsmState<F>,
            TStateTo: FsmState<F>, Self: Sized,
            T: FsmTimers<F>
    {
        let inspect_ctx = inspect_event_ctx.for_transition::<Self>();

        <TStateFrom>::execute_on_exit(context, region);
        
        // transition action
        {
            inspect_ctx.on_action::<Self>();

            let mut event_context = EventContext {
                context: &mut context.backend.context,
                queue: context.queue,
                region
            };        
            let states: (&mut TStateFrom, &mut TStateTo) = context.backend.states.as_state_transition_mut();
            Self::action(event, &mut event_context, states.0, states.1);
        }
        

        <TStateTo>::execute_on_entry(context, region);

        let cs = context.backend.current_states.as_mut();
        cs[region] = FsmCurrentState::State(<TStateTo>::fsm_state());
    }

    /// Executed after the transition on the parent FSM (F) and triggers the first `start()` call if necessary. Subsequent
    /// dispatches are handled using the main dispatch table.
    fn execute_on_sub_entry<'a, 'b, 'c, 'd, Q, I, T>(context: &'d mut DispatchContext<'a, 'b, 'c, F, Q, I, T>, _region: FsmRegionId, inspect_event_ctx: &mut I) 
        -> FsmDispatchResult
        where
            TStateTo: FsmBackend,
            Q: FsmEventQueue<F>,
            I: Inspect,
            <F as FsmBackend>::Events: From<<TStateTo as FsmBackend>::Events>,
            <F as FsmBackend>::States: AsMut<TStateTo>,
            TStateTo: DerefMut<Target = FsmBackendImpl<TStateTo>>,
            T: FsmTimers<F>,
            <F as FsmBackend>::Timers: From<<TStateTo as FsmBackend>::Timers>
    {
        let sub_backend: &mut TStateTo = context.backend.states.as_mut();
        let states = sub_backend.get_current_states();
        if FsmCurrentState::all_stopped(states.as_ref()) {
            let mut queue_adapter = FsmEventQueueSub {
                parent: context.queue,
                _parent_fsm: PhantomData::<F>::default(),
                _sub_fsm: PhantomData::<TStateTo>::default()
            };

            let mut timers_adapter = FsmTimersSub {
                parent: context.timers,
                _parent_fsm: core::marker::PhantomData::<F>::default(),
                _sub_fsm: core::marker::PhantomData::<TStateTo>::default()
            };

            let mut inspect = inspect_event_ctx.for_sub_machine::<TStateTo>();

            let sub_dispatch_context = DispatchContext {
                backend: sub_backend,
                inspect: &mut inspect,
                queue: &mut queue_adapter,
                timers: &mut timers_adapter
            };

            return TStateTo::dispatch_event(sub_dispatch_context, FsmEvent::Start);
        }

        Ok(())
    }
}

/// An internal or self action can only mutate itself.
pub trait FsmAction<F: FsmBackend, E, State> {
    /// This action is executed as part of an internal or self transition.
    fn action<'a, Q: FsmEventQueue<F>>(event: &E, context: &mut EventContext<'a, F, Q>, state: &mut State);
    /// Is this a self transition which should trigger the state's exit and entry actions?
    fn should_trigger_state_actions() -> bool;

    fn execute_action<'a, 'b, 'c, 'd, Q: FsmEventQueue<F>, I, T>(context: &'d mut DispatchContext<'a, 'b, 'c, F, Q, I, T>, event: &E, region: FsmRegionId)
        where <F as FsmBackend>::States: AsMut<State>, I: Inspect, T: FsmTimers<F>
    {
        let mut event_context = EventContext {
            context: &mut context.backend.context,
            queue: context.queue,
            region
        };

        let state: &mut State = context.backend.states.as_mut();

        Self::action(event, &mut event_context, state);
    }

    fn execute_transition<'a, 'b, 'c, 'd, Q: FsmEventQueue<F>, I, T>(context: &'d mut DispatchContext<'a, 'b, 'c, F, Q, I, T>, event: &E, region: FsmRegionId, inspect_event_ctx: &mut I)
        where I: Inspect,
            State: FsmState<F>,
            <F as FsmBackend>::States: AsMut<State>, Self: Sized,
            T: FsmTimers<F>
    {
        let ctx = inspect_event_ctx.for_transition::<Self>();

        if Self::should_trigger_state_actions() {
            <State>::execute_on_exit(context, region);
        }

        Self::execute_action(context, event, region);

        if Self::should_trigger_state_actions() {
            <State>::execute_on_entry(context, region);
        }
    }
}