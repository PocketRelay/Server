use std::{
    num::{ParseFloatError, ParseIntError},
    str::Split,
};

use thiserror::Error;

pub type ParseResult<T> = Result<T, ParseError>;

#[derive(Debug, Error)]
pub enum ParseError {
    #[error(transparent)]
    ParseInt(#[from] ParseIntError),

    #[error(transparent)]
    ParseFloat(#[from] ParseFloatError),

    #[error("not enough data parts")]
    NotEnoughParts,

    #[error("unexpected value")]
    UnexpectedValue,
}

pub fn next_int(p: &mut Split<'_, char>) -> ParseResult<u32> {
    let value = p.next().ok_or(ParseError::NotEnoughParts)?.parse()?;
    Ok(value)
}

pub fn next_float(p: &mut Split<'_, char>) -> ParseResult<f32> {
    let value = p.next().ok_or(ParseError::NotEnoughParts)?.parse()?;
    Ok(value)
}

pub fn next_string(p: &mut Split<'_, char>) -> ParseResult<String> {
    let value = p.next().ok_or(ParseError::NotEnoughParts)?;
    Ok(value.to_string())
}

pub fn next_str<'a>(p: &mut Split<'a, char>) -> ParseResult<&'a str> {
    let value = p.next().ok_or(ParseError::NotEnoughParts)?;
    Ok(value)
}

pub fn next_bool(p: &mut Split<'_, char>) -> ParseResult<bool> {
    let value = p.next().ok_or(ParseError::NotEnoughParts)?;
    match value {
        "True" => Ok(true),
        "False" => Ok(false),
        _ => Err(ParseError::UnexpectedValue),
    }
}
