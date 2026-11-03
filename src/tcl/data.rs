// TCL interpreter internal data
use std::sync::mpsc;
use tracing::{Level, event};

use crate::tcl::interpreter::MsgFromTcl;

pub struct Data {
    msg_sender: mpsc::Sender<MsgFromTcl>,
    pub egui_ctx: Option<egui::Context>,

    pub vhdl_project: Option<crate::vhdl::Project>,
    pub vhdl_hierarchy: Option<crate::vhdl::Hierarchy>
}

impl Data {
    pub fn new(msg_sender: mpsc::Sender<MsgFromTcl>) -> Self {
        Data {
            msg_sender,
            egui_ctx: None,
            vhdl_project: None,
            vhdl_hierarchy: None
        }
    }

    fn send_msg(&self, msg: MsgFromTcl) {
        event!(Level::DEBUG, ?msg, "sending message to GUI");
        if let Err(e) = self.msg_sender.send(msg) {
            event!(Level::ERROR, ?e, "Error sending message to GUI");
        }
        if let Some(ctx) = &self.egui_ctx {
            event!(Level::DEBUG, "requesting repaint");
            ctx.request_repaint();
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

    pub fn send_new_hierarchy(&self, hierarchy: Vec<super::project::ProjectEntry>) {
        self.send_msg(MsgFromTcl::NewProjectHierarchy(hierarchy));
    }

    pub fn send_reset_layout(&self) {
        self.send_msg(MsgFromTcl::ResetGuiLayout);
    }
}
