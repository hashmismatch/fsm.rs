use crate::{FsmBackend, FsmEventQueue, FsmEventQueueSender, lib::*};

/// The internal event type that also allows stopping or starting the machine.
#[derive(Clone)]
pub enum FsmEvent<E, T> {
    Start,
    Stop,
    Timer(T),
    Event(E)
}

impl<E, T> From<E> for FsmEvent<E, T> {
    fn from(event: E) -> Self {
        FsmEvent::Event(event)
    }
}

impl<E, T> Debug for FsmEvent<E, T> where E: Debug, T: Debug {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FsmEvent::Start => f.write_str("Fsm::Start"),
            FsmEvent::Stop => f.write_str("Fsm::Stop"),
            FsmEvent::Timer(t) => f.write_fmt(format_args!("Fsm::Timer({:?})", t)),
            FsmEvent::Event(ev) => ev.fmt(f)
        }
    }
}

impl<E, T> AsRef<str> for FsmEvent<E, T> where E: AsRef<str> {
    fn as_ref(&self) -> &str {
        match self {
            FsmEvent::Start => "Fsm::Start",
            FsmEvent::Stop => "Fsm::Stop",
            FsmEvent::Timer(_) => "Fsm::Timer",
            FsmEvent::Event(e) => e.as_ref()
        }
    }
}

impl<E, T> FsmEvent<E, T> {
    pub fn to_sub_fsm<FSub>(self) -> FsmEvent<<FSub as FsmBackend>::Events, <FSub as FsmBackend>::Timers>
        where FSub: FsmBackend,
        <FSub as FsmBackend>::Timers: From<T>,
        <FSub as FsmBackend>::Timers: From<E>
    {
        match self {
            FsmEvent::Start => FsmEvent::Start,
            FsmEvent::Stop => FsmEvent::Stop,
            FsmEvent::Timer(t) => {
                FsmEvent::Timer(t.into())
            }
            FsmEvent::Event(ev) => {
                FsmEvent::Timer(ev.into())
            }
        }
    }
}


pub type FsmRegionId = usize;

/// The context that is given to all of the guards and actions.
pub struct EventContext<'a, TFsm, Q> where TFsm: FsmBackend, Q: FsmEventQueueSender<TFsm> {
    pub context: &'a mut TFsm::Context,
    pub queue: &'a mut Q,
    pub region: FsmRegionId
}

impl<'a, TFsm, Q> Deref for EventContext<'a, TFsm, Q> where TFsm: FsmBackend, Q: FsmEventQueueSender<TFsm>
{
    type Target = <TFsm as FsmBackend>::Context;

    fn deref(&self) -> &Self::Target {
        self.context
    }
}

impl<'a, TFsm: FsmBackend, Q> DerefMut for EventContext<'a, TFsm, Q> where TFsm: FsmBackend, Q: FsmEventQueueSender<TFsm> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.context
    }
}
