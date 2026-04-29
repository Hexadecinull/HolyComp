/**
 * tree-sitter grammar for HolyC.
 *
 * Covers the subset of TempleOS HolyC implemented by HolyComp:
 * primitive types, expressions, statements, functions, classes, typedefs,
 * and preprocessor directives.
 *
 * Install:  npm install && npx tree-sitter generate
 * Test:     npx tree-sitter test
 */

module.exports = grammar({
  name: 'holyc',

  extras: $ => [$.comment, /\s/],

  word: $ => $.identifier,

  rules: {
    source_file: $ => repeat($._top_level),

    // ── Top-level items ───────────────────────────────────────────────────────

    _top_level: $ => choice(
      $.function_definition,
      $.function_declaration,
      $.global_variable,
      $.class_definition,
      $.typedef_declaration,
      $.preprocessor_define,
      $.preprocessor_include,
    ),

    function_definition: $ => seq(
      optional($.visibility),
      field('return_type', $._type),
      field('name', $.identifier),
      '(', optional($.parameter_list), ')',
      field('body', $.block),
    ),

    function_declaration: $ => seq(
      field('return_type', $._type),
      field('name', $.identifier),
      '(', optional($.parameter_list), ')',
      ';',
    ),

    global_variable: $ => seq(
      optional($.visibility),
      field('type', $._type),
      field('name', $.identifier),
      optional(seq('=', field('init', $._expression))),
      ';',
    ),

    class_definition: $ => seq(
      'class',
      field('name', $.identifier),
      '{',
      repeat($.field_declaration),
      '}',
      optional(';'),
    ),

    field_declaration: $ => seq(
      field('type', $._type),
      field('name', $.identifier),
      optional(seq(':', $.integer_literal)),
      ';',
    ),

    typedef_declaration: $ => seq(
      'typedef',
      field('type', $._type),
      field('alias', $.identifier),
      ';',
    ),

    preprocessor_define: $ => seq(
      '#define',
      field('name', $.identifier),
      optional(field('value', $._expression)),
    ),

    preprocessor_include: $ => seq(
      '#include',
      choice(
        seq('"', $.string_content, '"'),
        seq('<', $.string_content, '>'),
      ),
    ),

    string_content: $ => /[^"<>\n]+/,

    visibility: $ => choice('public', 'private'),

    parameter_list: $ => seq(
      $.parameter,
      repeat(seq(',', $.parameter)),
    ),

    parameter: $ => seq(
      field('type', $._type),
      field('name', $.identifier),
    ),

    // ── Types ─────────────────────────────────────────────────────────────────

    _type: $ => choice($.primitive_type, $.pointer_type, $.array_type, $.named_type),

    primitive_type: $ => choice(
      'U0', 'I8', 'U8', 'I16', 'U16', 'I32', 'U32', 'I64', 'U64',
      'F32', 'F64', 'Bool',
    ),

    pointer_type: $ => seq($._type, '*'),

    array_type: $ => seq(
      $._type,
      '[', optional($.integer_literal), ']',
    ),

    named_type: $ => $.identifier,

    // ── Statements ────────────────────────────────────────────────────────────

    block: $ => seq('{', repeat($._statement), '}'),

    _statement: $ => choice(
      $.block,
      $.variable_declaration,
      $.if_statement,
      $.while_statement,
      $.do_while_statement,
      $.for_statement,
      $.switch_statement,
      $.return_statement,
      $.break_statement,
      $.continue_statement,
      $.asm_statement,
      $.expression_statement,
    ),

    variable_declaration: $ => seq(
      field('type', $._type),
      field('name', $.identifier),
      optional(seq('[', optional($.integer_literal), ']')),
      optional(seq('=', field('init', $._expression))),
      ';',
    ),

    if_statement: $ => seq(
      'if', '(', field('condition', $._expression), ')',
      field('then', $._statement),
      optional(seq('else', field('else', $._statement))),
    ),

    while_statement: $ => seq(
      'while', '(', field('condition', $._expression), ')',
      field('body', $._statement),
    ),

    do_while_statement: $ => seq(
      'do', field('body', $._statement),
      'while', '(', field('condition', $._expression), ')', ';',
    ),

    for_statement: $ => seq(
      'for', '(',
      optional(choice($.variable_declaration, $.expression_statement)),
      optional(field('condition', $._expression)), ';',
      optional(field('step', $._expression)),
      ')',
      field('body', $._statement),
    ),

    switch_statement: $ => seq(
      'switch', '(', field('value', $._expression), ')',
      '{', repeat($.switch_case), '}',
    ),

    switch_case: $ => seq(
      choice(
        seq('case', $._expression, ':'),
        seq('default', ':'),
      ),
      repeat($._statement),
    ),

    return_statement: $ => seq('return', optional($._expression), ';'),
    break_statement: $ => seq('break', ';'),
    continue_statement: $ => seq('continue', ';'),

    asm_statement: $ => seq('asm', '{', /[^}]*/, '}'),

    expression_statement: $ => seq($._expression, ';'),

    // ── Expressions ───────────────────────────────────────────────────────────

    _expression: $ => choice(
      $.assignment_expression,
      $.ternary_expression,
      $.binary_expression,
      $.unary_expression,
      $.call_expression,
      $.subscript_expression,
      $.member_expression,
      $.cast_expression,
      $.sizeof_expression,
      $.identifier,
      $.integer_literal,
      $.float_literal,
      $.string_literal,
      $.char_literal,
      $.boolean_literal,
      $.null_literal,
      $.parenthesized_expression,
    ),

    assignment_expression: $ => prec.right(2, seq(
      $._expression,
      choice('=','+=','-=','*=','/=','%=','&=','|=','^=','<<=','>>='),
      $._expression,
    )),

    ternary_expression: $ => prec.right(3, seq(
      $._expression, '?', $._expression, ':', $._expression,
    )),

    binary_expression: $ => {
      const table = [
        [4,  ['||']],
        [6,  ['&&']],
        [8,  ['|']],
        [10, ['^']],
        [12, ['&']],
        [14, ['==','!=']],
        [16, ['<','<=','>','>=']],
        [18, ['<<','>>']],
        [20, ['+','-']],
        [22, ['*','/','%']],
      ];
      return choice(...table.map(([prec, ops]) =>
        prec.left(prec, seq($._expression, choice(...ops), $._expression))
      ));
    },

    unary_expression: $ => choice(
      prec.right(30, seq(choice('-','~','!','*','&','++','--'), $._expression)),
      prec.left(30,  seq($._expression, choice('++','--'))),
    ),

    call_expression: $ => prec(40, seq(
      field('callee', $._expression),
      '(',
      optional(seq($._expression, repeat(seq(',', $._expression)))),
      ')',
    )),

    subscript_expression: $ => prec(40, seq(
      $._expression, '[', $._expression, ']',
    )),

    member_expression: $ => prec.left(40, seq(
      $._expression, choice('.', '->'), $.identifier,
    )),

    cast_expression: $ => prec.right(35, seq(
      '(', $._type, ')', $._expression,
    )),

    sizeof_expression: $ => seq(
      'sizeof', '(',
      choice($._type, $._expression),
      ')',
    ),

    parenthesized_expression: $ => seq('(', $._expression, ')'),

    // ── Literals ──────────────────────────────────────────────────────────────

    integer_literal: $ => /0[xX][0-9a-fA-F]+|[0-9]+/,
    float_literal: $ => /[0-9]+\.[0-9]+([eE][+-]?[0-9]+)?/,
    string_literal: $ => seq('"', /([^"\\]|\\.)*/, '"'),
    char_literal: $ => seq("'", /([^'\\]|\\.)/, "'"),
    boolean_literal: $ => choice('TRUE', 'FALSE', 'true', 'false'),
    null_literal: $ => choice('NULL', 'null'),

    identifier: $ => /[a-zA-Z_][a-zA-Z0-9_]*/,

    comment: $ => choice(
      seq('//', /.*/),
      seq('/*', /[^*]*\*+([^/*][^*]*\*+)*/, '/'),
    ),
  },
});
