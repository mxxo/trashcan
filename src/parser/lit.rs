//! trashcan's sub-parsers for operators

use nom::{self, IResult, ErrorKind};

use ast::*;
use super::*;

named!(pub literal<Literal>, alt_complete!(
    literal_bool
  | literal_float // try this before int
  | literal_int
  | literal_string
//  TODO: "wacky" literal types
//  | literal_currency
//  | literal_date));
));

named!(literal_bool<Literal>, complete!(preceded!(
    opt!(call!(nom::multispace)),
    alt!(
        map!(tag!("true"), |_| Literal::Bool(true))
      | map!(tag!("false"), |_| Literal::Bool(false))
    )
)));

named!(literal_int<Literal>, complete!(map_res!(do_parse!(
         opt!(call!(nom::multispace)) >>
    num: call!(nom::digit) >>
    tag: opt!(complete!(alt!(
            tag!("u8")
          | tag!("i16")
          | tag!("i32")
          | tag!("isize")
         ))) >>
    (num, tag)), |(num, tag): (&[u8], Option<&[u8]>)| {
        let num = unsafe { str::from_utf8_unchecked(num) };
        let tag = tag.map(|t| unsafe { str::from_utf8_unchecked(t) });
        match tag {
            Some("u8") => num.parse::<u8>().map(Literal::UInt8),
            Some("i16") => num.parse::<i16>().map(Literal::Int16),
            Some("i32") => num.parse::<i32>().map(Literal::Int32),
            Some("isize") => num.parse::<i64>().map(Literal::IntPtr),
            // default i32
            None => num.parse::<i32>().map(Literal::Int32),
            _ => panic!("internal parser error")
        }
    })));

named!(literal_float<Literal>, complete!(map_res!(do_parse!(
         opt!(call!(nom::multispace)) >>
  whole: call!(nom::digit) >>
         char!('.') >> // mandatory decimal point
   frac: opt!(complete!(call!(nom::digit))) >>
    tag: opt!(complete!(alt!(
            tag!("f32")
          | tag!("f64")
         ))) >>
    (whole, frac, tag)), |(w, f, tag): (&[u8], Option<&[u8]>, Option<&[u8]>)| {
        let num = unsafe {
            let mut s = String::from(str::from_utf8_unchecked(w));
            match f {
                Some(frac) => {
                    s.push_str(".");
                    s.push_str(str::from_utf8_unchecked(frac));
                }
                None => {}
            }
            s
        };
        let tag = tag.map(|t| unsafe { str::from_utf8_unchecked(t) });
        match tag {
            Some("f32") => num.parse::<f32>().map(Literal::Float32),
            Some("f64") => num.parse::<f64>().map(Literal::Float64),
            // default f64
            None => num.parse::<f64>().map(Literal::Float64),
            _ => panic!("internal parser error")
        }
    })));

named!(literal_string<Literal>, map_res!(complete!(preceded!(
    opt!(call!(nom::multispace)),
    delimited!(
        char!('"'),
        escaped_string,
        char!('"')
    )
)), |bytes| {
    String::from_utf8(bytes).map(Literal::String)
}));

fn escaped_string(input: &[u8]) -> nom::IResult<&[u8], Vec<u8>> {
    let mut s = Vec::new();
    let mut bytes = input.iter();
    while let Some(c) = bytes.next() {
        if *c == b'"' {
            break;
        }

        if *c == b'\\' {
            match bytes.next() {
                Some(&b'n') => s.push(b'\n'),
                Some(&b't') => s.push(b'\t'),
                // TODO: more escapes here
                _ => return IResult::Error(
                    ErrorKind::Custom(CustomErrors::InvalidEscape as u32))
            }
        }

        // TODO: it'd be nice to allow rust style multiline strings
        //   (or maybe C-style adjacent-literal concatenation)
        // first option needs peek here; second just needs a change to the
        // literal_string production

        s.push(*c);
    }

    IResult::Done(&input[s.len()..], s)
}