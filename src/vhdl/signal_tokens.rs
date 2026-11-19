// look for all tokens corresponding to a port of a signal in a source tile

use vhdl_lang::{Token,TokenAccess};

/// information about a signal/port token
pub struct SignalToken {
    /// token start
    pub start: vhdl_lang::Position,
    /// token end
    pub end: vhdl_lang::Position,
    /// signal/port name
    pub name: String
}

impl SignalToken {
    pub fn new(token: &Token, name: String) -> Self {
        SignalToken {
            start: token.pos.start(),
            end: token.pos.end(),
            name
        }
    }

    pub fn pos_match(&self, current_position: vhdl_lang::Position) -> bool {
        current_position == self.start
    }
}

/// extract all tokens related to ports and signals in a design file
pub fn extract_tokens(design_file: &vhdl_lang::ast::DesignFile) -> Vec<SignalToken> {
    let mut output_tokens = Vec::new();

    for (tokens,unit) in &design_file.design_units {
        match unit {
            vhdl_lang::ast::AnyDesignUnit::Primary(primary_unit) => {
                match primary_unit {
                    vhdl_lang::ast::AnyPrimaryUnit::Entity(e) => {
                        if let Some(ports) = &e.port_clause {
                            for port in &ports.items {
                                match port {
                                    vhdl_lang::ast::InterfaceDeclaration::Object(object) => {
                                        for ident in &object.idents {
                                            if let Some(token) = tokens.get_token(ident.tree.token) {
                                                output_tokens.push(SignalToken::new(
                                                    token, ident.tree.item.to_string()));
                                            }
                                        }
                                    },
                                    _ => {}
                                }
                            }
                        }
                    },
                    _ => {}

                }
            },
            vhdl_lang::ast::AnyDesignUnit::Secondary(secondary_unit) => {}
        }
    }
    output_tokens
}