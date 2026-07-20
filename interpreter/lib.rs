use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use object::builtins::*;
use object::environment::*;
use object::{BoundMethodObject, ClassObject, EvalError, InstanceObject, InstanceRef, Object};
use parser::ast::*;
use parser::lexer::token::{Token, TokenKind};
use parser::validation::validate_program;

mod interpreter_test;

pub fn eval(node: Node, env: &Env) -> Result<Rc<Object>, EvalError> {
    match node {
        Node::Program(p) => {
            let mut predefined_names = env.borrow().visible_names();
            predefined_names.extend(BuiltIns.iter().map(|builtin| builtin.name.to_string()));
            let predefined_names = predefined_names
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>();
            validate_program(&p, &predefined_names).map_err(|error| error.message)?;
            eval_block_statements(&p.body, env)
        }
        Node::Statement(statements) => eval_statement(&statements, env),
        Node::Expression(expression) => eval_expression(&expression, env),
    }
}

fn eval_block_statements(statements: &Vec<Statement>, env: &Env) -> Result<Rc<Object>, EvalError> {
    let mut result = Rc::new(Object::Null);
    for statement in statements {
        let val = eval_statement(statement, &Rc::clone(env))?;
        match *val {
            Object::ReturnValue(_) => return Ok(val),
            _ => {
                result = val;
            }
        }
    }

    return Ok(result);
}

fn eval_statement(statement: &Statement, env: &Env) -> Result<Rc<Object>, EvalError> {
    match statement {
        Statement::Expr(expr) => eval_expression(expr, env),
        Statement::Return(ReturnStatement {
            argument,
            ..
        }) => {
            let val = eval_expression(argument, env)?;
            return Ok(Rc::new(Object::ReturnValue(val)));
        }
        Statement::Let(Let {
            identifier: id,
            expr,
            ..
        }) => {
            let val = eval_expression(expr, &Rc::clone(env))?;
            let obj: Rc<Object> = Rc::clone(&val);
            if let TokenKind::IDENTIFIER {
                name,
            } = &id.kind
            {
                env.borrow_mut().set(name.clone(), obj);
            }
            return Ok(Rc::new(Object::Null));
        }
        Statement::Class(class) => eval_class_declaration(class, env),
        Statement::SetProperty(statement) => {
            let receiver = eval_expression(&statement.object, env)?;
            let value = eval_expression(&statement.value, env)?;
            set_property(&receiver, statement.property.name.clone(), value)?;
            Ok(Rc::new(Object::Null))
        }
    }
}

fn eval_class_declaration(
    declaration: &ClassDeclaration,
    env: &Env,
) -> Result<Rc<Object>, EvalError> {
    let mut constructor = None;
    let mut methods = HashMap::new();
    for method in &declaration.methods {
        let function =
            Rc::new(Object::Function(method.params.clone(), method.body.clone(), Rc::clone(env)));
        match method.kind {
            MethodKind::Constructor => constructor = Some(function),
            MethodKind::Method => {
                methods.insert(method.name.name.clone(), function);
            }
        }
    }

    let class = Rc::new(RefCell::new(ClassObject {
        name: declaration.name.name.clone(),
        constructor,
        methods,
    }));
    env.borrow_mut()
        .set(declaration.name.name.clone(), Rc::new(Object::Class(class)));
    Ok(Rc::new(Object::Null))
}

fn is_truthy(obj: &Object) -> bool {
    match obj {
        Object::Null => return false,
        Object::Boolean(false) => return false,
        _ => true,
    }
}

fn eval_expression(expression: &Expression, env: &Env) -> Result<Rc<Object>, EvalError> {
    match expression {
        Expression::LITERAL(literal) => eval_literal(literal, env),
        Expression::PREFIX(UnaryExpression {
            op,
            operand: expr,
            ..
        }) => {
            let right = eval_expression(expr, &Rc::clone(env))?;
            return eval_prefix(op, &right);
        }
        Expression::INFIX(BinaryExpression {
            op,
            left,
            right,
            ..
        }) => {
            let left = eval_expression(left, &Rc::clone(env))?;
            let right = eval_expression(right, &Rc::clone(env))?;
            return eval_infix(op, &left, &right);
        }
        Expression::IF(IF {
            condition,
            consequent,
            alternate,
            ..
        }) => {
            let condition = eval_expression(condition, &Rc::clone(env))?;
            if is_truthy(&condition) {
                eval_block_statements(&(consequent.body), env)
            } else {
                match alternate {
                    Some(alt) => eval_block_statements(&(alt.body), env),
                    None => Ok(Rc::new(Object::Null)),
                }
            }
        }
        Expression::IDENTIFIER(IDENTIFIER {
            name: id,
            ..
        }) => eval_identifier(id, env),
        Expression::FUNCTION(FunctionDeclaration {
            params,
            body,
            ..
        }) => {
            return Ok(Rc::new(Object::Function(params.clone(), body.clone(), Rc::clone(env))));
        }
        Expression::FunctionCall(FunctionCall {
            callee,
            arguments,
            ..
        }) => {
            let func = eval_expression(callee, &Rc::clone(env))?;
            let args = eval_expressions(arguments, env)?;
            apply_function(&func, &args)
        }
        Expression::Index(Index {
            object: left,
            index,
            ..
        }) => {
            let literal = eval_expression(left, &Rc::clone(env))?;
            let index = eval_expression(index, env)?;
            eval_index_expression(&literal, &index)
        }
        Expression::This(_) => eval_identifier("this", env),
        Expression::Property(property) => {
            let receiver = eval_expression(&property.object, env)?;
            get_property(&receiver, &property.property.name)
        }
        Expression::New(new_expression) => {
            let class = eval_identifier(&new_expression.callee.name, env)?;
            let arguments = eval_expressions(&new_expression.arguments, env)?;
            construct_instance(&class, &arguments)
        }
    }
}

fn get_property(receiver: &Rc<Object>, name: &str) -> Result<Rc<Object>, EvalError> {
    let Object::Instance(instance) = &**receiver else {
        return Err(format!("cannot read property '{}' of {}", name, receiver));
    };

    if let Some(value) = instance.borrow().fields.get(name).cloned() {
        return Ok(value);
    }

    let (class_name, method) = {
        let instance = instance.borrow();
        let class = instance.class.borrow();
        (class.name.clone(), class.methods.get(name).cloned())
    };
    if let Some(method) = method {
        return Ok(Rc::new(Object::BoundMethod(Rc::new(BoundMethodObject {
            receiver: Rc::clone(instance),
            method,
            name: name.to_string(),
        }))));
    }

    Err(format!("property '{}' does not exist on {}", name, class_name))
}

fn set_property(receiver: &Rc<Object>, name: String, value: Rc<Object>) -> Result<(), EvalError> {
    let Object::Instance(instance) = &**receiver else {
        return Err(format!("cannot set property '{}' of {}", name, receiver));
    };
    instance.borrow_mut().fields.insert(name, value);
    Ok(())
}

fn construct_instance(
    class_value: &Rc<Object>,
    args: &[Rc<Object>],
) -> Result<Rc<Object>, EvalError> {
    let Object::Class(class) = &**class_value else {
        return Err(format!("cannot construct {}", class_value));
    };
    let instance = Rc::new(RefCell::new(InstanceObject {
        class: Rc::clone(class),
        fields: HashMap::new(),
    }));
    let instance_value = Rc::new(Object::Instance(Rc::clone(&instance)));
    let constructor = class.borrow().constructor.clone();
    if let Some(constructor) = constructor {
        apply_method(
            &constructor,
            &instance,
            args,
            &format!("{}.constructor", class.borrow().name),
        )?;
    } else if !args.is_empty() {
        return Err(format!(
            "wrong number of arguments for {}.constructor: want=0, got={}",
            class.borrow().name,
            args.len()
        ));
    }
    Ok(instance_value)
}

fn eval_index_expression(left: &Rc<Object>, index: &Rc<Object>) -> Result<Rc<Object>, EvalError> {
    match (&**left, &**index) {
        (Object::Array(arr), Object::Integer(idx)) => match arr.get(*idx as usize) {
            Some(obj) => return Ok(Rc::clone(obj)),
            None => return Ok(Rc::new(Object::Null)),
        },
        (Object::Hash(map), key) => {
            if !(key.is_hashable()) {
                return Err("not a valid hash key".to_string());
            }

            match map.get(key) {
                Some(obj) => return Ok(Rc::clone(obj)),
                None => return Ok(Rc::new(Object::Null)),
            }
        }
        _ => return Err(format!("index operator not supported for {}", left)),
    }
}

fn apply_function(function: &Rc<Object>, args: &[Rc<Object>]) -> Result<Rc<Object>, EvalError> {
    match &**function {
        Object::Function(params, body, env) => {
            if params.len() != args.len() {
                return Err(format!(
                    "wrong number of arguments: want={}, got={}",
                    params.len(),
                    args.len()
                ));
            }
            let mut env = Environment::new_enclosed_environment(env);

            params.iter().enumerate().for_each(|(i, param)| {
                env.set(param.name.clone(), args[i].clone());
            });

            let evaluated = eval_block_statements(&body.body, &Rc::new(RefCell::new(env)))?;
            return unwrap_return(evaluated);
        }
        Object::Builtin(b) => Ok(b(args.to_vec())),
        Object::BoundMethod(bound) => {
            apply_method(&bound.method, &bound.receiver, args, &bound.name)
        }
        Object::Class(class) => {
            Err(format!("class {} must be constructed with new", class.borrow().name))
        }
        f => Err(format!("expected {} to be a function", f)),
    }
}

fn apply_method(
    method: &Rc<Object>,
    receiver: &InstanceRef,
    args: &[Rc<Object>],
    display_name: &str,
) -> Result<Rc<Object>, EvalError> {
    let Object::Function(params, body, declaration_env) = &**method else {
        return Err(format!("{} is not a method", display_name));
    };
    if params.len() != args.len() {
        return Err(format!(
            "wrong number of arguments for {}: want={}, got={}",
            display_name,
            params.len(),
            args.len()
        ));
    }

    let mut call_env = Environment::new_enclosed_environment(declaration_env);
    call_env.set("this".to_string(), Rc::new(Object::Instance(Rc::clone(receiver))));
    for (parameter, argument) in params.iter().zip(args) {
        call_env.set(parameter.name.clone(), Rc::clone(argument));
    }
    let evaluated = eval_block_statements(&body.body, &Rc::new(RefCell::new(call_env)))?;
    unwrap_return(evaluated)
}

fn unwrap_return(obj: Rc<Object>) -> Result<Rc<Object>, EvalError> {
    if let Object::ReturnValue(val) = &*obj {
        Ok(Rc::clone(val))
    } else {
        Ok(obj)
    }
}

fn eval_expressions(exprs: &Vec<Expression>, env: &Env) -> Result<Vec<Rc<Object>>, EvalError> {
    let mut list = Vec::new();
    for expr in exprs {
        let val = eval_expression(expr, &Rc::clone(env))?;
        list.push(val);
    }

    Ok(list)
}

fn eval_identifier(identifier: &str, env: &Env) -> Result<Rc<Object>, EvalError> {
    match env.borrow().get(identifier) {
        Some(obj) => Ok(obj.clone()),
        None => match BuiltIns.iter().find(|builtin| builtin.name == identifier) {
            Some(obj) => Ok(Rc::new(Object::Builtin(obj.function))),
            None => Err(format!("unknown identifier {}", identifier)),
        },
    }
}

fn eval_prefix(op: &Token, right: &Object) -> Result<Rc<Object>, EvalError> {
    match op.kind {
        TokenKind::BANG => eval_prefix_bang(right),
        TokenKind::MINUS => eval_prefix_minus(right),
        _ => Err(format!("unknown prefix operator: {}", op)),
    }
}

fn eval_prefix_bang(expr: &Object) -> Result<Rc<Object>, EvalError> {
    match *expr {
        Object::Null => Ok(Rc::new(Object::Boolean(true))),
        Object::Boolean(b) => Ok(Rc::new(Object::Boolean(!b))),
        _ => Ok(Rc::new(Object::Boolean(false))),
    }
}

fn eval_prefix_minus(expr: &Object) -> Result<Rc<Object>, EvalError> {
    match *expr {
        Object::Integer(i) => Ok(Rc::from(Object::Integer(-i))),
        _ => Err(format!("can't apply prefix minus operator: {}", expr)),
    }
}

fn eval_infix(op: &Token, left: &Object, right: &Object) -> Result<Rc<Object>, EvalError> {
    if op.kind == TokenKind::EQ || op.kind == TokenKind::NotEq {
        let equal = left == right;
        return Ok(Rc::new(Object::Boolean(if op.kind == TokenKind::EQ { equal } else { !equal })));
    }
    match (left, right) {
        (Object::Integer(left), Object::Integer(right)) => {
            return eval_integer_infix(op, *left, *right);
        }
        (Object::Boolean(left), Object::Boolean(right)) => {
            return eval_boolean_infix(op, *left, *right);
        }
        (Object::String(left), Object::String(right)) => {
            return eval_string_infix(op, left.to_string(), right.to_string());
        }
        _ => Err(format!("eval infix error for op: {}, left: {}, right: {}", op, left, right)),
    }
}

fn eval_integer_infix(op: &Token, left: i64, right: i64) -> Result<Rc<Object>, EvalError> {
    let result = match &op.kind {
        TokenKind::PLUS => Object::Integer(left + right),
        TokenKind::MINUS => Object::Integer(left - right),
        TokenKind::ASTERISK => Object::Integer(left * right),
        TokenKind::SLASH => Object::Integer(left / right),
        TokenKind::LT => Object::Boolean(left < right),
        TokenKind::GT => Object::Boolean(left > right),
        TokenKind::EQ => Object::Boolean(left == right),
        TokenKind::NotEq => Object::Boolean(left != right),
        op => return Err(format!("Invalid infix operator {} for int", op)),
    };

    Ok(Rc::from(result))
}

fn eval_boolean_infix(op: &Token, left: bool, right: bool) -> Result<Rc<Object>, EvalError> {
    let result = match &op.kind {
        TokenKind::EQ => Object::Boolean(left == right),
        TokenKind::NotEq => Object::Boolean(left != right),
        op => return Err(format!("Invalid infix operator for boolean: {}", op)),
    };

    Ok(Rc::from(result))
}

fn eval_string_infix(op: &Token, left: String, right: String) -> Result<Rc<Object>, EvalError> {
    let result = match &op.kind {
        TokenKind::EQ => Object::Boolean(left == right),
        TokenKind::NotEq => Object::Boolean(left != right),
        TokenKind::PLUS => Object::String(format!("{}{}", left, right)),
        op => return Err(format!("Invalid infix {} operator for string", op)),
    };

    Ok(Rc::from(result))
}

fn eval_literal(literal: &Literal, env: &Env) -> Result<Rc<Object>, EvalError> {
    match literal {
        Literal::Integer(Integer {
            raw: i,
            ..
        }) => Ok(Rc::from(Object::Integer(*i))),
        Literal::Boolean(Boolean {
            raw: b,
            ..
        }) => Ok(Rc::from(Object::Boolean(*b))),
        Literal::String(StringType {
            raw: s,
            ..
        }) => Ok(Rc::from(Object::String(s.clone()))),
        Literal::Array(Array {
            elements,
            ..
        }) => {
            let list = eval_expressions(elements, env)?;
            return Ok(Rc::from(Object::Array(list)));
        }
        Literal::Hash(Hash {
            elements: map,
            ..
        }) => {
            // Object's Hash impl only covers Integer/Boolean/String, which have no
            // interior mutability; keys are checked with is_hashable() before insert.
            #[allow(clippy::mutable_key_type)]
            let mut hash_map = HashMap::new();

            for (k, v) in map {
                let key = eval_expression(k, env)?;
                if !key.is_hashable() {
                    return Err(format!("key {} is not hashable", key));
                }
                let value = eval_expression(v, env)?;
                hash_map.insert(key, value);
            }

            return Ok(Rc::new(Object::Hash(hash_map)));
        } // l => return Err(format!("unknown literal: {}", *l))
    }
}
