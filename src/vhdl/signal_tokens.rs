// look for all tokens corresponding to a port of a signal in a source tile

use vhdl_lang::{HasTokenSpan, Token, TokenAccess};

/// information about a signal/port token
pub struct SignalToken {
    /// token start
    pub start: vhdl_lang::Position,
    /// token end
    pub end: vhdl_lang::Position,
    /// signal/port name
    pub name: String,
}

impl SignalToken {
    pub fn new(token: &Token, name: String) -> Self {
        SignalToken {
            start: token.pos.start(),
            end: token.pos.end(),
            name,
        }
    }

    pub fn pos_match(&self, current_position: vhdl_lang::Position) -> bool {
        current_position == self.start
    }
}

fn add_signal_token(
    output_tokens: &mut Vec<SignalToken>,
    tokens_list: &Vec<Token>,
    token_id: vhdl_lang::TokenId,
    name: String,
) {
    if let Some(token) = tokens_list.get_token(token_id) {
        output_tokens.push(SignalToken::new(token, name));
    }
}

/// extract all tokens related to ports and signals in a design file
pub fn extract_tokens(
    entity_name: &str,
    design_file: &vhdl_lang::ast::DesignFile,
) -> Vec<SignalToken> {
    let mut output_tokens = Vec::new();

    for (tokens_list, unit) in &design_file.design_units {
        match unit {
            vhdl_lang::ast::AnyDesignUnit::Primary(primary_unit) => {
                if let vhdl_lang::ast::AnyPrimaryUnit::Entity(e) = primary_unit
                    && e.ident.tree.to_string() == entity_name
                    && let Some(ports) = &e.port_clause
                {
                    for port in &ports.items {
                        if let vhdl_lang::ast::InterfaceDeclaration::Object(object) = port {
                            for ident in &object.idents {
                                add_signal_token(
                                    &mut output_tokens,
                                    tokens_list,
                                    ident.tree.token,
                                    ident.tree.to_string(),
                                );
                            }
                        }
                    }
                }
            }
            vhdl_lang::ast::AnyDesignUnit::Secondary(secondary_unit) => match secondary_unit {
                vhdl_lang::ast::AnySecondaryUnit::Architecture(a)
                    if a.entity_name.to_string() == entity_name =>
                {
                    for decl in &a.decl {
                        if let vhdl_lang::ast::Declaration::Object(obj) = &decl.item
                            && obj.class == vhdl_lang::ast::ObjectClass::Signal
                        {
                            for ident in &obj.idents {
                                add_signal_token(
                                    &mut output_tokens,
                                    tokens_list,
                                    ident.tree.token,
                                    ident.tree.to_string(),
                                );
                            }
                        }
                    }
                    for statement in &a.statements {
                        process_statement(&mut output_tokens, tokens_list, statement);
                    }
                }
                _ => {}
            },
        }
    }
    output_tokens
}

/// find expressions which may use a signal in a statement
fn process_statement(
    output_tokens: &mut Vec<SignalToken>,
    tokens_list: &Vec<Token>,
    statement: &vhdl_lang::ast::LabeledConcurrentStatement,
) {
    match &statement.statement.item {
        vhdl_lang::ast::ConcurrentStatement::Assert(assertion) => {
            process_expression(
                output_tokens,
                tokens_list,
                &assertion.statement.condition.item,
                assertion.statement.condition.get_start_token(),
            );
        }
        vhdl_lang::ast::ConcurrentStatement::Assignment(assignment) => {
            match &assignment.assignment.target.item {
                vhdl_lang::ast::Target::Name(name) => {
                    process_name(
                        output_tokens,
                        tokens_list,
                        name,
                        assignment.assignment.target.get_start_token(),
                    );
                }
                vhdl_lang::ast::Target::Aggregate(aggregate) => {
                    for element_association in aggregate {
                        match &element_association.item {
                            vhdl_lang::ast::ElementAssociation::Positional(expr) => {
                                process_expression(
                                    output_tokens,
                                    tokens_list,
                                    &expr.item,
                                    expr.get_start_token(),
                                );
                            }
                            vhdl_lang::ast::ElementAssociation::Named(_, expr) => {
                                process_expression(
                                    output_tokens,
                                    tokens_list,
                                    &expr.item,
                                    expr.get_start_token(),
                                );
                            }
                        }
                    }
                }
            }
            match &assignment.assignment.rhs {
                vhdl_lang::ast::AssignmentRightHand::Simple(waveform) => {
                    process_waveform(output_tokens, tokens_list, waveform);
                }
                vhdl_lang::ast::AssignmentRightHand::Conditional(conditional) => {
                    for condition in &conditional.conditionals {
                        process_waveform(output_tokens, tokens_list, &condition.item);
                    }
                    if let Some((else_item, _)) = &conditional.else_item {
                        process_waveform(output_tokens, tokens_list, else_item);
                    }
                }
                vhdl_lang::ast::AssignmentRightHand::Selected(selected) => {
                    process_expression(
                        output_tokens,
                        tokens_list,
                        &selected.expression.item,
                        selected.expression.get_start_token(),
                    );
                    /*                    for alternative in &selected.alternatives {
                                            process_waveform(output_tokens, tokens_list, &alternative.item);

                                        }
                    */
                }
            }
        }
        vhdl_lang::ast::ConcurrentStatement::Block(block) => {
            for statement in &block.statements {
                process_statement(output_tokens, tokens_list, statement);
            }
        }
        vhdl_lang::ast::ConcurrentStatement::CaseGenerate(cases) => {
            for case in &cases.sels.alternatives {
                for statement in &case.item.statements {
                    process_statement(output_tokens, tokens_list, statement);
                }
            }
        }
        vhdl_lang::ast::ConcurrentStatement::ForGenerate(forgen) => {
            for statement in &forgen.body.statements {
                process_statement(output_tokens, tokens_list, statement);
            }
        }
        vhdl_lang::ast::ConcurrentStatement::IfGenerate(ifgen) => {
            for conditional in &ifgen.conds.conditionals {
                for statement in &conditional.item.statements {
                    process_statement(output_tokens, tokens_list, statement);
                }
            }
            if let Some((else_bod, _)) = &ifgen.conds.else_item {
                for statement in &else_bod.statements {
                    process_statement(output_tokens, tokens_list, statement);
                }
            }
        }
        vhdl_lang::ast::ConcurrentStatement::Instance(instance) => {
            if let Some(portmap) = &instance.port_map {
                for port in &portmap.list.items {
                    if let vhdl_lang::ast::ActualPart::Expression(conditional) = &port.actual.item {
                        process_cond_expression(
                            output_tokens,
                            tokens_list,
                            conditional,
                            port.actual.get_start_token(),
                        );
                    }
                }
            }
        }
        vhdl_lang::ast::ConcurrentStatement::ProcedureCall(call) => {
            for parameter in &call.call.item.parameters.items {
                if let vhdl_lang::ast::ActualPart::Expression(e) = &parameter.actual.item {
                    process_cond_expression(
                        output_tokens,
                        tokens_list,
                        e,
                        statement.span().start_token,
                    );
                }
            }
        }
        vhdl_lang::ast::ConcurrentStatement::Process(process) => {
            if let Some(sensitivity_list) = &process.sensitivity_list
                && let vhdl_lang::ast::SensitivityList::Names(name_list) = &sensitivity_list.item
            {
                for name in name_list {
                    process_name(
                        output_tokens,
                        tokens_list,
                        &name.item,
                        name.get_start_token(),
                    );
                }
            }
            for statement in &process.statements {
                process_sequential_statement(output_tokens, tokens_list, statement);
            }
        }
    }
}

/// find expressions which may use a signal in a conditional expression
fn process_cond_expression(
    output_tokens: &mut Vec<SignalToken>,
    tokens_list: &Vec<Token>,
    cond_expression: &vhdl_lang::ast::ConditionalExpression,
    token_id: vhdl_lang::TokenId,
) {
    match cond_expression {
        vhdl_lang::ast::ConditionalExpression::Conditional(c) => {
            for expr in &c.conditionals {
                process_expression(
                    output_tokens,
                    tokens_list,
                    &expr.item.item,
                    expr.item.get_start_token(),
                );
            }
            if let Some(else_expr) = &c.else_item {
                process_expression(
                    output_tokens,
                    tokens_list,
                    &else_expr.0.item,
                    else_expr.0.get_start_token(),
                );
            }
        }
        vhdl_lang::ast::ConditionalExpression::Simple(e) => {
            process_expression(output_tokens, tokens_list, e, token_id);
        }
    }
}

/// find expressions which may use a signal in an expression
fn process_expression(
    output_tokens: &mut Vec<SignalToken>,
    tokens_list: &Vec<Token>,
    expression: &vhdl_lang::ast::Expression,
    token_id: vhdl_lang::TokenId,
) {
    match expression {
        vhdl_lang::ast::Expression::Binary(_, expr1, expr2) => {
            process_expression(
                output_tokens,
                tokens_list,
                &expr1.item,
                expr1.get_start_token(),
            );
            process_expression(
                output_tokens,
                tokens_list,
                &expr2.item,
                expr2.get_start_token(),
            );
        }
        vhdl_lang::ast::Expression::Unary(_, expr) => {
            process_expression(
                output_tokens,
                tokens_list,
                &expr.item,
                expr.get_start_token(),
            );
        }
        vhdl_lang::ast::Expression::Aggregate(associations) => {
            for association in associations {
                process_element_association(output_tokens, tokens_list, &association.item);
            }
        }
        vhdl_lang::ast::Expression::Qualified(qualified_expression) => {
            process_name(
                output_tokens,
                tokens_list,
                &qualified_expression.type_mark.item,
                qualified_expression.type_mark.get_start_token(),
            );
            process_expression(
                output_tokens,
                tokens_list,
                &qualified_expression.expr.item,
                qualified_expression.expr.get_start_token(),
            );
        }
        vhdl_lang::ast::Expression::Name(name) => {
            process_name(output_tokens, tokens_list, name, token_id);
        }
        vhdl_lang::ast::Expression::Literal(_) => {}
        vhdl_lang::ast::Expression::New(_) => {}
        vhdl_lang::ast::Expression::Parenthesized(cond_expression) => {
            process_cond_expression(
                output_tokens,
                tokens_list,
                &cond_expression.item,
                cond_expression.get_start_token(),
            );
        }
    }
}

/// find expressions which may use a signal in a name object
fn process_name(
    output_tokens: &mut Vec<SignalToken>,
    tokens_list: &Vec<Token>,
    name: &vhdl_lang::ast::Name,
    token_id: vhdl_lang::TokenId,
) {
    match name {
        vhdl_lang::ast::Name::Designator(designator) => {
            process_designator(output_tokens, tokens_list, &designator.item, token_id);
        }
        vhdl_lang::ast::Name::Selected(name, _) => {
            process_name(
                output_tokens,
                tokens_list,
                &name.item,
                name.get_start_token(),
            );
        }
        vhdl_lang::ast::Name::SelectedAll(name) => {
            process_name(
                output_tokens,
                tokens_list,
                &name.item,
                name.get_start_token(),
            );
        }
        vhdl_lang::ast::Name::Slice(name, discrete_range) => {
            process_name(
                output_tokens,
                tokens_list,
                &name.item,
                name.get_start_token(),
            );
            process_discrete_range(output_tokens, tokens_list, discrete_range);
        }
        vhdl_lang::ast::Name::CallOrIndexed(call) => {
            process_name(
                output_tokens,
                tokens_list,
                &call.name.item,
                call.name.get_start_token(),
            );
            for element in &call.parameters.items {
                process_association_element(output_tokens, tokens_list, element);
            }
        }
        vhdl_lang::ast::Name::Attribute(_) => {}
        vhdl_lang::ast::Name::External(_) => {}
    }
}

/// find expressions which may use a signal in a designatorn
fn process_designator(
    output_tokens: &mut Vec<SignalToken>,
    tokens_list: &Vec<Token>,
    designator: &vhdl_lang::ast::Designator,
    token_id: vhdl_lang::TokenId,
) {
    if let vhdl_lang::ast::Designator::Identifier(ident) = designator {
        add_signal_token(output_tokens, tokens_list, token_id, ident.name_utf8());
    }
}

/// find expressions which may use a signal in a range object
fn process_range(
    output_tokens: &mut Vec<SignalToken>,
    tokens_list: &Vec<Token>,
    range: &vhdl_lang::ast::Range,
) {
    if let vhdl_lang::ast::Range::Range(internal_range) = range {
        process_expression(
            output_tokens,
            tokens_list,
            &internal_range.left_expr.item,
            internal_range.left_expr.get_start_token(),
        );
        process_expression(
            output_tokens,
            tokens_list,
            &internal_range.right_expr.item,
            internal_range.right_expr.get_start_token(),
        );
    }
}

/// find expressions which may use a signal in an association element
fn process_association_element(
    output_tokens: &mut Vec<SignalToken>,
    tokens_list: &Vec<Token>,
    association_element: &vhdl_lang::ast::AssociationElement,
) {
    if let vhdl_lang::ast::ActualPart::Expression(expr) = &association_element.actual.item {
        process_cond_expression(
            output_tokens,
            tokens_list,
            expr,
            association_element.actual.get_start_token(),
        );
    }
}

/// find expressions which may use a signal in an element association
fn process_element_association(
    output_tokens: &mut Vec<SignalToken>,
    tokens_list: &Vec<Token>,
    element_association: &vhdl_lang::ast::ElementAssociation,
) {
    match &element_association {
        vhdl_lang::ast::ElementAssociation::Named(choices, expr) => {
            for choice in choices {
                match &choice.item {
                    vhdl_lang::ast::Choice::DiscreteRange(range) => {
                        process_discrete_range(output_tokens, tokens_list, range);
                    }
                    vhdl_lang::ast::Choice::Expression(expression) => {
                        process_expression(
                            output_tokens,
                            tokens_list,
                            expression,
                            choice.get_start_token(),
                        );
                    }
                    vhdl_lang::ast::Choice::Others => {}
                }
            }
            process_expression(
                output_tokens,
                tokens_list,
                &expr.item,
                expr.get_start_token(),
            );
        }
        vhdl_lang::ast::ElementAssociation::Positional(expr) => {
            process_expression(
                output_tokens,
                tokens_list,
                &expr.item,
                expr.get_start_token(),
            );
        }
    }
}

/// find expressions which may use a signal in a waveform
fn process_waveform(
    output_tokens: &mut Vec<SignalToken>,
    tokens_list: &Vec<Token>,
    waveform: &vhdl_lang::ast::Waveform,
) {
    match waveform {
        vhdl_lang::ast::Waveform::Elements(elements) => {
            for waveform_element in elements {
                process_expression(
                    output_tokens,
                    tokens_list,
                    &waveform_element.value.item,
                    waveform_element.value.get_start_token(),
                );
            }
        }
        vhdl_lang::ast::Waveform::Unaffected(_) => {}
    }
}

/// find expressions which may use a signal in a discrete range
fn process_discrete_range(
    output_tokens: &mut Vec<SignalToken>,
    tokens_list: &Vec<Token>,
    discrete_range: &vhdl_lang::ast::DiscreteRange,
) {
    match discrete_range {
        vhdl_lang::ast::DiscreteRange::Discrete(name, range_maybe) => {
            process_name(
                output_tokens,
                tokens_list,
                &name.item,
                name.get_start_token(),
            );
            if let Some(range) = range_maybe {
                process_range(output_tokens, tokens_list, range);
            }
        }
        vhdl_lang::ast::DiscreteRange::Range(range) => {
            process_range(output_tokens, tokens_list, range);
        }
    }
}

/// find expressions which may use a signal in a sequential statement
fn process_sequential_statement(
    output_tokens: &mut Vec<SignalToken>,
    tokens_list: &Vec<Token>,
    statement: &vhdl_lang::ast::LabeledSequentialStatement,
) {
    match &statement.statement.item {
        vhdl_lang::ast::SequentialStatement::Assert(assert) => {
            process_expression(
                output_tokens,
                tokens_list,
                &assert.condition.item,
                assert.condition.get_start_token(),
            );
        }
        vhdl_lang::ast::SequentialStatement::Case(case) => {
            process_expression(
                output_tokens,
                tokens_list,
                &case.expression.item,
                case.expression.get_start_token(),
            );
            for alternative in &case.alternatives {
                for statement in &alternative.item {
                    process_sequential_statement(output_tokens, tokens_list, statement);
                }
            }
        }
        vhdl_lang::ast::SequentialStatement::If(if_statement) => {
            for conditional in &if_statement.conds.conditionals {
                process_expression(
                    output_tokens,
                    tokens_list,
                    &conditional.condition.item,
                    conditional.condition.get_start_token(),
                );
                for statement in &conditional.item {
                    process_sequential_statement(output_tokens, tokens_list, statement);
                }
            }
            if let Some((else_statement, _)) = &if_statement.conds.else_item {
                for statement in else_statement {
                    process_sequential_statement(output_tokens, tokens_list, statement);
                }
            }
        }
        vhdl_lang::ast::SequentialStatement::Loop(loop_statement) => {
            if let Some(scheme) = &loop_statement.iteration_scheme {
                match &scheme {
                    vhdl_lang::ast::IterationScheme::For(_, discrete_range) => {
                        process_discrete_range(output_tokens, tokens_list, discrete_range);
                    }
                    vhdl_lang::ast::IterationScheme::While(expression) => {
                        process_expression(
                            output_tokens,
                            tokens_list,
                            &expression.item,
                            expression.get_start_token(),
                        );
                    }
                }
            }
            for statement in &loop_statement.statements {
                process_sequential_statement(output_tokens, tokens_list, statement);
            }
        }
        vhdl_lang::ast::SequentialStatement::ProcedureCall(proc_call) => {
            for parameter in &proc_call.item.parameters.items {
                process_association_element(output_tokens, tokens_list, parameter);
            }
        }
        vhdl_lang::ast::SequentialStatement::Return(return_statement) => {
            if let Some(expression) = &return_statement.expression {
                process_expression(
                    output_tokens,
                    tokens_list,
                    &expression.item,
                    expression.get_start_token(),
                );
            }
        }
        vhdl_lang::ast::SequentialStatement::SignalAssignment(signal_assignment) => {
            match &signal_assignment.target.item {
                vhdl_lang::ast::Target::Aggregate(aggregate) => {
                    for association in aggregate {
                        process_element_association(output_tokens, tokens_list, &association.item);
                    }
                }
                vhdl_lang::ast::Target::Name(name) => {
                    process_name(
                        output_tokens,
                        tokens_list,
                        name,
                        signal_assignment.target.get_start_token(),
                    );
                }
            }
            match &signal_assignment.rhs {
                vhdl_lang::ast::AssignmentRightHand::Conditional(conditionals) => {
                    for statement in &conditionals.conditionals {
                        process_waveform(output_tokens, tokens_list, &statement.item);
                    }
                    if let Some((else_item, _)) = &conditionals.else_item {
                        process_waveform(output_tokens, tokens_list, else_item);
                    }
                }
                vhdl_lang::ast::AssignmentRightHand::Selected(selected) => {
                    process_expression(
                        output_tokens,
                        tokens_list,
                        &selected.expression.item,
                        selected.expression.get_start_token(),
                    );
                    for alternative in &selected.alternatives {
                        process_waveform(output_tokens, tokens_list, &alternative.item);
                    }
                }
                vhdl_lang::ast::AssignmentRightHand::Simple(simple) => {
                    process_waveform(output_tokens, tokens_list, simple);
                }
            }
        }
        vhdl_lang::ast::SequentialStatement::VariableAssignment(variable_assignment) => {
            match &variable_assignment.rhs {
                vhdl_lang::ast::AssignmentRightHand::Conditional(conditionals) => {
                    for statement in &conditionals.conditionals {
                        process_expression(
                            output_tokens,
                            tokens_list,
                            &statement.item.item,
                            statement.item.get_start_token(),
                        );
                    }
                    if let Some((else_item, _)) = &conditionals.else_item {
                        process_expression(
                            output_tokens,
                            tokens_list,
                            &else_item.item,
                            else_item.get_start_token(),
                        );
                    }
                }
                vhdl_lang::ast::AssignmentRightHand::Selected(selected) => {
                    process_expression(
                        output_tokens,
                        tokens_list,
                        &selected.expression.item,
                        selected.expression.get_start_token(),
                    );
                    for alternative in &selected.alternatives {
                        process_expression(
                            output_tokens,
                            tokens_list,
                            &alternative.item.item,
                            alternative.item.get_start_token(),
                        );
                    }
                }
                vhdl_lang::ast::AssignmentRightHand::Simple(simple) => {
                    process_expression(
                        output_tokens,
                        tokens_list,
                        &simple.item,
                        simple.get_start_token(),
                    );
                }
            }
        }
        _ => {}
    }
}
