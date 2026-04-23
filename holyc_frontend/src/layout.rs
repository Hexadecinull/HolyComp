//! Struct layout engine for HolyC.
//!
//! [`StructLayout`] computes the byte offset of every field inside a `class`
//! definition, respecting the natural alignment rules used by TempleOS (which
//! matches the System V AMD64 ABI for all primitive types).
//!
//! A [`TypeEnv`] collects both `class` definitions (as [`StructLayout`]s) and
//! `typedef` aliases so that any downstream pass — interpreter, LLVM codegen —
//! can resolve a [`HolyType::Named`] to a concrete size and field map without
//! re-scanning the AST.

use std::collections::HashMap;

use crate::types::HolyType;

// ── Field layout ─────────────────────────────────────────────────────────────

/// The resolved position of a single field inside its parent struct.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldLayout {
    /// Byte offset from the start of the struct.
    pub offset: u64,
    /// Byte size of this field (after typedef / nested-struct resolution).
    pub size: u64,
    /// The resolved concrete type (typedefs are expanded).
    pub ty: HolyType,
}

// ── Struct layout ─────────────────────────────────────────────────────────────

/// Fully computed layout for a single `class` definition.
#[derive(Debug, Clone, PartialEq)]
pub struct StructLayout {
    /// Total byte size including trailing padding.
    pub size: u64,
    /// Alignment requirement (largest field alignment, minimum 1).
    pub align: u64,
    /// Field name → layout entry, in declaration order.
    pub fields: Vec<(String, FieldLayout)>,
}

impl StructLayout {
    /// Look up a field by name; returns `None` if the field does not exist.
    pub fn field(&self, name: &str) -> Option<&FieldLayout> {
        self.fields.iter().find(|(n, _)| n == name).map(|(_, f)| f)
    }
}

// ── Type environment ──────────────────────────────────────────────────────────

/// Accumulated type knowledge for a compilation unit.
///
/// Populated during the first AST pass (before any interpretation or codegen)
/// and consulted whenever a [`HolyType::Named`] needs to be resolved.
#[derive(Debug, Clone, Default)]
pub struct TypeEnv {
    /// `class` / `union` names → their computed layouts.
    pub structs: HashMap<String, StructLayout>,
    /// `typedef` aliases → the underlying type they expand to.
    pub typedefs: HashMap<String, HolyType>,
}

impl TypeEnv {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a `typedef OldTy NewName;` mapping.
    pub fn add_typedef(&mut self, alias: impl Into<String>, ty: HolyType) {
        self.typedefs.insert(alias.into(), ty);
    }

    /// Register a struct layout under `name`.
    pub fn add_struct(&mut self, name: impl Into<String>, layout: StructLayout) {
        self.structs.insert(name.into(), layout);
    }

    /// Expand a single level of typedef, returning the underlying type.
    /// If `ty` is not a `Named` alias, it is returned unchanged.
    pub fn resolve_one(&self, ty: &HolyType) -> HolyType {
        match ty {
            HolyType::Named(name) => self
                .typedefs
                .get(name)
                .cloned()
                .unwrap_or_else(|| ty.clone()),
            _ => ty.clone(),
        }
    }

    /// Fully expand all typedef layers, returning the first non-alias type.
    /// Cycles are broken after 32 iterations (practically impossible in HolyC).
    pub fn resolve(&self, ty: &HolyType) -> HolyType {
        let mut cur = ty.clone();
        for _ in 0..32 {
            let next = self.resolve_one(&cur);
            if next == cur {
                return cur;
            }
            cur = next;
        }
        cur
    }

    /// Byte size of `ty` given this environment (resolves Named/typedef).
    /// Returns `None` for unsized types (e.g. unknown names).
    pub fn size_of(&self, ty: &HolyType) -> Option<u64> {
        match self.resolve(ty) {
            HolyType::Named(name) => self.structs.get(&name).map(|l| l.size),
            resolved => resolved.size_of(),
        }
    }

    /// Alignment of `ty` given this environment.
    pub fn align_of(&self, ty: &HolyType) -> u64 {
        match self.resolve(ty) {
            HolyType::Named(name) => self.structs.get(&name).map(|l| l.align).unwrap_or(1),
            HolyType::I8 | HolyType::U8 | HolyType::Bool => 1,
            HolyType::I16 | HolyType::U16 => 2,
            HolyType::I32 | HolyType::U32 | HolyType::F32 => 4,
            _ => 8,
        }
    }

    /// Compute and register the layout of a struct from its raw field list.
    ///
    /// Fields are laid out in declaration order with natural alignment.
    /// The struct's total size is rounded up to its own alignment.
    pub fn compute_layout(
        &mut self,
        name: impl Into<String>,
        fields: &[(String, HolyType)],
    ) -> StructLayout {
        let mut offset: u64 = 0;
        let mut struct_align: u64 = 1;
        let mut out_fields = Vec::with_capacity(fields.len());

        for (fname, fty) in fields {
            let resolved = self.resolve(fty);
            let size = self.size_of(&resolved).unwrap_or(8);
            let align = self.align_of(&resolved);

            // Align the current offset up to this field's alignment.
            if offset % align != 0 {
                offset += align - (offset % align);
            }

            out_fields.push((
                fname.clone(),
                FieldLayout {
                    offset,
                    size,
                    ty: resolved,
                },
            ));

            offset += size;
            if align > struct_align {
                struct_align = align;
            }
        }

        // Round total size up to struct alignment.
        if struct_align > 0 && offset % struct_align != 0 {
            offset += struct_align - (offset % struct_align);
        }

        let layout = StructLayout {
            size: offset,
            align: struct_align,
            fields: out_fields,
        };
        self.structs.insert(name.into(), layout.clone());
        layout
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_struct_offsets() {
        let mut env = TypeEnv::new();
        let layout = env.compute_layout(
            "Point",
            &[("x".into(), HolyType::I64), ("y".into(), HolyType::I64)],
        );
        assert_eq!(layout.size, 16);
        assert_eq!(layout.field("x").unwrap().offset, 0);
        assert_eq!(layout.field("y").unwrap().offset, 8);
    }

    #[test]
    fn mixed_field_alignment() {
        // struct { U8 a; I64 b; U8 c; }
        // a @ 0 (size 1), pad 7, b @ 8 (size 8), c @ 16 (size 1), pad 7 → total 24
        let mut env = TypeEnv::new();
        let layout = env.compute_layout(
            "Mixed",
            &[
                ("a".into(), HolyType::U8),
                ("b".into(), HolyType::I64),
                ("c".into(), HolyType::U8),
            ],
        );
        assert_eq!(layout.field("a").unwrap().offset, 0);
        assert_eq!(layout.field("b").unwrap().offset, 8);
        assert_eq!(layout.field("c").unwrap().offset, 16);
        assert_eq!(layout.size, 24);
    }

    #[test]
    fn typedef_resolved_in_struct() {
        let mut env = TypeEnv::new();
        env.add_typedef("Index", HolyType::I64);
        let layout =
            env.compute_layout("Entry", &[("idx".into(), HolyType::Named("Index".into()))]);
        assert_eq!(layout.size, 8);
        assert_eq!(layout.field("idx").unwrap().size, 8);
        assert_eq!(layout.field("idx").unwrap().ty, HolyType::I64);
    }

    #[test]
    fn nested_struct_layout() {
        let mut env = TypeEnv::new();
        // Inner: { I64 x; I64 y; } → size 16
        env.compute_layout(
            "Vec2",
            &[("x".into(), HolyType::I64), ("y".into(), HolyType::I64)],
        );
        // Outer: { Vec2 pos; I64 z; } → pos@0(16), z@16(8) → size 24
        let outer = env.compute_layout(
            "Vec3",
            &[
                ("pos".into(), HolyType::Named("Vec2".into())),
                ("z".into(), HolyType::I64),
            ],
        );
        assert_eq!(outer.field("pos").unwrap().offset, 0);
        assert_eq!(outer.field("pos").unwrap().size, 16);
        assert_eq!(outer.field("z").unwrap().offset, 16);
        assert_eq!(outer.size, 24);
    }

    #[test]
    fn typedef_resolve_chain() {
        let mut env = TypeEnv::new();
        env.add_typedef("MyI64", HolyType::I64);
        env.add_typedef("AliasOfMyI64", HolyType::Named("MyI64".into()));
        assert_eq!(
            env.resolve(&HolyType::Named("AliasOfMyI64".into())),
            HolyType::I64
        );
    }

    #[test]
    fn size_of_named() {
        let mut env = TypeEnv::new();
        env.compute_layout(
            "Pair",
            &[("a".into(), HolyType::I64), ("b".into(), HolyType::I64)],
        );
        assert_eq!(env.size_of(&HolyType::Named("Pair".into())), Some(16));
    }
}
