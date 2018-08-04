use super::*;

// test expr_literals
// fn foo() {
//     let _ = true;
//     let _ = false;
//     let _ = 1;
//     let _ = 2.0;
//     let _ = b'a';
//     let _ = 'b';
//     let _ = "c";
//     let _ = r"d";
//     let _ = b"e";
//     let _ = br"f";
// }
const LITERAL_FIRST: TokenSet =
    token_set![TRUE_KW, FALSE_KW, INT_NUMBER, FLOAT_NUMBER, BYTE, CHAR,
               STRING, RAW_STRING, BYTE_STRING, RAW_BYTE_STRING];

pub(crate) fn literal(p: &mut Parser) -> Option<CompletedMarker> {
    if !LITERAL_FIRST.contains(p.current()) {
        return None;
    }
    let m = p.start();
    p.bump();
    Some(m.complete(p, LITERAL))
}

pub(super) const ATOM_EXPR_FIRST: TokenSet =
    token_set_union![
        LITERAL_FIRST,
        token_set![L_PAREN, PIPE, MOVE_KW, IF_KW, MATCH_KW, UNSAFE_KW, L_CURLY, RETURN_KW,
                   IDENT, SELF_KW, SUPER_KW, COLONCOLON ],
    ];

pub(super) fn atom_expr(p: &mut Parser, r: Restrictions) -> Option<CompletedMarker> {
    match literal(p) {
        Some(m) => return Some(m),
        None => (),
    }
    if paths::is_path_start(p) {
        return Some(path_expr(p, r));
    }
    let la = p.nth(1);
    let done = match p.current() {
        L_PAREN => tuple_expr(p),
        PIPE => lambda_expr(p),
        MOVE_KW if la == PIPE => lambda_expr(p),
        IF_KW => if_expr(p),
        MATCH_KW => match_expr(p),
        UNSAFE_KW if la == L_CURLY => block_expr(p),
        L_CURLY => block_expr(p),
        RETURN_KW => return_expr(p),
        _ => {
            p.err_and_bump("expected expression");
            return None;
        }
    };
    Some(done)
}

fn tuple_expr(p: &mut Parser) -> CompletedMarker {
    assert!(p.at(L_PAREN));
    let m = p.start();
    p.expect(L_PAREN);
    p.expect(R_PAREN);
    m.complete(p, TUPLE_EXPR)
}

// test lambda_expr
// fn foo() {
//     || ();
//     || -> i32 { 92 };
//     |x| x;
//     move |x: i32,| x;
// }
fn lambda_expr(p: &mut Parser) -> CompletedMarker {
    assert!(p.at(PIPE) || (p.at(MOVE_KW) && p.nth(1) == PIPE));
    let m = p.start();
    p.eat(MOVE_KW);
    params::param_list_opt_types(p);
    if fn_ret_type(p) {
        block(p);
    } else {
        expr(p)
    }
    m.complete(p, LAMBDA_EXPR)
}

// test if_expr
// fn foo() {
//     if true {};
//     if true {} else {};
//     if true {} else if false {} else {};
//     if S {};
// }
fn if_expr(p: &mut Parser) -> CompletedMarker {
    assert!(p.at(IF_KW));
    let m = p.start();
    if_head(p);
    block(p);
    if p.at(ELSE_KW) {
        p.bump();
        if p.at(IF_KW) {
            if_expr(p);
        } else {
            block(p);
        }
    }
    m.complete(p, IF_EXPR)
}

fn if_head(p: &mut Parser) {
    assert!(p.at(IF_KW));
    p.bump();
    expr_no_struct(p);
}

// test match_expr
// fn foo() {
//     match () { };
//     match S {};
// }
fn match_expr(p: &mut Parser) -> CompletedMarker {
    assert!(p.at(MATCH_KW));
    let m = p.start();
    p.bump();
    expr_no_struct(p);
    p.eat(L_CURLY);
    while !p.at(EOF) && !p.at(R_CURLY) {
        match_arm(p);
        if !p.at(R_CURLY) {
            p.expect(COMMA);
        }
    }
    p.expect(R_CURLY);
    m.complete(p, MATCH_EXPR)
}

// test match_arm
// fn foo() {
//     match () {
//         _ => (),
//         X | Y if Z => (),
//     };
// }
fn match_arm(p: &mut Parser) {
    let m = p.start();
    loop {
        patterns::pattern(p);
        if !p.eat(PIPE) {
            break;
        }
    }
    if p.at(IF_KW) {
        if_head(p)
    }
    p.expect(FAT_ARROW);
    expr(p);
    m.complete(p, MATCH_ARM);
}

// test block_expr
// fn foo() {
//     {};
//     unsafe {};
// }
pub(super) fn block_expr(p: &mut Parser) -> CompletedMarker {
    assert!(p.at(L_CURLY) || p.at(UNSAFE_KW) && p.nth(1) == L_CURLY);
    let m = p.start();
    p.eat(UNSAFE_KW);
    p.bump();
    while !p.at(EOF) && !p.at(R_CURLY) {
        match p.current() {
            LET_KW => let_stmt(p),
            _ => {
                // test block_items
                // fn a() { fn b() {} }
                let m = p.start();
                match items::maybe_item(p) {
                    items::MaybeItem::Item(kind) => {
                        m.complete(p, kind);
                    }
                    items::MaybeItem::Modifiers => {
                        m.abandon(p);
                        p.error("expected an item");
                    }
                    // test pub_expr
                    // fn foo() { pub 92; } //FIXME
                    items::MaybeItem::None => {
                        expressions::expr(p);
                        if p.eat(SEMI) {
                            m.complete(p, EXPR_STMT);
                        } else {
                            m.abandon(p);
                        }
                    }
                }
            }
        }
    }
    p.expect(R_CURLY);
    m.complete(p, BLOCK_EXPR)
}

// test let_stmt;
// fn foo() {
//     let a;
//     let b: i32;
//     let c = 92;
//     let d: i32 = 92;
// }
fn let_stmt(p: &mut Parser) {
    assert!(p.at(LET_KW));
    let m = p.start();
    p.bump();
    patterns::pattern(p);
    if p.at(COLON) {
        types::ascription(p);
    }
    if p.eat(EQ) {
        expressions::expr(p);
    }
    p.expect(SEMI);
    m.complete(p, LET_STMT);
}

// test return_expr
// fn foo() {
//     return;
//     return 92;
// }
fn return_expr(p: &mut Parser) -> CompletedMarker {
    assert!(p.at(RETURN_KW));
    let m = p.start();
    p.bump();
    if EXPR_FIRST.contains(p.current()) {
        expr(p);
    }
    m.complete(p, RETURN_EXPR)
}
