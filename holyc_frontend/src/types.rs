//! HolyC type representation.

/// Every type that can appear in HolyC source code.
#[derive(Debug, Clone, PartialEq)]
pub enum HolyType {
    // Primitive integers
    I8, U8,
    I16, U16,
    I32, U32,
    I64, U64,
    // Floating-point
    F32, F64,
    // Other primitives
    Bool,
    /// `U0` – void / unit
    Void,
    // Compound / derived
    Ptr(Box<HolyType>),
    /// Fixed-length array, `None` = unsized (decayed from pointer)
    Array { elem: Box<HolyType>, len: Option<u64> },
    /// Named struct/class/typedef
    Named(String),
    /// Function-pointer: `ret_ty (*)(param_tys)`
    FnPtr { ret: Box<HolyType>, params: Vec<HolyType> },
}

impl HolyType {
    /// Return the byte-size of the type as it would be in TempleOS (64-bit).
    pub fn size_of(&self) -> Option<u64> {
        match self {
            HolyType::I8  | HolyType::U8               => Some(1),
            HolyType::I16 | HolyType::U16              => Some(2),
            HolyType::I32 | HolyType::U32 | HolyType::F32 => Some(4),
            HolyType::I64 | HolyType::U64 | HolyType::F64 => Some(8),
            HolyType::Bool                              => Some(1),
            HolyType::Void                              => Some(0),
            HolyType::Ptr(_) | HolyType::FnPtr { .. }  => Some(8),
            HolyType::Array { elem, len: Some(n) } => {
                elem.size_of().map(|s| s * n)
            }
            HolyType::Array { .. } | HolyType::Named(_) => None,
        }
    }

    /// Is this a signed integer type?
    pub fn is_signed_int(&self) -> bool {
        matches!(self, HolyType::I8 | HolyType::I16 | HolyType::I32 | HolyType::I64)
    }

    /// Is this any integer type (signed or unsigned)?
    pub fn is_integer(&self) -> bool {
        matches!(
            self,
            HolyType::I8  | HolyType::U8  |
            HolyType::I16 | HolyType::U16 |
            HolyType::I32 | HolyType::U32 |
            HolyType::I64 | HolyType::U64
        )
    }

    /// Is this a floating-point type?
    pub fn is_float(&self) -> bool {
        matches!(self, HolyType::F32 | HolyType::F64)
    }
}

impl std::fmt::Display for HolyType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HolyType::I8   => write!(f, "I8"),
            HolyType::U8   => write!(f, "U8"),
            HolyType::I16  => write!(f, "I16"),
            HolyType::U16  => write!(f, "U16"),
            HolyType::I32  => write!(f, "I32"),
            HolyType::U32  => write!(f, "U32"),
            HolyType::I64  => write!(f, "I64"),
            HolyType::U64  => write!(f, "U64"),
            HolyType::F32  => write!(f, "F32"),
            HolyType::F64  => write!(f, "F64"),
            HolyType::Bool => write!(f, "Bool"),
            HolyType::Void => write!(f, "U0"),
            HolyType::Ptr(inner) => write!(f, "{inner}*"),
            HolyType::Array { elem, len: Some(n) } => write!(f, "{elem}[{n}]"),
            HolyType::Array { elem, len: None }    => write!(f, "{elem}[]"),
            HolyType::Named(name)                  => write!(f, "{name}"),
            HolyType::FnPtr { ret, params } => {
                write!(f, "{ret} (*)(")?;
                for (i, p) in params.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{p}")?;
                }
                write!(f, ")")
            }
        }
    }
}
