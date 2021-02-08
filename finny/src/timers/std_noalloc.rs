//! A naive timers implementation based on standard libraries' `Instant` time provider and no runtime allocations.
//! Type system has to be setup manually.

use std::{time::{Duration, Instant}};
use crate::{FsmBackend, FsmTimers, TimersStorage, AllVariants};

pub struct TimersStdNoAlloc<F, S>
    where F: FsmBackend
{
    timers: S,
    pending_intervals: Option<(<F as FsmBackend>::Timers, usize)>
}

#[derive(Debug)]
pub enum StdTimer {
    Timeout { started_at: Instant, duration: Duration },
    Interval { started_at: Instant, interval: Duration }
}

impl<F, S> TimersStdNoAlloc<F, S>
    where F: FsmBackend,
    S: TimersStorage<<F as FsmBackend>::Timers, StdTimer>,
{
    pub fn new(timers: S) -> Self {
        Self {
            timers,
            pending_intervals: None
        }
    }
}

impl<F, S> FsmTimers<F> for TimersStdNoAlloc<F, S>
    where F: FsmBackend,
    S: TimersStorage<<F as FsmBackend>::Timers, StdTimer>
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

        for timer_id in <F as FsmBackend>::Timers::iter() {
            let timer = self.timers.get_timer_storage_mut(&timer_id);
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