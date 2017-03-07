use std::env;
use std::fs::File;

use std::io;
use std::io::Read;
use std::io::Write;

use std::collections::HashMap;

extern crate nom;

extern crate trashcan;
use trashcan::parser;
use trashcan::analysis;
use trashcan::codegen::Emit;

fn main() {
    let args = env::args_os().collect::<Vec<_>>();
    if args.len() <= 1 {
        return;
    }

    let mut dumpsters = vec![];
    for f in &args[1..] {
        let mut file = File::open(f).expect(
            &format!("Unable to open {}.", f.to_string_lossy()));
        let mut contents = vec![];
        let _ = file.read_to_end(&mut contents).expect(
            &format!("Unable to read {}.", f.to_string_lossy()));
        let strip = parser::strip_comments(&contents);
        match parser::dumpster(&strip) {
            nom::IResult::Done(rest, dumpster) => {
                if !rest.is_empty() {
                    panic!("Invalid trailing content in {}",
                           f.to_string_lossy());
                }
                dumpsters.push(dumpster);
            },

            nom::IResult::Error(err) => {
                println!("{}", String::from_utf8_lossy(&strip));
                panic!("Parse error: {:?}", err);
            },

            res => {
                panic!("Something weird: {:?}", res);
            },
        }
    }

    let mut stdout = io::LineWriter::new(io::stdout());
    let dumpster = analysis::merge_dumpsters(dumpsters);
    let dumpster = analysis::for_loop_var_gensym(dumpster);
    let dumpster = analysis::short_circuit_logicals(dumpster);
    let symtab = analysis::symbol_table(&dumpster).expect("symtab error");

    for (i, m) in dumpster.modules.iter().enumerate() {
        if i != 0 {
            stdout.write_all(b"\n").unwrap();
        }
        m.emit(&mut stdout, &symtab, (), 0).unwrap();
    }

    stdout.write_all(b"\nSYMBOL TABLE DUMP\n").unwrap();
    for (m, tbl) in symtab {
        write!(stdout, "module {}:\n", m).unwrap();
        dump_tbl(&mut stdout, tbl, 1);
    }
}

fn dump_tbl<W: Write>(out: &mut W, tbl: HashMap<String, analysis::Symbol>,
  ind: usize) {
    for (k, sym) in tbl {
        write!(out, "{:in$}item {}: ", "", k, in=ind*4).unwrap();
        match sym {
            analysis::Symbol::Const(ty) =>
                write!(out, "constant {:?}\n", ty).unwrap(),
            analysis::Symbol::Value(ty, mode) =>
                write!(out, "value {:?} {:?}\n", mode, ty).unwrap(),
            analysis::Symbol::Type(ty) =>
                write!(out, "type {:?}\n", ty).unwrap(),
            analysis::Symbol::Fun { def, locals } => {
                write!(out, "fn {}\n", def.name.0).unwrap();
                dump_tbl(out, locals, ind + 1);
            },
        }
    }
}
