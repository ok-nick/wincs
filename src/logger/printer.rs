use widestring::{U16CStr, U16Str, U16String};

use crate::logger::{Logger, ProviderState, Reason};

pub struct PrintLogger<L: Logger> {
    inner: L,
}

impl<L: Logger> PrintLogger<L> {
    pub fn new(logger: L) -> Self {
        Self { inner: logger }
    }
}

// TODO: use log crate or something
impl<L: Logger> Logger for PrintLogger<L> {
    fn logs(&self) -> &[Reason] {
        self.inner.logs()
    }

    fn add_log(&mut self, reason: Reason) {
        self.inner.add_log(reason)
    }

    fn message(&self) -> &U16CStr {
        self.inner.message()
    }

    fn set_message(&mut self, message: U16String) {
        self.inner.set_message(message)
    }

    fn state(&self) -> ProviderState {
        self.inner.state()
    }

    fn set_state(&mut self, state: ProviderState) {
        self.inner.set_state(state)
    }
}
