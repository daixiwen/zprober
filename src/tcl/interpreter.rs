// Interface to the TCL interpreter from the GUI

use super::data::Data;
use molt::{ContextID, Interp, MoltResult, Value, check_args, molt_ok};
use std::sync::mpsc;
use tracing::{Level, event, span};

pub struct Interpreter {
    thread_handle: Option<std::thread::JoinHandle<()>>,
    msg_sender: mpsc::Sender<MsgToTcl>,
    msg_receiver: mpsc::Receiver<MsgFromTcl>,
}

impl Default for Interpreter {
    fn default() -> Self {
        Interpreter::new()
    }
}

impl Interpreter {
    pub fn new() -> Self {
        let (to_snd, to_rcv) = mpsc::channel();
        let (from_snd, from_rcv) = mpsc::channel();

        let tcl_data = crate::tcl::data::Data::new(from_snd);

        let thread_handle = std::thread::spawn(move || Self::run(tcl_data, to_rcv));

        Interpreter {
            thread_handle: Some(thread_handle),
            msg_sender: to_snd,
            msg_receiver: from_rcv,
        }
    }

    fn run(tcl_data: crate::tcl::data::Data, msg_receiver: mpsc::Receiver<MsgToTcl>) {
        let mut interpreter = Interp::new();

        let data_id = interpreter.save_context(tcl_data);

        // inject a replacement for puts to print into the GUI console
        interpreter.remove_command("puts");
        interpreter.add_context_command("puts", Interpreter::cmd_puts, data_id);
        interpreter.add_context_command("reset_layout", Interpreter::cmd_reset_layout, data_id);
        super::project::install_tcl(&mut interpreter, data_id);

        loop {
            match msg_receiver.recv() {
                Err(_) => {
                    event!(Level::ERROR, "error receiving a message from TCL thread");
                    break;
                }
                Ok(MsgToTcl::UpdateEguiContext(ctx)) => {
                    let tcl_data: &mut Data = interpreter.context(data_id);
                    tcl_data.egui_ctx = Some(ctx);
                }
                Ok(MsgToTcl::Quit) => {
                    event!(Level::DEBUG, "tcl thread quitting");
                    break;
                }
                Ok(MsgToTcl::Command(cmd)) => {
                    let span = span!(Level::INFO, "executing tcl command \"{}\"", cmd);
                    let _enter = span.enter();

                    let tcl_result = interpreter.eval(&cmd);
                    let tcl_data: &mut Data = interpreter.context(data_id);

                    match tcl_result {
                        Ok(value) => {
                            event!(Level::DEBUG, ?value, "TCL result");
                            tcl_data.gui_command_ok(&value.to_string());
                        }
                        Err(exception) => {
                            if exception.is_error() {
                                event!(Level::ERROR, cmd, ?exception, "TCL error");
                                tcl_data.gui_command_error(&exception.value().to_string());
                            } else {
                                event!(Level::DEBUG, result = ?exception.value(), "TCL result");
                                tcl_data.gui_command_ok(&exception.value().to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    fn send_msg(&self, msg: MsgToTcl) {
        event!(Level::DEBUG, ?msg, "sending message to TCL interpreter");
        if let Err(e) = self.msg_sender.send(msg) {
            event!(Level::ERROR, ?e, "Error sending message to TCL interpreter");
        }
    }

    pub fn update_egui_context(&self, ctx: egui::Context) {
        self.send_msg(MsgToTcl::UpdateEguiContext(ctx));
    }

    pub fn execute(&self, command: &str) {
        self.send_msg(MsgToTcl::Command(command.into()));
    }

    pub fn quit(&mut self) {
        self.send_msg(MsgToTcl::Quit);
        event!(Level::DEBUG, "waiting for TCL thread to quit");
        if let Some(thread_handle) = self.thread_handle.take() {
            if let Err(e) = thread_handle.join() {
                event!(Level::ERROR, ?e, "Error exiting TCL interpreter thread")
            } else {
                event!(Level::DEBUG, "thread exited");
            }
        }
    }

    pub fn receive(&self) -> Option<MsgFromTcl> {
        match self.msg_receiver.try_recv() {
            Ok(msg) => Some(msg),
            Err(mpsc::TryRecvError::Disconnected) => {
                event!(Level::ERROR, "interpreter thread disconnected");
                None
            }
            Err(mpsc::TryRecvError::Empty) => None,
        }
    }

    // generic TCL commands
    fn cmd_puts(interp: &mut Interp, context_id: ContextID, argv: &[Value]) -> MoltResult {
        check_args(1, argv, 2, 2, "string")?;

        let tcl_data: &Data = interp.context(context_id);
        tcl_data.gui_write(&argv[1].to_string());
        molt_ok!()
    }

    fn cmd_reset_layout(interp: &mut Interp, context_id: ContextID, argv: &[Value]) -> MoltResult {
        check_args(1, argv, 1, 1, "")?;

        let tcl_data: &Data = interp.context(context_id);
        tcl_data.send_reset_layout();
        molt_ok!()
    }
}

#[derive(Debug)]
pub enum MsgToTcl {
    UpdateEguiContext(egui::Context),
    Command(String),
    Quit,
}

#[derive(Debug)]
pub enum MsgFromTcl {
    Write(String),
    CommandOk(String),
    CommandError(String),
    NewProjectHierarchy(Vec<super::project::ProjectEntry>),
    ResetGuiLayout,
    SelectInstance(
        String,                              // path
        Vec<crate::vhdl::hierarchy::Signal>, // vhdl module
        Vec<crate::probes::Probe>,           // probes for the module
    ),
    UpdateProbes(
        String,                    // path
        Vec<crate::probes::Probe>, // probes for the module
    ),
}
