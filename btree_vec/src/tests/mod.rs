use std::{fs, io::BufReader, path::Path};

use ron::{tokenize, ParseRon};

use crate::{BTreeVecNode, BTreeVecNodeValue};

#[test]
fn overflow() {

    let mut node = parse_reproduction_files::<BTreeVecNode<u64, ()>>("src/tests/reproduce_overflow_lessbigarray.ron").unwrap();

    validate_assumptions_on_node(&node);

    node.push(259417569017,())
    
}

fn validate_assumptions_on_node(node: &BTreeVecNode<u64, ()>) {
    assert_eq!(node.keys.len(), node.values.len());

    for window in node.keys.windows(2) {
        let (first_min, first_max) = window[0];
        let (second_min, second_max) = window[1];

        assert!(first_min < second_min);
        assert!(first_min < first_max);
        assert!(first_max < second_min);
        assert!(second_min < second_max);
    }

    for value in node.values.iter() {
        if let BTreeVecNodeValue::ChildList(child) = value {
            validate_assumptions_on_node(child);
        }
    }
}

fn parse_reproduction_files<T: ParseRon>(filename: impl AsRef<Path>) -> std::io::Result<T> {
    let file = fs::File::open(filename)?;

    let reader = BufReader::new(file);
    let mut tokens = tokenize(reader).peekable();

    T::parse_ron(&mut tokens)
}

mod ron {
    use std::{io::Result, iter::Peekable, num::ParseIntError};

    use crate::{nonempty_vec::NonEmptyUnorderVec, BTreeVecNode, BTreeVecNodeValue};

    #[derive(Debug, PartialEq)]
    pub enum Token {
        OpenCurly,
        OpenSquare,
        OpenParen,
        CloseCurly,
        CloseSquare,
        CloseParen,
        Comma,
        Identifier(String),
        Int(u64),
        Colon,
    }
    enum TokenizeState {
        Start,
        Identifier(String),
        Number(String),
    }
    fn to_io_error(err: ParseIntError) -> std::io::Error {
        (match err.kind() {
            std::num::IntErrorKind::Empty => std::io::ErrorKind::UnexpectedEof,
            std::num::IntErrorKind::InvalidDigit => std::io::ErrorKind::InvalidData,
            std::num::IntErrorKind::PosOverflow => std::io::ErrorKind::Unsupported,
            std::num::IntErrorKind::NegOverflow => std::io::ErrorKind::Unsupported,
            std::num::IntErrorKind::Zero => std::io::ErrorKind::Unsupported,
            _ => std::io::ErrorKind::InvalidData,
        })
        .into()
    }

    pub fn tokenize(reader: impl std::io::Read) -> impl Iterator<Item = Result<Token>> {
        let mut bytes = reader.bytes().peekable();

        return std::iter::from_fn(move || {
            let mut state = TokenizeState::Start;
            loop {
                match state {
                    TokenizeState::Start => match bytes.next() {
                        Some(Err(_)) => {
                            return Some(Err(bytes.next()?.unwrap_err()));
                        }
                        Some(Ok(b' ' | b'\n' | b'\t' | b'\r')) => continue,
                        Some(Ok(b']')) => return Some(Ok(Token::CloseSquare)),
                        Some(Ok(b'(')) => return Some(Ok(Token::OpenParen)),
                        Some(Ok(b'[')) => return Some(Ok(Token::OpenSquare)),
                        Some(Ok(b'{')) => return Some(Ok(Token::OpenCurly)),
                        Some(Ok(b'}')) => return Some(Ok(Token::CloseCurly)),
                        Some(Ok(b')')) => return Some(Ok(Token::CloseParen)),
                        Some(Ok(b',')) => return Some(Ok(Token::Comma)),
                        Some(Ok(b':')) => return Some(Ok(Token::Colon)),
                        Some(Ok(b @ b'0'..=b'9')) => {
                            state = TokenizeState::Number((b as char).to_string())
                        }
                        Some(Ok(b)) => state = TokenizeState::Identifier((b as char).to_string()),
                        None => break,
                    },
                    TokenizeState::Identifier(ref mut str) => match bytes.peek() {
                        Some(Err(_)) => {
                            return Some(Err(bytes.next()?.unwrap_err()));
                        }
                        Some(Ok(b'a'..=b'z' | b'A'..=b'Z' | b'_')) => {
                            let next_char = bytes.next()?.unwrap();
                            str.push(next_char as char);
                        }
                        Some(Ok(_)) => {
                            let str = std::mem::take(str);
                            return Some(Ok(Token::Identifier(str)))
                        },
                        None => break,
                    },
                    TokenizeState::Number(ref mut n) => match bytes.peek() {
                        Some(Err(_)) => {
                            return Some(Err(bytes.next()?.unwrap_err()));
                        }
                        Some(Ok(b'0'..=b'9')) => {
                            let next_char = bytes.next()?.unwrap();
                            n.push(next_char as char);
                        }
                        Some(Ok(_)) => return Some(n.parse().map_err(to_io_error).map(Token::Int)),
                        None => break,
                    },
                }
            }

            //if the file ends in the middle of a token:
            match state {
                TokenizeState::Start => None,
                TokenizeState::Identifier(s) => Some(Ok(Token::Identifier(s))),
                TokenizeState::Number(n) => Some(n.parse().map_err(to_io_error).map(Token::Int)),
            }
        });
    }

    pub trait ParseRon: Sized {
        fn parse_ron(tokens: &mut Peekable<impl Iterator<Item = Result<Token>>>) -> Result<Self>;
    }

    impl<A:ParseRon,B:ParseRon> ParseRon for (A,B) {
        fn parse_ron(tokens: &mut Peekable<impl Iterator<Item = Result<Token>>>) -> Result<Self> {
            expect(tokens, Token::OpenParen)?;

            let a = A::parse_ron(tokens)?;

            expect(tokens, Token::Comma)?;

            let b = B::parse_ron(tokens)?;

            expect(tokens, Token::CloseParen)?;

            Ok((a,b))
        }
    }

    impl<T:ParseRon> ParseRon for Option<T> {
        fn parse_ron(tokens: &mut Peekable<impl Iterator<Item = Result<Token>>>) -> Result<Self> {
            let tag = identifier(tokens)?;

            if tag == "None" {return Ok(None);}

            if tag != "Some" {
                return invalid_data();
            }

            expect(tokens, Token::OpenParen)?;

            let t = T::parse_ron(tokens)?;

            expect(tokens, Token::CloseParen)?;

            Ok(Some(t))
        }
    }

    impl ParseRon for () {
        fn parse_ron(tokens: &mut Peekable<impl Iterator<Item = Result<Token>>>) -> Result<Self> {
            expect(tokens, Token::OpenParen)?;
            expect(tokens, Token::CloseParen)?;
            Ok(())
        }
    }

    impl ParseRon for u64 {
        fn parse_ron(tokens: &mut Peekable<impl Iterator<Item = Result<Token>>>) -> Result<Self> {
            match tokens.next().transpose()? {
                Some(Token::Int(i)) => Ok(i),
                Some(_) => invalid_data(),
                None => unexpected_eof(),
            }
        }
    }

    impl<K:ParseRon,V:ParseRon> ParseRon for BTreeVecNode<K,V> {
        fn parse_ron(tokens: &mut Peekable<impl Iterator<Item = Result<Token>>>) -> Result<Self> {
            //ensure and consume the label stating the type
            if identifier(tokens)? != "BTreeVecNode" {
                return invalid_data();
            }


            let mut obj = BTreeVecNode {
                keys: Vec::with_capacity(0),
                values: Vec::with_capacity(0)
            };

            expect(tokens, Token::OpenCurly)?;

            loop {
                consume_all(tokens, Token::Comma)?;

                if next_if_eq(tokens, Token::CloseCurly)? {
                    break;
                }

                let field_name = identifier(tokens)?;
                expect(tokens, Token::Colon)?;

                match &*field_name {
                    "keys" => {
                        obj.keys = ParseRon::parse_ron(tokens)?;
                    },
                    "values" => {
                        obj.values = ParseRon::parse_ron(tokens)?;
                    }
                    _ => {return invalid_data()},
                }
            }

            Ok(obj)
        }
    }

    impl<K: ParseRon, V: ParseRon> ParseRon for BTreeVecNodeValue<K,V> {
        fn parse_ron(tokens: &mut Peekable<impl Iterator<Item = Result<Token>>>) -> Result<Self> {
            let tag = identifier(tokens)?;

            expect(tokens, Token::OpenParen)?;

            let t = match &*tag {
                "Leaf" => BTreeVecNodeValue::Leaf(ParseRon::parse_ron(tokens)?),
                "ChildList" => BTreeVecNodeValue::ChildList(ParseRon::parse_ron(tokens)?),
                _ => return invalid_data(),
            };

            expect(tokens, Token::CloseParen)?;

            Ok(t)
        }
    }

    impl<T: ParseRon> ParseRon for NonEmptyUnorderVec<T> {
        fn parse_ron(tokens: &mut Peekable<impl Iterator<Item = Result<Token>>>) -> Result<Self> {
            if identifier(tokens)? != "NonEmptyUnorderVec" {
                return invalid_data();
            }

            expect(tokens, Token::OpenParen)?;

            let head = T::parse_ron(tokens)?;

            expect(tokens, Token::Comma)?;

            let rest = Vec::<T>::parse_ron(tokens)?;

            expect(tokens, Token::CloseParen)?;

            Ok(NonEmptyUnorderVec::from_head_and_rest(head, rest))
        }
    }

    impl<T: ParseRon> ParseRon for Vec<T> {
        fn parse_ron(tokens: &mut Peekable<impl Iterator<Item = Result<Token>>>) -> Result<Self> {
            expect(tokens, Token::OpenSquare)?;

            let mut v = Vec::new();

            loop {
                consume_all(tokens, Token::Comma)?;
                if next_if_eq(tokens, Token::CloseSquare)? {
                    break;
                }
                v.push(T::parse_ron(tokens)?);
            }

            Ok(v)
        }
    }

    fn next_if_eq(tokens: &mut Peekable<impl Iterator<Item = Result<Token>>>, target_tkn: Token) -> Result<bool> {
        match tokens.peek() {
            Some(Err(_)) => Err(tokens.next().unwrap().unwrap_err()),
            Some(Ok(tkn)) => {
                if *tkn == target_tkn {
                    tokens.next();
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            None => Ok(false),
        }
    }

    fn consume_all(tokens: &mut Peekable<impl Iterator<Item = Result<Token>>>, target_tkn: Token) -> Result<()> {
        loop {
            match tokens.peek() {
                Some(Err(_)) => return Err(tokens.next().unwrap().unwrap_err()),
                Some(Ok(tkn)) => {
                    if *tkn == target_tkn {
                        tokens.next();
                        continue;
                    } else {
                        break;
                    }
                }
                None => break,
            }
        }
        Ok(())
    }

    fn expect(tokens: &mut Peekable<impl Iterator<Item = Result<Token>>>, target_tkn: Token) -> Result<()> {
        match tokens.next().transpose()? {
            None => unexpected_eof(),
            Some(tkn) if tkn == target_tkn => Ok(()),
            Some(_) => invalid_data()
        }
    }

    fn identifier(tokens: &mut Peekable<impl Iterator<Item = Result<Token>>>) -> Result<String> {
        match tokens.next().transpose()? {
            None => return unexpected_eof(),
            Some(Token::Identifier(s)) => Ok(s),
            Some(_) => return invalid_data()
        }

    }
    
    fn invalid_data<T>() -> std::result::Result<T, std::io::Error> {
        return Err(std::io::ErrorKind::InvalidData.into());
    }
    
    fn unexpected_eof<T>() -> std::result::Result<T, std::io::Error> {
        return Err(std::io::ErrorKind::UnexpectedEof.into());
    }
}
