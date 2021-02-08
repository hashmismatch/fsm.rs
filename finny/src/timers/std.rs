//! Standard library timers with dynamic allocation of the timer's storage.

use std::{time::{Duration, Instant}};
use crate::{FsmBackend, FsmTimers};

pub struct TimersStd<F>
    where F: FsmBackend
{
    timers: Vec<(<F as FsmBackend>::Timers, StdTimer)>,
    pending_intervals: Option<(<F as FsmBackend>::Timers, usize)>
}

#[derive(Debug)]
enum StdTimer {
    Timeout { started_at: Instant, duration: Duration },
    Interval { started_at: Instant, interval: Duration }
}

impl<F> TimersStd<F>
    where F: FsmBackend
{
    pub fn new() -> Self {
        Self {
            timers: vec![],
            pending_intervals: None
        }
    }
}

impl<F> FsmTimers<F> for TimersStd<F>
    where F: FsmBackend
{
    fn create(&mut self, id: <F as FsmBackend>::Timers, settings: &crate::TimerSettings) -> crate::FsmResult<()> {
        // try to cancel any existing ones
        self.cancel(id.clone())?;

        if settings.renew {
            self.timers.push((id, StdTimer::Interval { started_at: Instant::now(), interval: settings.timeout }));
        } else {
            self.timers.push((id, StdTimer::Timeout { started_at: Instant::now(), duration: settings.timeout }));
        }

        Ok(())
    }

    fn cancel(&mut self, id: <F as FsmBackend>::Timers) -> crate::FsmResult<()> {
        self.timers.retain(|(timer_id, _)| *timer_id != id);
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

        let mut timed_out_idx = None;
        let now = Instant::now();
        for (idx, (timer_id, timer)) in self.timers.iter_mut().enumerate() {
            match timer {
                StdTimer::Timeout { started_at, duration } if now.duration_since(*started_at) >= *duration => {
                    timed_out_idx = Some(idx);
                    break;
                },
                StdTimer::Interval { ref mut started_at, interval } if now.duration_since(*started_at) >= *interval => {
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

        if let Some(idx) = timed_out_idx {
            let (id, _) = self.timers.remove(idx);
            return Some(id);
        }

        None
    }
}