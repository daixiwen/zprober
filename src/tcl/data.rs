// TCL interpreter internal data
use std::sync::mpsc;
use tracing::{Level, event};

use crate::tcl::interpreter::MsgFromTcl;

pub struct Data {
    msg_sender : mpsc::Sender<MsgFromTcl>
}

impl Data {
    pub fn new(msg_sender : mpsc::Sender<MsgFromTcl>) -> Self {
        Data{msg_sender}
    }

    fn send_msg(&self, msg : MsgFromTcl) {
        event!(Level::DEBUG, ?msg, "sending message to GUI");
        if let Err(e) = self.msg_sender.send(msg) {
            event!(Level::ERROR, ?e, "Error sending message to GUI");
        }
    }

    pub fn gui_write(&self, msg: &str) {
        self.send_msg(MsgFromTcl::Write(msg.into()));
    }

    pub fn gui_command_ok(&self, msg: &str) {
        self.send_msg(MsgFromTcl::CommandOk(msg.into()));
    }

    pub fn gui_command_error(&self, msg: &str) {
        self.send_msg(MsgFromTcl::CommandError(msg.into()));
    }
}
