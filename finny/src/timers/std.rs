//! A naive timers implementation based on standard libraries' `Instant` time provider.

use std::{marker::PhantomData, time::{Duration, Instant}};
use crate::{FsmBackend, FsmTimers, TimersStorage};

pub struct TimersStd<F, S>
    where F: FsmBackend
{
    //timers: Vec<(<F as FsmBackend>::Timers, StdTimer)>,
    timers: S,
    pending_intervals: Option<(<F as FsmBackend>::Timers, usize)>
}

#[derive(Debug)]
pub enum StdTimer {
    Timeout { started_at: Instant, duration: Duration },
    Interval { started_at: Instant, interval: Duration }
}

impl<'a, F, S> TimersStd<F, S>
    where F: FsmBackend,
    S: TimersStorage<'a, <F as FsmBackend>::Timers, StdTimer>
{
    pub fn new() -> Self {
        Self {
            timers: S::default(),
            pending_intervals: None
        }
    }
}

impl<'a, F, S> FsmTimers<F> for TimersStd<F, S>
    where F: FsmBackend,
    S: TimersStorage<'a, <F as FsmBackend>::Timers, StdTimer>
{
    fn create(&mut self, id: <F as FsmBackend>::Timers, settings: &crate::TimerSettings) -> crate::FsmResult<()> {
        // try to cancel any existing ones
        self.cancel(id.clone())?;

        let t = self.timers.get_timer_storage_mut(&id);

        if settings.renew {
            *t = Some(StdTimer::Interval { started_at: Instant::now(), interval: settings.timeout });
        } else {
            *t = Some(StdTimer::Timeout { started_at: Instant::now(), duration: settings.timeout });
        }

        Ok(())
    }

    fn cancel(&mut self, id: <F as FsmBackend>::Timers) -> crate::FsmResult<()> {
        let t = self.timers.get_timer_storage_mut(&id);
        *t = None;
        Ok(())
    }

    fn get_triggered_timer(&mut self) -> Option<<F as FsmBackend>::Timers> {
        if let Some((id, mut times)) = self.pending_intervals.take() {
            times -= 1;
            if times > 0 {
                self.pending_intervals = Some((id.clone(), times));
            }

            return Some(id);
        }

        
        let mut timed_out_id = None;
        let now = Instant::now();

        //let iter = self.timers.iter();

        
        for (timer_id, timer) in self.timers.iter_mut() {
            match timer {
                Some(StdTimer::Timeout { started_at, duration }) if now.duration_since(*started_at) >= *duration => {
                    timed_out_id = Some(timer_id);
                    break;
                },
                Some(StdTimer::Interval { ref mut started_at, interval }) if now.duration_since(*started_at) >= *interval => {
                    let t = now.duration_since(*started_at);
                    let times = ((t.as_secs_f32() / interval.as_secs_f32()).floor() as usize) - 1;
                    if times > 0 {
                        self.pending_intervals = Some((timer_id.clone(), times));
                    }
                    *started_at = now;
                    return Some(timer_id.clone());
                },
                _ => ()
            }
        }

        if let Some(id) = timed_out_id {
            let timer = self.timers.get_timer_storage_mut(&id);
            *timer = None;
            return Some(id);
        }
        

        None
    }
}