use widestring::{U16CStr, U16Str, U16String};

use crate::logger::{Logger, ProviderState, Reason};

pub struct BasicLogger {
    logs: Vec<Reason>,
}

// TODO: this
impl Logger for BasicLogger {
    fn logs(&self) -> &[Reason] {
        &self.logs
    }

    fn add_log(&mut self, reason: Reason) {
        self.logs.push(reason);
    }

    fn message(&self) -> &U16CStr {
        todo!()
    }

    fn set_message(&mut self, message: U16String) {
        todo!()
    }

    fn state(&self) -> ProviderState {
        todo!()
    }

    fn set_state(&mut self, state: ProviderState) {
        todo!()
    }
}
