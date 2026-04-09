//! Integration tests for the tree-walk interpreter.

use holyc_frontend::{Lexer, Parser};
use holyc_interpreter::vm::Interpreter;
use holyc_stdlib::builtins::Value;

fn call(src: &str, func: &str, args: &[Value]) -> Value {
    let tokens  = Lexer::new(src).tokenize().expect("lex failed");
    let module  = Parser::new(tokens).parse_module().expect("parse failed");
    let mut interp = Interpreter::new();
    let _ = interp.exec_module(&module);
    interp.call_func(func, args).expect("call failed")
}

fn call_err(src: &str, func: &str, args: &[Value]) -> bool {
    let tokens  = Lexer::new(src).tokenize().unwrap();
    let module  = Parser::new(tokens).parse_module().unwrap();
    let mut interp = Interpreter::new();
    let _ = interp.exec_module(&module);
    interp.call_func(func, args).is_err()
}

// ── Arithmetic ────────────────────────────────────────────────────────────────
#[test] fn add()  { assert_eq!(call("I64 F(I64 a, I64 b){return a+b;}", "F", &[Value::Int(3), Value::Int(4)]), Value::Int(7)); }
#[test] fn sub()  { assert_eq!(call("I64 F(I64 a, I64 b){return a-b;}", "F", &[Value::Int(10),Value::Int(6)]), Value::Int(4)); }
#[test] fn mul()  { assert_eq!(call("I64 F(I64 a, I64 b){return a*b;}", "F", &[Value::Int(6), Value::Int(7)]), Value::Int(42)); }
#[test] fn div()  { assert_eq!(call("I64 F(I64 a, I64 b){return a/b;}", "F", &[Value::Int(17),Value::Int(5)]), Value::Int(3)); }
#[test] fn rem()  { assert_eq!(call("I64 F(I64 a, I64 b){return a%b;}", "F", &[Value::Int(17),Value::Int(5)]), Value::Int(2)); }
#[test] fn neg()  { assert_eq!(call("I64 F(I64 x){return -x;}",        "F", &[Value::Int(42)]),               Value::Int(-42)); }
#[test] fn shl()  { assert_eq!(call("I64 F(I64 x){return x<<3;}",      "F", &[Value::Int(1)]),                Value::Int(8)); }
#[test] fn shr()  { assert_eq!(call("I64 F(I64 x){return x>>2;}",      "F", &[Value::Int(64)]),               Value::Int(16)); }
#[test] fn band() { assert_eq!(call("I64 F(I64 a,I64 b){return a&b;}", "F", &[Value::Int(0b1100),Value::Int(0b1010)]), Value::Int(0b1000)); }
#[test] fn bor()  { assert_eq!(call("I64 F(I64 a,I64 b){return a|b;}", "F", &[Value::Int(0b1100),Value::Int(0b1010)]), Value::Int(0b1110)); }
#[test] fn bxor() { assert_eq!(call("I64 F(I64 a,I64 b){return a^b;}", "F", &[Value::Int(0b1100),Value::Int(0b1010)]), Value::Int(0b0110)); }
#[test] fn float_add() { assert_eq!(call("F64 F(F64 a,F64 b){return a+b;}", "F", &[Value::Float(1.5),Value::Float(2.5)]), Value::Float(4.0)); }

// ── Comparison ────────────────────────────────────────────────────────────────
#[test] fn eq_true()  { assert_eq!(call("Bool F(I64 a,I64 b){return a==b;}", "F", &[Value::Int(5),Value::Int(5)]),  Value::Bool(true)); }
#[test] fn eq_false() { assert_eq!(call("Bool F(I64 a,I64 b){return a==b;}", "F", &[Value::Int(5),Value::Int(6)]),  Value::Bool(false)); }
#[test] fn ne()       { assert_eq!(call("Bool F(I64 a,I64 b){return a!=b;}", "F", &[Value::Int(5),Value::Int(6)]),  Value::Bool(true)); }
#[test] fn lt()       { assert_eq!(call("Bool F(I64 a,I64 b){return a<b;}",  "F", &[Value::Int(3),Value::Int(5)]),  Value::Bool(true)); }
#[test] fn gt()       { assert_eq!(call("Bool F(I64 a,I64 b){return a>b;}",  "F", &[Value::Int(5),Value::Int(3)]),  Value::Bool(true)); }

// ── Logical short-circuit ─────────────────────────────────────────────────────
#[test] fn and_short() { assert_eq!(call("Bool F(){return FALSE&&(1/0==0);}", "F", &[]), Value::Bool(false)); }
#[test] fn or_short()  { assert_eq!(call("Bool F(){return TRUE||(1/0==0);}",  "F", &[]), Value::Bool(true)); }

// ── Variables & assignment ────────────────────────────────────────────────────
#[test] fn var_init()    { assert_eq!(call("I64 F(){I64 x=99;return x;}",        "F",&[]), Value::Int(99)); }
#[test] fn add_assign()  { assert_eq!(call("I64 F(){I64 x=10;x+=5;return x;}",   "F",&[]), Value::Int(15)); }
#[test] fn sub_assign()  { assert_eq!(call("I64 F(){I64 x=10;x-=3;return x;}",   "F",&[]), Value::Int(7)); }
#[test] fn mul_assign()  { assert_eq!(call("I64 F(){I64 x=4;x*=3;return x;}",    "F",&[]), Value::Int(12)); }
#[test] fn pre_inc()     { assert_eq!(call("I64 F(){I64 x=5;++x;return x;}",     "F",&[]), Value::Int(6)); }
#[test] fn post_inc_old(){ assert_eq!(call("I64 F(){I64 x=5;I64 y=x++;return y;}","F",&[]), Value::Int(5)); }
#[test] fn post_inc_new(){ assert_eq!(call("I64 F(){I64 x=5;x++;return x;}",     "F",&[]), Value::Int(6)); }

// ── Control flow ──────────────────────────────────────────────────────────────
#[test]
fn if_else_abs() {
    let src = "I64 Abs(I64 x){if(x<0){return -x;}else{return x;}}";
    assert_eq!(call(src,"Abs",&[Value::Int(-7)]), Value::Int(7));
    assert_eq!(call(src,"Abs",&[Value::Int(7)]),  Value::Int(7));
}

#[test]
fn while_sum() {
    let v = call("I64 F(I64 n){I64 s=0;I64 i=1;while(i<=n){s+=i;i++;}return s;}", "F", &[Value::Int(10)]);
    assert_eq!(v, Value::Int(55));
}

#[test]
fn for_sum() {
    let v = call("I64 F(I64 n){I64 s=0;for(I64 i=1;i<=n;i++){s+=i;}return s;}", "F", &[Value::Int(100)]);
    assert_eq!(v, Value::Int(5050));
}

#[test]
fn do_while() {
    let v = call("I64 F(){I64 x=0;do{x++;}while(x<5);return x;}", "F", &[]);
    assert_eq!(v, Value::Int(5));
}

#[test]
fn break_loop() {
    let v = call("I64 F(){I64 i=0;while(TRUE){if(i==7)break;i++;}return i;}", "F", &[]);
    assert_eq!(v, Value::Int(7));
}

#[test]
fn continue_loop() {
    let v = call("I64 F(){I64 s=0;for(I64 i=0;i<10;i++){if(i%2!=0)continue;s+=i;}return s;}", "F", &[]);
    assert_eq!(v, Value::Int(20)); // 0+2+4+6+8
}

#[test]
fn switch_stmt() {
    let src = "I64 F(I64 x){I64 r=0;switch(x){case 1:r=10;break;case 2:r=20;break;default:r=99;break;}return r;}";
    assert_eq!(call(src,"F",&[Value::Int(1)]), Value::Int(10));
    assert_eq!(call(src,"F",&[Value::Int(2)]), Value::Int(20));
    assert_eq!(call(src,"F",&[Value::Int(9)]), Value::Int(99));
}

// ── Recursion ─────────────────────────────────────────────────────────────────
#[test]
fn factorial() {
    let src = "I64 Fact(I64 n){if(n<=1)return 1;return n*Fact(n-1);}";
    assert_eq!(call(src,"Fact",&[Value::Int(0)]),  Value::Int(1));
    assert_eq!(call(src,"Fact",&[Value::Int(5)]),  Value::Int(120));
    assert_eq!(call(src,"Fact",&[Value::Int(10)]), Value::Int(3628800));
}

#[test]
fn fibonacci() {
    let src = "I64 Fib(I64 n){if(n<=1)return n;return Fib(n-1)+Fib(n-2);}";
    let exp = [0i64,1,1,2,3,5,8,13,21,34,55];
    for (i,&e) in exp.iter().enumerate() {
        assert_eq!(call(src,"Fib",&[Value::Int(i as i64)]), Value::Int(e));
    }
}

// ── Ternary ───────────────────────────────────────────────────────────────────
#[test] fn ternary_true()  { assert_eq!(call("I64 F(Bool b){return b?1:2;}", "F",&[Value::Bool(true)]),  Value::Int(1)); }
#[test] fn ternary_false() { assert_eq!(call("I64 F(Bool b){return b?1:2;}", "F",&[Value::Bool(false)]), Value::Int(2)); }

// ── sizeof ────────────────────────────────────────────────────────────────────
#[test] fn sizeof_i64() { assert_eq!(call("I64 F(){return sizeof(I64);}", "F",&[]), Value::UInt(8)); }
#[test] fn sizeof_u8()  { assert_eq!(call("I64 F(){return sizeof(U8);}",  "F",&[]), Value::UInt(1)); }

// ── Builtins ──────────────────────────────────────────────────────────────────
#[test] fn builtin_abs()    { assert_eq!(call("I64 F(){return Abs(-42);}",  "F",&[]), Value::Int(42)); }
#[test] fn builtin_sqrt()   { assert_eq!(call("F64 F(){return Sqrt(9.0);}", "F",&[]), Value::Float(3.0)); }
#[test] fn builtin_strlen() { assert_eq!(call("I64 F(){return StrLen(\"hello\");}", "F",&[]), Value::Int(5)); }

// ── Error cases ───────────────────────────────────────────────────────────────
#[test] fn div_zero_errors()   { assert!(call_err("I64 F(){return 1/0;}", "F", &[])); }
#[test] fn undef_var_errors()  { assert!(call_err("I64 F(){return x;}", "F", &[])); }
#[test] fn undef_func_errors() {
    let tokens = Lexer::new("").tokenize().unwrap();
    let module  = Parser::new(tokens).parse_module().unwrap();
    let mut i = Interpreter::new();
    let _ = i.exec_module(&module);
    assert!(i.call_func("NoSuch",&[]).is_err());
}
