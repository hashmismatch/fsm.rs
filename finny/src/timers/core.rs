//! An implementation of timers that relies just on the `Duration`. Has to be called with
//! a reasonable period rate to trigger the timers.

use crate::{FsmBackend, FsmTimers, TimersStorage, AllVariants};
use crate::lib::*;
use Duration;
use arraydeque::{Array, ArrayDeque};

pub struct TimersCore<F, S, Q>
    where F: FsmBackend,
          Q: Array<Item = <F as FsmBackend>::Timers>,
          S: TimersStorage<<F as FsmBackend>::Timers, CoreTimer>
{
    timers: S,
    pending_events: ArrayDeque<Q>,
    _fsm: PhantomData<F>
}

#[derive(Debug)]
pub enum CoreTimer {
    Timeout { time_remaining: Duration },
    Interval { time_remaining: Duration, interval: Duration }
}

impl<F, S, Q> TimersCore<F, S, Q>
    where F: FsmBackend,
    Q: Array<Item = <F as FsmBackend>::Timers>,
    S: TimersStorage<<F as FsmBackend>::Timers, CoreTimer>
{
    pub fn new(timers: S) -> Self {
        Self {
            timers,
            pending_events: ArrayDeque::new(),
            _fsm: PhantomData::default()
        }
    }

    pub fn tick(&mut self, elapsed_since_last_tick: Duration) {
        let iter = <F as FsmBackend>::Timers::iter();
        for id in iter {
            let mut timer = self.timers.get_timer_storage_mut(&id);

            // todo: account for the difference between time remaining and elapsed time, currently we just reset it
            match timer {
                Some(CoreTimer::Timeout { time_remaining}) => {
                    if *time_remaining <= elapsed_since_last_tick {
                        self.pending_events.push_front(id);
                        *timer = None
                    } else {
                        *time_remaining -= elapsed_since_last_tick;
                    }
                },
                Some(CoreTimer::Interval { time_remaining, interval }) => {
                    if *time_remaining <= elapsed_since_last_tick {
                        self.pending_events.push_front(id);
                        *time_remaining = *interval;
                    } else {
                        *time_remaining -= elapsed_since_last_tick;
                    }
                }
                None => {}
            }
        }
    }
}

impl<F, S, Q> FsmTimers<F> for TimersCore<F, S, Q>
    where F: FsmBackend,
    Q: Array<Item = <F as FsmBackend>::Timers>,
    S: TimersStorage<<F as FsmBackend>::Timers, CoreTimer>
{
    fn create(&mut self, id: <F as FsmBackend>::Timers, settings: &crate::TimerSettings) -> crate::FsmResult<()> {
        self.cancel(id.clone());

        if settings.enabled {
            let mut timer = self.timers.get_timer_storage_mut(&id);
            if settings.renew {
                *timer = Some(CoreTimer::Interval { interval: settings.timeout, time_remaining: settings.timeout });
            } else {
                *timer = Some(CoreTimer::Timeout { time_remaining: settings.timeout });
            }
        }
        
        Ok(())
    }

    fn cancel(&mut self, id: <F as FsmBackend>::Timers) -> crate::FsmResult<()> {
        let timer = self.timers.get_timer_storage_mut(&id);
        *timer = None;
        Ok(())
    }

    fn get_triggered_timer(&mut self) -> Option<<F as FsmBackend>::Timers> {
        self.pending_events.pop_back()
    }
}