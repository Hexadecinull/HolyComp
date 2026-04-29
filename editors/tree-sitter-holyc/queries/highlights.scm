; Keywords
["if" "else" "while" "for" "do" "switch" "case" "default"
 "return" "break" "continue" "class" "typedef" "asm"
 "sizeof" "public" "private"] @keyword

; Types
(primitive_type) @type.builtin
(named_type) @type
(pointer_type) @type

; Literals
(integer_literal) @number
(float_literal) @number.float
(string_literal) @string
(char_literal) @character
(boolean_literal) @boolean
(null_literal) @constant.builtin

; Identifiers
(function_definition name: (identifier) @function)
(function_declaration name: (identifier) @function)
(call_expression callee: (identifier) @function.call)
(class_definition name: (identifier) @type)
(typedef_declaration alias: (identifier) @type.definition)
(parameter name: (identifier) @variable.parameter)
(variable_declaration name: (identifier) @variable)
(global_variable name: (identifier) @variable.global)
(field_declaration name: (identifier) @variable.member)
(member_expression (identifier) @variable.member)

; Operators
["+" "-" "*" "/" "%" "&" "|" "^" "~" "!" "<<" ">>"
 "==" "!=" "<" "<=" ">" ">=" "&&" "||"
 "=" "+=" "-=" "*=" "/=" "%=" "&=" "|=" "^=" "<<=" ">>="
 "++" "--" "->" "."] @operator

; Preprocessor
(preprocessor_define "#define" @keyword.directive name: (identifier) @constant)
(preprocessor_include "#include" @keyword.directive)

; Comments
(comment) @comment
