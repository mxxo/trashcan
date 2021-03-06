//! code generation for trashcan items

use std::io;
use std::io::Write;

use ast::*;
use super::*;
use super::ty::*;

impl<'a> Emit<()> for NormalItem {
    fn emit<W: Write>(&self, out: &mut W, symtab: &SymbolTable,
      _ctxt: (), indent: u32) -> io::Result<()> {
        match *self {
            NormalItem::Function(ref def) =>
                def.emit(out, symtab, (), indent),

            NormalItem::Struct(ref def) =>
                def.emit(out, symtab, (), indent),

            NormalItem::Static(ref def) =>
                def.emit(out, symtab, (), indent),

            NormalItem::Const(ref def) =>
                def.emit(out, symtab, (), indent),
        }
    }
}

impl<'a> Emit<()> for FunDef {
    fn emit<W: Write>(&self, out: &mut W, symtab: &SymbolTable,
      _ctxt: (), indent: u32) -> io::Result<()> {
        self.access.emit(out, symtab, (), indent)?;

        let fnsub = match self.ret {
            Type::Void => "Sub",
            _ => "Function",
        };

        write!(out, " {} ", fnsub)?;

        self.name.emit(out, symtab, (), 0)?;

        out.write_all(b"(")?;

        // regular params
        for (i, p) in self.params.iter().enumerate() {
            if i != 0 {
                out.write_all(b", ")?;
            }
            p.emit(out, symtab, (), 0)?;
        }

        // optional params
        if !self.params.is_empty() && self.optparams.is_some() {
            out.write_all(b", ")?;
        }

        match self.optparams {
            Some(FunOptParams::Named(ref optparams)) => {
                for (i, &(ref p, ref default)) in optparams.iter().enumerate() {
                    if i != 0 {
                        out.write_all(b", ")?;
                    }
                    out.write_all(b"Optional ")?;
                    p.emit(out, symtab, (), 0)?;
                    out.write_all(b" = ")?;
                    default.emit(out, symtab, (), 0)?;
                }
            },

            Some(FunOptParams::VarArgs(ref name, _)) => {
                out.write_all(b"ParamArray ")?;
                name.emit(out, symtab, (), 0)?;
                out.write_all(b"() As Variant")?;
            },

            None => { },
        }

        out.write_all(b")")?;

        match self.ret {
            Type::Void => {},
            ref ty => ty.emit(out, symtab, TypePos::FunRet, 0)?,
        };

        out.write_all(b"\n")?;

        for stmt in self.body.iter() {
            stmt.emit(out, symtab, &self, indent + 1)?;
        }

        write!(out, "{:in$}End {}\n", "", fnsub,
          in = (indent * INDENT) as usize)
    }
}

impl Emit<()> for FunParam {
    fn emit<W: Write>(&self, out: &mut W, symtab: &SymbolTable,
      _ctxt: (), indent: u32) -> io::Result<()> {
        self.mode.emit(out, symtab, (), indent)?;
        out.write_all(b" ")?;
        self.name.emit(out, symtab, (), 0)?;
        self.ty.emit(out, symtab, TypePos::FunParam, 0)
    }
}

impl<'a> Emit<()> for StructDef {
    fn emit<W: Write>(&self, out: &mut W, symtab: &SymbolTable,
      _ctxt: (), indent: u32) -> io::Result<()> {
        self.access.emit(out, symtab, (), indent)?;
        out.write_all(b" Type ")?;
        self.name.emit(out, symtab, (), 0)?;
        out.write_all(b"\n")?;

        for m in &self.members {
            m.emit(out, symtab, (), indent + 1)?;
        }

        write!(out, "{:in$}End Type\n", "", in = (indent * INDENT) as usize)
    }
}

impl Emit<()> for StructMem {
    fn emit<W: Write>(&self, out: &mut W, symtab: &SymbolTable,
      _ctxt: (), indent: u32) -> io::Result<()> {
        self.name.emit(out, symtab, (), indent)?;
        self.ty.emit(out, symtab, TypePos::Decl, 0)?;
        out.write_all(b"\n")
    }
}

impl<'a> Emit<()> for Static {
    fn emit<W: Write>(&self, out: &mut W, symtab: &SymbolTable,
      _ctxt: (), indent: u32) -> io::Result<()> {
        self.access.emit(out, symtab, (), indent)?;
        out.write_all(b" ")?;
        self.name.emit(out, symtab, (), 0)?;
        self.ty.emit(out, symtab, TypePos::Decl, 0)?;
        if let Some(_) = self.init {
            panic!("dumpster fire: we can't emit static initializers \
                     (lazy statics) yet");
        }
        out.write_all(b"\n")
    }
}

impl<'a> Emit<()> for Constant {
    fn emit<W: Write>(&self, out: &mut W, symtab: &SymbolTable,
      _ctxt: (), indent: u32) -> io::Result<()> {
        self.access.emit(out, symtab, (), indent)?;
        out.write_all(b" Const ")?;
        self.name.emit(out, symtab, (), 0)?;
        self.ty.emit(out, symtab, TypePos::Decl, 0)?;
        out.write_all(b" = ")?;
        self.value.emit(out, symtab, (), 0)?;
        out.write_all(b"\n")
    }
}
