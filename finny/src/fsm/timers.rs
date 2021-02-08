use crate::{AllVariants, DispatchContext, FsmError, FsmEvent, FsmEventQueue, Inspect, lib::*};
use crate::{FsmBackend, FsmResult};

/// Associate some data with a specific timer ID.
pub trait TimersStorage<FT, T> : Default
    where FT: AllVariants
{
    fn get_timer_storage_mut(&mut self, id: &FT) -> &mut Option<T>;
}


pub struct TimerInstance<F>
    where F: FsmBackend
{
    pub id: <F as FsmBackend>::Timers,
    pub settings: TimerFsmSettings
}

pub trait FsmTimer<F, S>
    where F: FsmBackend, Self: Default
{
    fn setup(ctx: &mut <F as FsmBackend>::Context, settings: &mut TimerFsmSettings);
    fn trigger(ctx: &<F as FsmBackend>::Context, state: &S) -> Option< <F as FsmBackend>::Events >;

    fn get_instance(&self) -> &Option<TimerInstance<F>>;
    fn get_instance_mut(&mut self) -> &mut Option<TimerInstance<F>>;

    fn execute_on_enter<I: Inspect, T: FsmTimers<F>>(&mut self, id: F::Timers, ctx: &mut <F as FsmBackend>::Context, inspect: &mut I, timers: &mut T) {
        let log = inspect.for_timer::<F>(id.clone());
        let mut settings = TimerFsmSettings::default();
        Self::setup(ctx, &mut settings);
        if settings.enabled {
            match timers.create(id.clone(), &settings.to_timer_settings()) {
                Ok(_) => {
                    let instance = self.get_instance_mut();
                    *instance = Some( TimerInstance { id, settings } );
                    log.info("Started the timer.");
                },
                Err(ref e) => {
                    log.on_error("Failed to create a timer", e);
                }
            }
        } else {
            log.info("The timer wasn't enabled.");
        }
    }

    fn execute_on_exit<I: Inspect, T: FsmTimers<F>>(&mut self, id: F::Timers, inspect: &mut I, timers: &mut T) {
        let log = inspect.for_timer::<F>(id.clone());
        match self.get_instance_mut() {
            Some(instance) => {
                if id == instance.id && instance.settings.cancel_on_state_exit {
                    match timers.cancel(id) {
                        Ok(_) => {
                            *self.get_instance_mut() = None;
                            log.info("Cancelled the timer.");
                        },
                        Err(ref e) => {
                            log.on_error("Failed to cancel the timer", e);
                        }
                    }
                }
            },
            _ => ()
        }
    }

    fn execute_trigger<'a, 'b, 'c, 'd, Q, I, T>(id: F::Timers, context: &'d mut DispatchContext<'a, 'b, 'c, F, Q, I, T>, inspect: &mut I)
        where 
            Q: FsmEventQueue<F>,
            I: Inspect,
            <F as FsmBackend>::States: AsRef<S>,
            <F as FsmBackend>::States: AsRef<Self>,
            T: FsmTimers<F>
    {
        let inspect = inspect.for_timer::<F>(id);
        let timer: &Self = context.backend.states.as_ref();
        match timer.get_instance() {
            Some(_) => {                
                match Self::trigger(&context.backend.context, context.backend.states.as_ref()) {
                    Some(ev) => {
                        let inspect = inspect.new_event::<F>(&FsmEvent::Event(ev.clone()), &context.backend);
                        match context.queue.enqueue(ev) {
                            Ok(_) => {
                                inspect.info("The event triggered by the timer was enqueued.");
                            },
                            Err(e) => {
                                inspect.on_error("The event triggered by the timer couldn't be enqueued.", &e);
                            }
                        }
                    },
                    _ => ()
                }

            },
            None => {
                let error = FsmError::TimerNotStarted;
                inspect.on_error("Timer hasn't been started.", &error);
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TimerFsmSettings {
    pub enabled: bool,
    pub timeout: Duration,
    pub renew: bool,
    pub cancel_on_state_exit: bool
}

impl TimerFsmSettings {
    pub fn to_timer_settings(&self) -> TimerSettings {
        TimerSettings {
            enabled: self.enabled,
            timeout: self.timeout,
            renew: self.renew
        }
    }
}

impl Default for TimerFsmSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            timeout: Duration::from_secs(1),
            renew: false,
            cancel_on_state_exit: true
        }
    }
}


#[derive(Debug, Clone)]
pub struct TimerSettings
{
    pub enabled: bool,
    pub timeout: Duration,
    pub renew: bool
}

pub trait FsmTimers<F>
    where F: FsmBackend
{
    fn create(&mut self, id: <F as FsmBackend>::Timers, settings: &TimerSettings) -> FsmResult<()>;
    fn cancel(&mut self, id: <F as FsmBackend>::Timers) -> FsmResult<()>;
    
    /// Return the timer that was triggered. Poll this until it returns None. The events
    /// should be dequeued in a FIFO manner.
    fn get_triggered_timer(&mut self) -> Option<<F as FsmBackend>::Timers>;
}


#[derive(Debug, Copy, Clone)]
pub struct FsmTimersTriggerEventsResult {
    pub triggered_events: usize
}

#[derive(Debug, Default, Copy, Clone)]
pub struct FsmTimersNull;

impl<F> FsmTimers<F> for FsmTimersNull
    where F: FsmBackend
{
    fn create(&mut self, _id: <F as FsmBackend>::Timers, _settings: &TimerSettings) -> FsmResult<()> {
        Err(FsmError::NotSupported)
    }

    fn cancel(&mut self, _id: <F as FsmBackend>::Timers) -> FsmResult<()> {
        Err(FsmError::NotSupported)
    }

    fn get_triggered_timer(&mut self) -> Option<<F as FsmBackend>::Timers> {
        None
    }
}


pub struct FsmTimersSub<'a, T, F, FSub>
    where
        F: FsmBackend,
        T: FsmTimers<F>
{
    pub parent: &'a mut T,
    pub _parent_fsm: PhantomData<F>,
    pub _sub_fsm: PhantomData<FSub>
}

impl<'a, T, F, FSub> FsmTimers<FSub> for FsmTimersSub<'a, T, F, FSub>
    where
        F: FsmBackend,
        T: FsmTimers<F>,
        FSub: FsmBackend,
        <F as FsmBackend>::Timers: From<<FSub as FsmBackend>::Timers>
{
    fn create(&mut self, id: <FSub as FsmBackend>::Timers, settings: &TimerSettings) -> FsmResult<()> {
        self.parent.create(id.into(), settings)
    }

    fn cancel(&mut self, id: <FSub as FsmBackend>::Timers) -> FsmResult<()> {
        self.parent.cancel(id.into())
    }

    fn get_triggered_timer(&mut self) -> Option<<FSub as FsmBackend>::Timers> {
        // todo: not needed, split the trait
        None
    }
}